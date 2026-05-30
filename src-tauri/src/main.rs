// Sur Windows, sans cet attribut, le binaire tourne en mode console et Windows
// ouvre automatiquement une fenêtre console (visible dans la barre des tâches
// à côté de l'app). En release, on force le sous-système "windows" pour éviter
// ça. En debug on garde la console pour les logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::panic;
use std::io::Write;

fn main() {
    // Capture toutes les panics Rust dans le fichier de log centralisé
    let log_dir = std::path::PathBuf::from(
        std::env::var("LOCALAPPDATA").unwrap_or_default()
    ).join("dev").join("hyrak").join("sinew").join("data").join("logs");
    let _ = std::fs::create_dir_all(&log_dir);
    let panic_log = log_dir.join("panic.log");
    panic::set_hook(Box::new(move |info| {
        let msg = format!(
            "[{}] PANIC: {}\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
            info
        );
        // Écriture synchrone pour garantir la capture même en cas de crash
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&panic_log) {
            let _ = f.write_all(msg.as_bytes());
            let _ = f.flush();
        }
        // Affiche aussi sur stderr si dispo
        eprintln!("{}", msg);
        std::process::abort();
    }));

    if sinew_desktop_lib::cli::handle_args() {
        return;
    }
    if sinew_index::run_helper_if_requested() {
        return;
    }
    sinew_desktop_lib::run()
}
