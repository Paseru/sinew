// Sur Windows, sans cet attribut, le binaire tourne en mode console et Windows
// ouvre automatiquement une fenêtre console (visible dans la barre des tâches
// à côté de l'app). En release, on force le sous-système "windows" pour éviter
// ça. En debug on garde la console pour les logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Avant tout, sur Windows, on s'assure d'avoir une console (cachée) attachée
    // au process. Cela évite que PowerShell.exe (spawné via ConPTY par l'outil
    // shell) plante au démarrage avec STATUS_DLL_INIT_FAILED (0xc0000142):
    // certaines DLLs de PowerShell (notamment Microsoft.PowerShell.ConsoleHost)
    // refusent de s'initialiser quand le process parent est GUI sans console.
    #[cfg(target_os = "windows")]
    ensure_hidden_console_for_child_processes();

    sinew_desktop_lib::run()
}

#[cfg(target_os = "windows")]
fn ensure_hidden_console_for_child_processes() {
    use winapi::um::consoleapi::AllocConsole;
    use winapi::um::wincon::GetConsoleWindow;
    use winapi::um::winuser::{ShowWindow, SW_HIDE};

    // SAFETY: appels Win32 sans aliasing, exécutés une seule fois au démarrage
    // depuis le thread main, avant tout autre code qui pourrait toucher à la
    // console.
    unsafe {
        // Si on a déjà une console attachée (par ex. lancé depuis un terminal
        // en mode debug), on ne fait rien — on garde la console parent.
        if !GetConsoleWindow().is_null() {
            return;
        }

        // Sinon, on alloue une nouvelle console pour ce process. AllocConsole
        // crée brièvement une fenêtre conhost, qu'on cache immédiatement. Le
        // flicker est sub-frame en pratique sur Windows 10/11.
        if AllocConsole() == 0 {
            return;
        }

        let hwnd = GetConsoleWindow();
        if !hwnd.is_null() {
            ShowWindow(hwnd, SW_HIDE);
        }
    }
}
