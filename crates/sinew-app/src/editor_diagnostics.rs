use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorDiagnostic {
    pub path: String,
    pub line: u32,
    pub column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub severity: String,
    pub message: String,
    #[serde(default)]
    pub source: String,
}

#[derive(Debug, Default)]
pub struct EditorDiagnosticsStore {
    pub updated_at_ms: i64,
    pub diagnostics: Vec<EditorDiagnostic>,
}

impl EditorDiagnosticsStore {
    pub fn replace(&mut self, diagnostics: Vec<EditorDiagnostic>) {
        self.updated_at_ms = current_time_ms();
        self.diagnostics = diagnostics;
    }

    pub fn matching<'a>(
        &'a self,
        paths: Option<&[String]>,
    ) -> impl Iterator<Item = &'a EditorDiagnostic> {
        let paths = paths.map(normalize_path_set);
        self.diagnostics.iter().filter(move |diag| {
            paths
                .as_ref()
                .map(|set| set.contains(&normalize_path(&diag.path)))
                .unwrap_or(true)
        })
    }
}

pub type SharedEditorDiagnosticsStore = Arc<RwLock<EditorDiagnosticsStore>>;

pub fn new_editor_diagnostics_store() -> SharedEditorDiagnosticsStore {
    Arc::new(RwLock::new(EditorDiagnosticsStore::default()))
}

fn normalize_path_set(paths: &[String]) -> std::collections::HashSet<String> {
    paths.iter().map(|path| normalize_path(path)).collect()
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_string()
}

fn current_time_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}
