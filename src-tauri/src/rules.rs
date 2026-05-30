use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use regex::Regex;
use chrono::Local;
use futures::StreamExt;
use sinew_core::{
    ChatMessage, ModelRef, Part, Provider, ProviderRequest, Role, StreamEvent,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorItem {
    pub id: String,
    pub count: i64,
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consolidated_at: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

fn normalize(s: &str) -> String {
    let s = s.to_lowercase().replace('_', " ").replace('-', " ");
    let re = Regex::new(r"\s+").unwrap();
    let s = re.replace_all(&s, " ");
    s.trim().to_string()
}

fn rule_covers_error(rules_text: &str, error_id: &str) -> bool {
    let rules_norm = normalize(rules_text);
    let id_norm = normalize(error_id);
    if !id_norm.is_empty() && rules_norm.contains(&id_norm) {
        return true;
    }
    
    let aliases = match error_id {
        "git_exclusions_build_node_modules" => vec!["node_modules", "build/"],
        "spawn_einval_windows" => vec!["spawn einval", "shell: true"],
        "recursive_postinstall_npm" => vec!["postinstall", "npm install"],
        "mcp_autoload_serialization" => vec!["autoload", "settingstojson"],
        "absolute_paths_windows" => vec!["chemins de fichiers absolus", "chemins relatifs"],
        _ => vec![],
    };
    
    if !aliases.is_empty() {
        if aliases.iter().all(|alias| rules_norm.contains(&normalize(alias))) {
            return true;
        }
    }
    
    false
}

fn next_rule_number(rules_text: &str) -> usize {
    let re = Regex::new(r"^###\s+(\d+)\.").unwrap();
    let mut max_val = 0;
    for line in rules_text.lines() {
        if let Some(caps) = re.captures(line.trim()) {
            if let Ok(num) = caps[1].parse::<usize>() {
                if num > max_val {
                    max_val = num;
                }
            }
        }
    }
    max_val + 1
}

fn title_from_error_id(error_id: &str) -> String {
    error_id
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}

fn build_rule(number: usize, error_id: &str, description: &str) -> String {
    let title = title_from_error_id(error_id);
    let desc_clean = description.replace("\r\n", " ").replace('\n', " ");
    let re_spaces = Regex::new(r"\s+").unwrap();
    let desc_clean = re_spaces.replace_all(&desc_clean, " ");
    let desc_clean = desc_clean.trim().trim_end_matches('.');
    format!(
        "\n\n### {number}. 🧠 Règle auto-consolidée — {title}\n* **Règle** : Cette erreur répétée a été détectée automatiquement : {desc_clean}. À chaque occurrence similaire, l'agent doit s'arrêter, identifier la cause générale, appliquer ou créer une règle globale adaptée, puis éviter de répététer la même tentative ciblée."
    )
}

pub async fn ai_consolidate_rules(
    provider: &(dyn Provider + Send + Sync),
    model_name: &str,
) -> Result<String, String> {
    let Ok(local_app_data) = std::env::var("LOCALAPPDATA") else {
        return Err("LOCALAPPDATA non disponible".to_string());
    };
    let sinew_dir = PathBuf::from(local_app_data).join("Sinew");
    let errors_path = sinew_dir.join("errors_raw.json");
    let rules_path = sinew_dir.join("instructions_consolidated.md");

    let errors_data = fs::read_to_string(&errors_path)
        .map_err(|e| format!("Impossible de lire errors_raw.json: {e}"))?;

    let errors: Vec<ErrorItem> = serde_json::from_str(&errors_data)
        .map_err(|e| format!("Format errors_raw.json invalide: {e}"))?;

    let unconsolidated: Vec<&ErrorItem> = errors
        .iter()
        .filter(|e| e.count >= 3 && !e.id.is_empty() && e.consolidated_at.is_none())
        .collect();

    if unconsolidated.is_empty() {
        return Ok("Aucune erreur à consolider (toutes les erreurs sont déjà traitées ou en dessous du seuil de 3).".to_string());
    }

    let current_rules = if rules_path.exists() {
        fs::read_to_string(&rules_path).unwrap_or_default()
    } else {
        String::from("# 🛡️ Instructions Globales Consolidées (Règles anti-erreurs répétitives)\n\nCes instructions ont été validées et consolidées après avoir été rencontrées au moins 3 fois. Tout agent intervenant sur ce projet doit les respecter à la lettre.\n")
    };

    // Build the prompt for the AI
    let errors_json = serde_json::to_string_pretty(
        &unconsolidated
            .iter()
            .map(|e| serde_json::json!({
                "id": e.id,
                "count": e.count,
                "description": e.description.as_deref().unwrap_or("Pas de description")
            }))
            .collect::<Vec<_>>(),
    )
    .unwrap_or_default();

    let system_prompt = indoc::indoc! {r#"
        Tu es un assistant d'auto-amélioration pour Sinew, un IDE agentique. Ta seule mission est d'analyser des erreurs répétitives rencontrées par l'agent et de produire ou mettre à jour le fichier de règles globales consolidées.

        CONTEXTE :
        - Le fichier "instructions_consolidated.md" contient les règles globales actuelles, injectées dans le prompt système de l'agent.
        - Le fichier "errors_raw.json" contient les erreurs brutes détectées (avec un compteur d'occurrences).
        - Quand une erreur atteint 3 occurrences, une règle est normalement créée.

        TA MISSION :
        1. Analyse les erreurs fournies ci-dessous.
        2. Identifie les doublons ou les erreurs qui partagent la MÊME CAUSE RACINE mais avec des chemins/noms différents.
        3. Fusionne les règles similaires en UNE SEULE règle plus générale (ex: "Ne jamais utiliser de chemins relatifs" au lieu d'avoir une règle par chemin).
        4. Crée une nouvelle règle pour chaque erreur vraiment distincte (avec le format standard ### N. 🧠 Règle auto-consolidée — Titre).
        5. Produit le fichier COMPLET mis à jour (règles existantes + nouvelles, dédoublonnées et numérotées proprement).

        FORMAT DE SORTIE ATTENDU :
        Tu dois retourner UNIQUEMENT le contenu complet du fichier instructions_consolidated.md mis à jour, sans commentaire ni explication avant ou après. Le fichier doit commencer par :
        "# 🛡️ Instructions Globales Consolidées (Règles anti-erreurs répétitives)"
    "#};

    let user_prompt = format!(
        "FICHIER DE RÈGLES ACTUEL :\n```markdown\n{current_rules}\n```\n\nERREURS À ANALYSER (JSON) :\n```json\n{errors_json}\n```\n\nProduis le fichier instructions_consolidated.md complet et mis à jour."
    );

    let transcript = vec![ChatMessage {
        role: Role::User,
        parts: vec![Part::Text {
            text: user_prompt,
            meta: None,
        }],
    }];

    let request = ProviderRequest {
        model: ModelRef {
            provider: "deepseek".to_string(),
            name: model_name.to_string(),
            effort: None,
        },
        system_prompt: Some(system_prompt.to_string()),
        transcript,
        tools: Vec::new(),
        max_output_tokens: Some(4096),
        effort: None,
        temperature: Some(0.3),
        cache_key: None,
        cache_stable_message_count: None,
        service_tier: None,
        workspace_root: None,
    };

    let mut stream = provider
        .stream(request)
        .await
        .map_err(|e| format!("Erreur du fournisseur IA: {e}"))?;

    let mut response_text = String::new();
    while let Some(event) = stream.next().await {
        match event {
            Ok(StreamEvent::TextDelta { delta, .. }) => {
                response_text.push_str(&delta);
            }
            Err(e) => {
                return Err(format!("Erreur pendant le streaming: {e}"));
            }
            _ => {}
        }
    }

    if response_text.trim().is_empty() {
        return Err("L'IA n'a produit aucune réponse.".to_string());
    }

    // Validate the response starts with the header
    let header = "# 🛡️ Instructions Globales Consolidées";
    let refined_rules = if response_text.contains(header) {
        let start = response_text.find(header).unwrap();
        response_text[start..].trim().to_string()
    } else {
        return Err(format!(
            "La réponse de l'IA ne contient pas l'en-tête attendu. Réponse reçue (début): {}...",
            &response_text[..response_text.len().min(200)]
        ));
    };

    // Write the refined rules
    if let Some(parent) = rules_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(&rules_path, format!("{refined_rules}\n"))
        .map_err(|e| format!("Impossible d'écrire instructions_consolidated.md: {e}"))?;

    // Mark all errors as consolidated
    let mut errors: Vec<ErrorItem> = serde_json::from_str(&errors_data).unwrap_or_default();
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let mut consolidated_count = 0;
    for error in &mut errors {
        if error.count >= 3 && !error.id.is_empty() && error.consolidated_at.is_none() {
            error.count = 0;
            error.consolidated_at = Some(now.clone());
            consolidated_count += 1;
        }
    }

    if let Ok(serialized) = serde_json::to_string_pretty(&errors) {
        let _ = fs::write(&errors_path, format!("{serialized}\n"));
    }

    let result = format!(
        "Consolidation IA terminée : {} règle(s) analysée(s), {} erreur(s) consolidée(s).",
        unconsolidated.len(),
        consolidated_count
    );
    println!("{result}");
    Ok(result)
}

pub fn consolidate_rules() {
    let Ok(local_app_data) = std::env::var("LOCALAPPDATA") else {
        return;
    };
    let sinew_dir = PathBuf::from(local_app_data).join("Sinew");
    let errors_path = sinew_dir.join("errors_raw.json");
    let rules_path = sinew_dir.join("instructions_consolidated.md");

    if !errors_path.exists() {
        return;
    }

    let Ok(errors_data) = fs::read_to_string(&errors_path) else {
        return;
    };

    let mut errors: Vec<ErrorItem> = match serde_json::from_str(&errors_data) {
        Ok(parsed) => parsed,
        Err(_) => return,
    };

    let mut rules_text = if rules_path.exists() {
        fs::read_to_string(&rules_path).unwrap_or_else(|_| {
            "# 🛡️ Instructions Globales Consolidées (Règles anti-erreurs répétitives)\n\n\
             Ces instructions ont été validées et consolidées après avoir été rencontrées au moins 3 fois. \
             Tout agent intervenant sur ce projet doit les respecter à la lettre.".to_string()
        })
    } else {
        "# 🛡️ Instructions Globales Consolidées (Règles anti-erreurs répétitives)\n\n\
         Ces instructions ont été validées et consolidées après avoir été rencontrées au moins 3 fois. \
         Tout agent intervenant sur ce projet doit les respecter à la lettre.".to_string()
    };

    let mut changed_errors = false;
    let mut changed_rules = false;
    let mut created_rules = 0;
    let mut cleaned_errors = 0;
    let mut number = next_rule_number(&rules_text);
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    for error in &mut errors {
        let count = error.count;
        if count < 3 || error.id.is_empty() {
            continue;
        }

        if rule_covers_error(&rules_text, &error.id) {
            cleaned_errors += 1;
        } else {
            let description = error.description.as_deref().unwrap_or("Erreur répétitive sans description.");
            rules_text.push_str(&build_rule(number, &error.id, description));
            number += 1;
            created_rules += 1;
            changed_rules = true;
        }

        error.count = 0;
        error.consolidated_at = Some(now.clone());
        changed_errors = true;
    }

    if changed_rules {
        if let Some(parent) = rules_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let rules_text = format!("{}\n", rules_text.trim_end());
        let _ = fs::write(&rules_path, rules_text);
    }

    if changed_errors {
        if let Ok(serialized) = serde_json::to_string_pretty(&errors) {
            let _ = fs::write(&errors_path, format!("{}\n", serialized));
        }
    }

    println!(
        "Consolidation terminée (Rust) : {} règle(s) créée(s), {} erreur(s) déjà couvertes nettoyée(s).",
        created_rules, cleaned_errors
    );
}
