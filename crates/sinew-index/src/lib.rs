mod background;
mod chunk;
mod embeddings;
mod indexer;
mod process;
mod search;
mod store;

pub use background::{start_background_indexing, warm_workspace_index};
pub use indexer::{ensure_workspace_index, index_stats, sync_changed_paths, IndexStats};
pub use process::{
    ensure_workspace_index_isolated, index_and_search_workspace_isolated, index_stats_isolated,
    process_isolation_enabled, run_helper_if_requested,
};
pub fn semantic_search_enabled() -> bool {
    embeddings::is_available()
}

pub use embeddings::is_available as embeddings_available;
pub use search::{search_workspace, CodebaseHit};

const SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    ".turbo",
    "__pycache__",
    ".sinew",
];

const TEXT_EXTENSIONS: &[&str] = &[
    "rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "kt", "cs", "cpp", "c", "h", "hpp", "md",
    "txt", "json", "yaml", "yml", "toml", "sql", "html", "css", "scss", "vue", "svelte", "sh",
    "ps1", "rb", "swift", "dart", "lua", "zig", "xml", "ini", "cfg", "env",
];
