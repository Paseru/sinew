//! Boost Local — distillation automatique des grosses sorties d'outils.
//!
//! Quand le Boost Local est actif (variable d'environnement `SINEW_BOOST_DISTILLER`
//! posée par le backend), les sorties **volumineuses** de `bash` et `web_fetch`
//! sont résumées par le petit modèle local avant d'entrer dans le contexte du
//! modèle principal. On économise ainsi des milliers de jetons par tour.
//!
//! Règles de sûreté (SOTA, ne jamais casser l'agent) :
//!   * jamais sur `read`/`grep`/`codebase_search` : le modèle a besoin du texte
//!     EXACT pour éditer (correspondance `oldContent`) ;
//!   * jamais sur une sortie en erreur : le modèle doit voir l'erreur brute ;
//!   * uniquement au-dessus d'un seuil élevé (les petites sorties restent brutes) ;
//!   * la **fin** de la sortie est conservée telle quelle (codes de sortie,
//!     erreurs finales) ;
//!   * en cas d'échec/timeout du distillateur, on renvoie la sortie brute.

use std::time::Duration;

use serde_json::json;

use crate::tool_names;

const OLLAMA_URL: &str = "http://127.0.0.1:11434";
/// Seuil de déclenchement (~6 000 jetons). En dessous, aucune distillation.
const DISTILL_MIN_CHARS: usize = 24_000;
/// Nombre de caractères de fin conservés bruts (codes d'erreur, exit codes…).
const RAW_TAIL_CHARS: usize = 1_000;

/// Le distillateur local est-il actif ? (drapeau posé par le backend)
fn distiller_model() -> Option<String> {
    std::env::var("SINEW_BOOST_DISTILLER")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Seul `bash` et `web_fetch` produisent de gros textes non requis à l'identique.
fn is_distillable_tool(tool: &str) -> bool {
    tool == tool_names::BASH || tool == tool_names::BASH_INPUT || tool == tool_names::WEB_FETCH
}

/// Renvoie une version distillée du contenu pour le contexte du modèle, ou `None`
/// si rien ne doit changer (la sortie brute sera alors utilisée).
pub(super) async fn maybe_distill_tool_output(
    tool: &str,
    content: &str,
    is_error: bool,
) -> Option<String> {
    if is_error || !is_distillable_tool(tool) || content.len() < DISTILL_MIN_CHARS {
        return None;
    }
    let model = distiller_model()?;

    let orig_tokens = content.len() / 4;

    // On distille le gros du texte mais on garde la fin brute intacte.
    let split = content.len().saturating_sub(RAW_TAIL_CHARS);
    let head = &content[..split];
    let tail = &content[split..];

    let prompt = format!(
        "You are a context distiller for an AI coding agent. The agent ran a tool and got verbose \
         output. Summarize the ESSENTIAL facts it needs: key results, file paths, names, numbers, \
         warnings, and especially any errors. Be terse and factual, no filler, no code fences. \
         Max 200 words.\n\nTOOL: {tool}\n\nOUTPUT:\n{head}"
    );

    let body = json!({
        "model": model,
        "prompt": prompt,
        "stream": false,
        "keep_alive": -1,
        "options": { "num_ctx": 8192, "temperature": 0.1 }
    });

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .ok()?;

    let resp = client
        .post(format!("{OLLAMA_URL}/api/generate"))
        .json(&body)
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let value: serde_json::Value = resp.json().await.ok()?;
    let summary = value.get("response").and_then(|r| r.as_str())?.trim();
    if summary.is_empty() {
        return None;
    }

    let new_tokens = (summary.len() + tail.len()) / 4;
    let saved = if orig_tokens > 0 {
        100 - (new_tokens * 100 / orig_tokens).min(100)
    } else {
        0
    };

    Some(format!(
        "[Boost Local — sortie distillée localement : ~{orig_tokens} → ~{new_tokens} jetons (-{saved}%). \
         Sortie complète visible dans l'interface.]\n\n{summary}\n\n[Fin de sortie brute conservée :]\n{tail}"
    ))
}
