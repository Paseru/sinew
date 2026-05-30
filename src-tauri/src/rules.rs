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
    ChatMessage, ModelRef, Provider, ProviderRequest, StreamEvent,
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
    provider: Arc<dyn Provider>,
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

    let clean_errors_data = errors_data.strip_prefix('\u{FEFF}').unwrap_or(&errors_data);

    let errors: Vec<ErrorItem> = serde_json::from_str(clean_errors_data)
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

    let system_prompt = concat!(
        "Tu es un assistant d'auto-amélioration pour Sinew, un IDE agentique. ",
        "Ta mission : analyser les erreurs répétitives et produire un fichier de règles consolidées INTELLIGENT, sans aucune intervention humaine.\n\n",
        "CONTEXTE :\n",
        "- Le fichier instructions_consolidated.md contient les règles globales actuelles, injectées dans le prompt système de l'agent.\n",
        "- Le fichier errors_raw.json contient les erreurs brutes (avec compteur d'occurrences).\n",
        "- Seuil de création : 3 occurrences minimum.\n\n",
        "TA MISSION (6 étapes) :\n",
        "1. Analyse les erreurs fournies et les règles existantes.\n",
        "2. Identifie les doublons ou erreurs partageant la MÊME CAUSE RACINE (ex: chemins relatifs sous Windows). Fusionne-les en UNE SEULE règle générale.\n",
        "3. Pour chaque règle (existante ou nouvelle), applique le SYSTÈME DE CONFIANCE :\n",
        "   - 🟢 ACTIVE : règle issue de 3+ occurrences confirmées → doit être respectée strictement\n",
        "   - 🟡 CANDIDATE : règle issue de 2 occurrences (presque seuil) → l'agent doit y prêter attention mais peut déroger avec justification\n",
        "   - 🔴 OBSOLÈTE : règle non déclenchée depuis plus de 2 mois, ou contredite par une règle plus récente → marquée comme historique, ne plus appliquer\n",
        "4. Chaque règle doit inclure OBLIGATOIREMENT :\n",
        "   - 🏷️ Statut de confiance (ACTIVE/CANDIDATE/OBSOLÈTE)\n",
        "   - 📊 Origine : quelles erreurs ont fusionné pour créer cette règle (ids + compteurs)\n",
        "   - 📅 Date de création ou dernière mise à jour\n",
        "   - 🔗 Règles liées ou remplacées (si fusion)\n",
        "5. DÉGRADATION AUTOMATIQUE : si une règle contredit une nouvelle règle plus générale, ou si elle n'a pas été mise à jour depuis 2+ mois, passe-la en 🔴 OBSOLÈTE (ne la supprime pas, garde l'historique).\n",
        "6. Produis le fichier COMPLET mis à jour (règles existantes + nouvelles, dédoublonnées, renumérotées).\n\n",
        "FORMAT EXACT D'UNE RÈGLE :\n",
        "### N. 🧠 Règle auto-consolidée — Titre\n",
        "* **Statut** : 🟢 ACTIVE (ou 🟡 CANDIDATE, ou 🔴 OBSOLÈTE)\n",
        "* **Origine** : Fusion de X erreurs : id1 (Y occurrences), id2 (Z occurrences)...\n",
        "* **Créée le** : JJ/MM/AAAA | **Mise à jour** : JJ/MM/AAAA\n",
        "* **Règle** : Description claire et actionnable de ce que l'agent doit faire ou éviter.\n",
        "* **Remplace** : règle N, règle M (si fusion) | **Remplacée par** : règle X (si obsolète)\n\n",
        "FORMAT DE SORTIE :\n",
        "Retourne UNIQUEMENT le contenu complet du fichier instructions_consolidated.md mis à jour. ",
        "Le fichier doit commencer par : \"# 🛡️ Instructions Globales Consolidées (Règles anti-erreurs répétitives)\""
    );

    let user_prompt = format!(
        "FICHIER DE RÈGLES ACTUEL :\n```markdown\n{}\n```\n\nERREURS À ANALYSER (JSON) :\n```json\n{}\n```\n\nProduis le fichier instructions_consolidated.md complet et mis à jour.",
        current_rules, errors_json
    );

    let request = ProviderRequest {
        model: ModelRef::new("deepseek", model_name),
        system_prompt: Some(system_prompt.to_string()),
        transcript: vec![ChatMessage::user_text(user_prompt)],
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

    // Filet de sécurité : refuser une réécriture anormalement courte (= règles
    // perdues / réponse tronquée), même si l'en-tête est présent. Une fusion
    // légitime peut raccourcir le fichier, mais pas le diviser par deux.
    let current_len = current_rules.trim().len();
    if current_len > 200 && refined_rules.len() < current_len / 2 {
        return Err(format!(
            "Réécriture refusée par sécurité : le résultat de l'IA est anormalement court \
             ({} caractères contre {} actuellement). Le fichier n'a PAS été modifié pour éviter \
             de perdre des règles.",
            refined_rules.len(),
            current_len
        ));
    }

    if let Some(parent) = rules_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    // Sauvegarde de la version précédente avant tout écrasement.
    if rules_path.exists() {
        let backup_path = rules_path.with_extension("bak.md");
        let _ = fs::copy(&rules_path, &backup_path);
    }

    fs::write(&rules_path, format!("{refined_rules}\n"))
        .map_err(|e| format!("Impossible d'écrire instructions_consolidated.md: {e}"))?;

    let clean_errors_data = errors_data.strip_prefix('\u{FEFF}').unwrap_or(&errors_data);
    let mut errors: Vec<ErrorItem> = serde_json::from_str(clean_errors_data).unwrap_or_default();
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
        "Consolidation IA terminée : {} erreur(s) analysée(s), {} erreur(s) consolidée(s).",
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

    let clean_errors_data = errors_data.strip_prefix('\u{FEFF}').unwrap_or(&errors_data);

    let mut errors: Vec<ErrorItem> = match serde_json::from_str(clean_errors_data) {
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
