//! Boost Local : un assistant local resident qui sert toutes les IA.
//!
//! Quand il est actif :
//!   * la recherche semantique vectorielle est activee (le "bibliothecaire") ;
//!   * un petit modele distillateur est charge dans Ollama et garde en memoire
//!     (`keep_alive = -1`) pour toute la session de codage ;
//!   * la commande `boost_local_distill` compresse n'importe quel gros texte
//!     (logs, fichiers, sorties d'outils) en quelques faits utiles.
//!
//! Objectif : donner aux IA (agent principal, equipe, autres modeles) exactement
//! ce dont elles ont besoin, pour limiter les jetons et accelerer la recherche.

use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use serde_json::json;

const OLLAMA_URL: &str = "http://127.0.0.1:11434";
const DEFAULT_DISTILLER: &str = "qwen2.5:3b";

struct BoostState {
    active: bool,
    distiller: String,
}

static BOOST: Mutex<BoostState> = Mutex::new(BoostState {
    active: false,
    distiller: String::new(),
});

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoostStatus {
    /// Le serveur Ollama repond.
    pub ollama_running: bool,
    /// Le distillateur est charge en memoire (visible dans /api/ps).
    pub distiller_loaded: bool,
    /// Nom du modele distillateur courant.
    pub distiller_model: String,
    /// La recherche semantique vectorielle est active.
    pub semantic_enabled: bool,
    /// Le boost a ete demarre dans cette session.
    pub active: bool,
}

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(600))
        .build()
        .unwrap_or_default()
}

fn current_distiller() -> String {
    let guard = BOOST.lock().ok();
    let name = guard
        .as_ref()
        .map(|s| s.distiller.clone())
        .unwrap_or_default();
    if name.is_empty() {
        DEFAULT_DISTILLER.to_string()
    } else {
        name
    }
}

fn semantic_enabled() -> bool {
    std::env::var_os("SINEW_INDEX_EMBEDDINGS").is_some()
}

/// Localise l'executable Ollama (PATH puis emplacement par defaut Windows).
fn ollama_binary() -> String {
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        let candidate = format!("{}\\Programs\\Ollama\\ollama.exe", local);
        if std::path::Path::new(&candidate).exists() {
            return candidate;
        }
    }
    "ollama".to_string()
}

/// Demarre `ollama serve` en arriere-plan, sans fenetre, s'il n'est pas deja lance.
fn spawn_ollama_server() {
    let bin = ollama_binary();
    let mut cmd = Command::new(bin);
    cmd.arg("serve")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        cmd.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS);
    }
    let _ = cmd.spawn();
}

async fn ollama_running(client: &reqwest::Client) -> bool {
    client
        .get(format!("{OLLAMA_URL}/api/version"))
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

/// Le modele est-il actuellement charge en memoire ?
async fn distiller_loaded(client: &reqwest::Client, model: &str) -> bool {
    let Ok(resp) = client
        .get(format!("{OLLAMA_URL}/api/ps"))
        .timeout(Duration::from_secs(3))
        .send()
        .await
    else {
        return false;
    };
    let Ok(value) = resp.json::<serde_json::Value>().await else {
        return false;
    };
    value
        .get("models")
        .and_then(|m| m.as_array())
        .map(|arr| {
            arr.iter().any(|m| {
                m.get("name")
                    .and_then(|n| n.as_str())
                    .map(|n| n == model || n.starts_with(&format!("{model}:")) || model.starts_with(n))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

async fn build_status(client: &reqwest::Client) -> BoostStatus {
    let model = current_distiller();
    let running = ollama_running(client).await;
    let loaded = if running {
        distiller_loaded(client, &model).await
    } else {
        false
    };
    let active = BOOST.lock().map(|s| s.active).unwrap_or(false);
    BoostStatus {
        ollama_running: running,
        distiller_loaded: loaded,
        distiller_model: model,
        semantic_enabled: semantic_enabled(),
        active,
    }
}

/// Etat courant du Boost Local.
#[tauri::command]
pub async fn boost_local_status() -> Result<BoostStatus, String> {
    let client = http_client();
    Ok(build_status(&client).await)
}

/// Active le Boost Local : serveur Ollama + distillateur resident + semantique.
#[tauri::command]
pub async fn boost_local_start(distiller: Option<String>) -> Result<BoostStatus, String> {
    let model = distiller
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_DISTILLER.to_string());

    let client = http_client();

    // 1) S'assurer que le serveur Ollama tourne (le demarrer sinon).
    if !ollama_running(&client).await {
        spawn_ollama_server();
        for _ in 0..20 {
            tokio::time::sleep(Duration::from_millis(500)).await;
            if ollama_running(&client).await {
                break;
            }
        }
    }
    if !ollama_running(&client).await {
        return Err("Impossible de démarrer le serveur Ollama. Vérifiez qu'Ollama est installé.".into());
    }

    // 2) Charger le distillateur et le garder en memoire pour la session.
    let load = client
        .post(format!("{OLLAMA_URL}/api/generate"))
        .json(&json!({
            "model": model,
            "prompt": "",
            "stream": false,
            "keep_alive": -1
        }))
        .send()
        .await
        .map_err(|e| format!("Échec du chargement du distillateur : {e}"))?;

    if !load.status().is_success() {
        let body = load.text().await.unwrap_or_default();
        return Err(format!(
            "Le modèle « {model} » n'est pas disponible. Lancez : ollama pull {model}\n{body}"
        ));
    }

    // 3) Activer la recherche semantique vectorielle.
    std::env::set_var("SINEW_INDEX_EMBEDDINGS", "1");

    // 4) Memoriser l'etat.
    if let Ok(mut guard) = BOOST.lock() {
        guard.active = true;
        guard.distiller = model.clone();
    }

    Ok(build_status(&client).await)
}

/// Desactive le Boost Local : libere le distillateur et coupe la semantique.
#[tauri::command]
pub async fn boost_local_stop() -> Result<BoostStatus, String> {
    let model = current_distiller();
    let client = http_client();

    if ollama_running(&client).await {
        // keep_alive = 0 => decharge immediatement le modele de la memoire.
        let _ = client
            .post(format!("{OLLAMA_URL}/api/generate"))
            .json(&json!({ "model": model, "prompt": "", "stream": false, "keep_alive": 0 }))
            .send()
            .await;
    }

    std::env::remove_var("SINEW_INDEX_EMBEDDINGS");
    if let Ok(mut guard) = BOOST.lock() {
        guard.active = false;
    }

    Ok(build_status(&client).await)
}

/// Distille un gros texte (log, fichier, sortie d'outil) en faits utiles.
/// C'est la fonction que les IA appellent pour economiser des jetons.
#[tauri::command]
pub async fn boost_local_distill(
    text: String,
    question: Option<String>,
) -> Result<String, String> {
    if text.trim().is_empty() {
        return Ok(String::new());
    }
    let model = current_distiller();
    let client = http_client();

    if !ollama_running(&client).await {
        return Err("Boost Local inactif : le serveur Ollama ne répond pas.".into());
    }

    let q = question
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "Summarize the essential facts an AI coding agent needs.".to_string());

    let prompt = format!(
        "You are a context distiller for an AI coding agent. Answer using ONLY the content below. \
         Be terse and factual: names, keys, paths, signatures. No filler, no code fences. Max 180 words.\n\n\
         TASK: {q}\n\nCONTENT:\n{text}"
    );

    let resp = client
        .post(format!("{OLLAMA_URL}/api/generate"))
        .json(&json!({
            "model": model,
            "prompt": prompt,
            "stream": false,
            "keep_alive": -1,
            "options": { "num_ctx": 8192, "temperature": 0.1 }
        }))
        .send()
        .await
        .map_err(|e| format!("Échec de la distillation : {e}"))?;

    let value: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Réponse invalide du distillateur : {e}"))?;

    Ok(value
        .get("response")
        .and_then(|r| r.as_str())
        .unwrap_or_default()
        .trim()
        .to_string())
}
