// Sur Windows, sans cet attribut, le binaire tourne en mode console et Windows
// ouvre automatiquement une fenêtre console (visible dans la barre des tâches
// à côté de l'app). En release, on force le sous-système "windows" pour éviter
// ça. En debug on garde la console pour les logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    claakecode_desktop_lib::run()
}
