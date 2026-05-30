//! Détection automatique des boucles d'outils (le maillon "Capture" du système
//! d'auto-amélioration). Quand l'agent rejoue la même commande / le même appel
//! d'outil sans progresser, on injecte un rappel ciblé, puis on enregistre
//! l'incident dans `errors_raw.json` et on coupe la boucle. La consolidation
//! (`rules.rs`) transformera ensuite ces incidents répétés en règles globales.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use chrono::Local;
use serde_json::Value;

use crate::tool_names;

/// Nombre de répétitions identiques avant d'injecter un rappel fort.
const WARN_THRESHOLD: u32 = 3;
/// Nombre de répétitions identiques avant d'enregistrer l'erreur et de couper.
const BREAK_THRESHOLD: u32 = 4;
/// Longueur maximale du détail de commande conservé dans la signature/description.
const DETAIL_MAX: usize = 160;

/// Incident de boucle détecté, prêt à être enregistré et à arrêter le tour.
pub struct RepeatHit {
    pub id: String,
    pub description: String,
    pub count: u32,
}

#[derive(Default)]
pub struct RepeatGuard {
    counts: HashMap<String, u32>,
    warned: HashSet<String>,
    pending_reminder: Option<String>,
    break_requested: Option<RepeatHit>,
}

impl RepeatGuard {
    pub fn new() -> Self {
        Self::default()
    }

    /// Observe le résultat d'un appel d'outil et met à jour les compteurs.
    /// On ne suit que les actions à risque de boucle : commandes shell (`bash`)
    /// et tout appel d'outil qui termine en erreur.
    pub fn observe(&mut self, canonical_name: &str, input: &Value, is_error: bool) {
        let is_bash = canonical_name == tool_names::BASH;
        if !is_bash && !is_error {
            return;
        }

        let detail = signature_detail(canonical_name, input);
        if detail.is_empty() {
            return;
        }
        let kind = if is_error { "err" } else { "cmd" };
        let signature = format!("{kind}:{canonical_name}:{detail}");

        let count = self.counts.entry(signature.clone()).or_insert(0);
        *count += 1;
        let count = *count;

        if count >= WARN_THRESHOLD && self.warned.insert(signature.clone()) {
            self.pending_reminder = Some(format!(
                "\n\n<system_reminder>\n\
                 BOUCLE DÉTECTÉE : vous venez de répéter {count} fois la même action sans progresser :\n\
                 « {short} »\n\
                 ARRÊTEZ de relancer cette commande/outil à l'identique. Identifiez la cause racine, \
                 changez d'approche, ou demandez une précision si vous êtes bloqué. \
                 Répéter encore sera enregistré comme une erreur et interrompra le tour.\n\
                 </system_reminder>",
                count = count,
                short = truncate(&human_detail(canonical_name, input), DETAIL_MAX),
            ));
        }

        if count >= BREAK_THRESHOLD && self.break_requested.is_none() {
            let human = human_detail(canonical_name, input);
            self.break_requested = Some(RepeatHit {
                id: error_id(canonical_name, &detail),
                description: format!(
                    "L'agent a répété {count} fois la même action sans progresser : `{detail}`. \
                     Détecté automatiquement (boucle d'outil {canonical_name}).",
                    count = count,
                    detail = truncate(&human, DETAIL_MAX),
                    canonical_name = canonical_name,
                ),
                count,
            });
        }
    }

    /// Renvoie (et consomme) le rappel à injecter dans le prompt système.
    pub fn take_reminder(&mut self) -> Option<String> {
        self.pending_reminder.take()
    }

    /// Renvoie (et consomme) une demande d'arrêt si une boucle est avérée.
    pub fn take_break(&mut self) -> Option<RepeatHit> {
        self.break_requested.take()
    }
}

/// Détail normalisé utilisé pour la signature (insensible à la casse/espaces).
fn signature_detail(canonical_name: &str, input: &Value) -> String {
    let raw = human_detail(canonical_name, input);
    let lowered = raw.to_lowercase();
    let collapsed: String = lowered.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate(&collapsed, DETAIL_MAX)
}

/// Détail lisible : pour bash, la commande ; sinon, l'input compact.
fn human_detail(canonical_name: &str, input: &Value) -> String {
    if canonical_name == tool_names::BASH {
        if let Some(cmd) = input.get("command").and_then(Value::as_str) {
            return cmd.trim().to_string();
        }
    }
    match input {
        Value::Object(_) | Value::Array(_) => {
            serde_json::to_string(input).unwrap_or_default()
        }
        Value::String(s) => s.trim().to_string(),
        other => other.to_string(),
    }
}

/// Génère un identifiant stable pour `errors_raw.json` à partir de la signature.
fn error_id(canonical_name: &str, detail: &str) -> String {
    let token: String = detail
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .take(3)
        .collect::<Vec<_>>()
        .join("_");
    let token = if token.is_empty() {
        "call".to_string()
    } else {
        token.to_lowercase()
    };
    format!("repeated_{canonical_name}_{token}")
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max).collect();
    out.push('…');
    out
}

/// Enregistre (ou incrémente) un incident de boucle dans `errors_raw.json`.
/// Crée le fichier au besoin. Tolérant aux erreurs : ne panique jamais.
pub fn record_repeated_error(hit: &RepeatHit) {
    let Ok(local_app_data) = std::env::var("LOCALAPPDATA") else {
        return;
    };
    let sinew_dir = PathBuf::from(local_app_data).join("Sinew");
    let _ = fs::create_dir_all(&sinew_dir);
    let errors_path = sinew_dir.join("errors_raw.json");

    let mut errors: Vec<Value> = match fs::read_to_string(&errors_path) {
        Ok(data) => {
            let clean = data.strip_prefix('\u{FEFF}').unwrap_or(&data);
            serde_json::from_str(clean).unwrap_or_default()
        }
        Err(_) => Vec::new(),
    };

    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let mut found = false;
    for entry in errors.iter_mut() {
        if entry.get("id").and_then(Value::as_str) == Some(hit.id.as_str()) {
            let count = entry.get("count").and_then(Value::as_i64).unwrap_or(0) + 1;
            if let Value::Object(map) = entry {
                map.insert("count".into(), Value::from(count));
                map.insert("description".into(), Value::from(hit.description.clone()));
                map.insert("last_occurrence".into(), Value::from(now.clone()));
                // Une nouvelle occurrence rouvre l'erreur pour reconsolidation.
                map.insert("consolidated_at".into(), Value::Null);
            }
            found = true;
            break;
        }
    }

    if !found {
        errors.push(serde_json::json!({
            "id": hit.id,
            "description": hit.description,
            "count": 1,
            "last_occurrence": now,
        }));
    }

    if let Ok(serialized) = serde_json::to_string_pretty(&errors) {
        let _ = fs::write(&errors_path, format!("{serialized}\n"));
    }
}
