mod chunk;
mod indexer;
mod search;
mod store;

pub use indexer::{ensure_workspace_index, index_stats, IndexStats};
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
    "rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "kt", "cs", "cpp", "c", "h", "hpp",
    "md", "txt", "json", "yaml", "yml", "toml", "sql", "html", "css", "scss", "vue", "svelte",
    "sh", "ps1", "rb", "swift", "dart", "lua", "zig", "xml", "ini", "cfg", "env",
];
