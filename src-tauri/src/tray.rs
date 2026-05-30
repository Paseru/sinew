use crate::DesktopState;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{
    menu::{Menu, MenuItemBuilder, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager, Runtime,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentWorkspace {
    pub path: String,
    pub name: String,
    pub last_opened_ms: u64,
}

fn recents_file() -> Option<PathBuf> {
    std::env::var("LOCALAPPDATA").ok().map(|appdata| {
        let dir = PathBuf::from(appdata).join("hyrak").join("sinew").join("data");
        let _ = fs::create_dir_all(&dir);
        dir.join("recent_workspaces.json")
    })
}

pub fn load_recents() -> Vec<RecentWorkspace> {
    if let Some(file) = recents_file() {
        if let Ok(content) = fs::read_to_string(&file) {
            if let Ok(recents) = serde_json::from_str::<Vec<RecentWorkspace>>(&content) {
                return recents;
            }
        }
    }
    Vec::new()
}

pub fn save_recents(recents: &[RecentWorkspace]) {
    if let Some(file) = recents_file() {
        if let Ok(content) = serde_json::to_string(recents) {
            let _ = fs::write(file, content);
        }
    }
}

pub fn record_recent(path: &str, name: &str) {
    let mut recents = load_recents();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    recents.retain(|r| r.path != path);
    recents.insert(
        0,
        RecentWorkspace {
            path: path.to_string(),
            name: name.to_string(),
            last_opened_ms: now,
        },
    );
    recents.truncate(12);

    save_recents(&recents);
}

pub fn update_tray_menu(app: &AppHandle) -> anyhow::Result<()> {
    let recents = load_recents();
    
    let new_window_item = MenuItemBuilder::with_id("new_window", "Nouvelle fenêtre").build(app)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    
    let mut recent_items = Vec::new();
    let empty_item;
    if recents.is_empty() {
        empty_item = Some(MenuItemBuilder::with_id("empty", "Aucun projet récent").enabled(false).build(app)?);
    } else {
        empty_item = None;
        for (i, recent) in recents.iter().enumerate() {
            recent_items.push(MenuItemBuilder::with_id(format!("recent_{}", i), &recent.name).build(app)?);
        }
    }
    
    let sep2 = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItemBuilder::with_id("quit", "Quitter Sinew").build(app)?;

    let mut refs: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> = Vec::new();
    refs.push(&new_window_item);
    refs.push(&sep1);
    if let Some(ref e) = empty_item {
        refs.push(e);
    } else {
        for item in &recent_items {
            refs.push(item);
        }
    }
    refs.push(&sep2);
    refs.push(&quit_item);

    let menu = Menu::with_items(app, &refs)?;

    if let Some(tray) = app.tray_by_id("main") {
        let _ = tray.set_menu(Some(menu));
    }
    Ok(())
}

pub fn setup_tray(app: &tauri::App) -> anyhow::Result<()> {
    let handle = app.handle().clone();
    let tray = TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("Sinew")
        .on_menu_event(move |app_handle, event| {
            let id = event.id.as_ref();
            if id == "new_window" {
                crate::platform::create_new_window_detached(app_handle);
            } else if id == "quit" {
                app_handle.exit(0);
            } else if id.starts_with("recent_") {
                if let Ok(idx) = id["recent_".len()..].parse::<usize>() {
                    let recents = load_recents();
                    if let Some(recent) = recents.get(idx) {
                        let mut builder = tauri::WebviewWindowBuilder::new(
                            app_handle,
                            crate::platform::next_window_label(app_handle),
                            tauri::WebviewUrl::App(std::path::PathBuf::from(format!(
                                "index.html?workspace={}",
                                url::form_urlencoded::byte_serialize(recent.path.as_bytes()).collect::<String>()
                            ))),
                        )
                        .title("Sinew")
                        .inner_size(1500.0, 940.0)
                        .min_inner_size(1100.0, 720.0)
                        .resizable(true)
                        .center();

                        #[cfg(target_os = "windows")]
                        {
                            builder = builder.decorations(false);
                        }
                        let _ = builder.build();
                    }
                }
            }
        })
        .on_tray_icon_event(|_tray, event| {
            if let tauri::tray::TrayIconEvent::Click { .. } = event {
                // Focus existing on click
            }
        })
        .build(app)?;

    let _ = update_tray_menu(app.handle());

    Ok(())
}
