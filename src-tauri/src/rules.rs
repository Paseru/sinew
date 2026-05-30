use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use regex::Regex;
use chrono::Local;

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
