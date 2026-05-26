use crate::*;

#[cfg(not(target_os = "windows"))]
use tauri::{
    menu::{AboutMetadata, Menu, MenuItemBuilder, PredefinedMenuItem, Submenu, SubmenuBuilder},
    Runtime,
};

pub(super) fn error_to_string(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(not(target_os = "windows"))]
pub(super) fn install_desktop_menu(app: &AppHandle) -> Result<()> {
    let menu = build_desktop_menu(app)?;
    app.set_menu(menu)?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn build_desktop_menu<R: Runtime>(app: &AppHandle<R>) -> Result<Menu<R>> {
    let pkg_info = app.package_info();
    let config = app.config();
    let about_metadata = AboutMetadata {
        name: Some(pkg_info.name.clone()),
        version: Some(pkg_info.version.to_string()),
        copyright: config.bundle.copyright.clone(),
        authors: config
            .bundle
            .publisher
            .clone()
            .map(|publisher| vec![publisher]),
        ..Default::default()
    };

    let close_tab_item = MenuItemBuilder::with_id(CLOSE_ACTIVE_TAB_MENU_ID, "Close Tab")
        .accelerator("CmdOrCtrl+W")
        .build(app)?;
    let new_window_item = MenuItemBuilder::with_id(NEW_WINDOW_MENU_ID, "New Window")
        .accelerator("CmdOrCtrl+Shift+N")
        .build(app)?;

    let file_menu = SubmenuBuilder::new(app, "File")
        .item(&close_tab_item)
        .item(&new_window_item)
        .build()?;
    let edit_menu = SubmenuBuilder::new(app, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;
    let terminal_menu = SubmenuBuilder::new(app, "Terminal")
        .text(TERMINAL_OPEN_MENU_ID, "Open Terminal")
        .build()?;

    #[cfg(target_os = "macos")]
    {
        let app_menu = Submenu::with_items(
            app,
            pkg_info.name.clone(),
            true,
            &[
                &PredefinedMenuItem::about(app, None, Some(about_metadata))?,
                &PredefinedMenuItem::separator(app)?,
                &PredefinedMenuItem::services(app, None)?,
                &PredefinedMenuItem::separator(app)?,
                &PredefinedMenuItem::hide(app, None)?,
                &PredefinedMenuItem::hide_others(app, None)?,
                &PredefinedMenuItem::separator(app)?,
                &PredefinedMenuItem::quit(app, None)?,
            ],
        )?;
        let view_menu = SubmenuBuilder::new(app, "View").fullscreen().build()?;
        let window_menu = Submenu::with_id_and_items(
            app,
            tauri::menu::WINDOW_SUBMENU_ID,
            "Window",
            true,
            &[
                &PredefinedMenuItem::minimize(app, None)?,
                &PredefinedMenuItem::maximize(app, None)?,
                &PredefinedMenuItem::separator(app)?,
            ],
        )?;
        let help_menu =
            Submenu::with_id_and_items(app, tauri::menu::HELP_SUBMENU_ID, "Help", true, &[])?;
        return Menu::with_items(
            app,
            &[
                &app_menu,
                &file_menu,
                &edit_menu,
                &view_menu,
                &window_menu,
                &terminal_menu,
                &help_menu,
            ],
        )
        .map_err(Into::into);
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = about_metadata;
        return Menu::with_items(app, &[&file_menu, &edit_menu, &terminal_menu])
            .map_err(Into::into);
    }
}

pub(super) fn create_new_window(app: &AppHandle) -> Result<()> {
    let label = next_window_label(app);
    let mut builder =
        WebviewWindowBuilder::new(app, label, WebviewUrl::App(PathBuf::from(NEW_WINDOW_URL)))
            .title("Sinew")
            .inner_size(1500.0, 940.0)
            .min_inner_size(1100.0, 720.0)
            .resizable(true)
            .center();

    #[cfg(target_os = "macos")]
    {
        builder = builder
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .hidden_title(true)
            .traffic_light_position(tauri::LogicalPosition::new(14.0, 18.0));
    }

    #[cfg(target_os = "windows")]
    {
        builder = builder.decorations(false);
    }

    let window = builder.build().context("unable to create new window")?;
    let _ = window.set_focus();
    Ok(())
}

pub(super) fn create_new_window_detached(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        if let Err(err) = create_new_window(&app) {
            tracing::warn!(%err, "unable to create new window");
        }
    });
}

#[cfg(target_os = "macos")]
pub(super) fn focus_existing_window(app: &AppHandle) -> bool {
    let mut windows = app.webview_windows();
    let window = windows
        .remove("main")
        .or_else(|| windows.into_values().next());

    if let Some(window) = window {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return true;
    }

    false
}

/// Set the window title to the workspace folder name. Falls back to the
/// product name when `folder_name` is empty so the macOS Dock window list
/// never shows a blank entry.
pub(super) fn apply_window_title(window: &tauri::WebviewWindow, folder_name: &str) {
    let trimmed = folder_name.trim();
    let title = if trimmed.is_empty() { "Sinew" } else { trimmed };
    if let Err(err) = window.set_title(title) {
        tracing::warn!(%err, label = %window.label(), "unable to update window title");
    }
}

pub(super) fn next_window_label(app: &AppHandle) -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    for index in 0..1000 {
        let label = format!("{NEW_WINDOW_LABEL_PREFIX}-{millis}-{index}");
        if app.get_webview_window(&label).is_none() {
            return label;
        }
    }

    format!("{NEW_WINDOW_LABEL_PREFIX}-{millis}-fallback")
}

#[cfg(target_os = "macos")]
pub(super) fn install_macos_dock_menu(app: &AppHandle) {
    let _ = MACOS_APP_HANDLE.set(app.clone());

    let Some(mtm) = MainThreadMarker::new() else {
        tracing::warn!("unable to install macOS dock menu outside main thread");
        return;
    };

    let ns_app = NSApplication::sharedApplication(mtm);
    let delegate: *mut AnyObject = unsafe { objc2::msg_send![&*ns_app, delegate] };
    if delegate.is_null() {
        tracing::warn!("unable to install macOS dock menu without app delegate");
        return;
    }

    let delegate_class = unsafe { &*delegate }.class() as *const AnyClass as *mut AnyClass;
    unsafe {
        let dock_menu_imp = std::mem::transmute::<
            unsafe extern "C-unwind" fn(&AnyObject, Sel, *mut AnyObject) -> *mut AnyObject,
            Imp,
        >(macos_application_dock_menu);
        let new_window_imp = std::mem::transmute::<
            unsafe extern "C-unwind" fn(&AnyObject, Sel, *mut AnyObject),
            Imp,
        >(macos_new_window_from_dock);

        let _ = class_addMethod(
            delegate_class,
            objc2::sel!(applicationDockMenu:),
            dock_menu_imp,
            c"@@:@".as_ptr().cast(),
        );
        let _ = class_addMethod(
            delegate_class,
            objc2::sel!(sinewNewWindowFromDock:),
            new_window_imp,
            c"v@:@".as_ptr().cast(),
        );
    }
}

#[cfg(target_os = "macos")]
unsafe extern "C-unwind" fn macos_application_dock_menu(
    target: &AnyObject,
    _cmd: Sel,
    _sender: *mut AnyObject,
) -> *mut AnyObject {
    let Some(mtm) = MainThreadMarker::new() else {
        return std::ptr::null_mut();
    };

    let menu_title = NSString::from_str("Sinew");
    let item_title = NSString::from_str("Nouvelle fenêtre");
    let empty_key = NSString::new();
    let menu = NSMenu::initWithTitle(mtm.alloc(), &menu_title);
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &item_title,
            Some(objc2::sel!(sinewNewWindowFromDock:)),
            &empty_key,
        )
    };

    unsafe {
        item.setTarget(Some(target));
    }
    menu.addItem(&item);

    Retained::autorelease_return(menu) as *mut AnyObject
}

#[cfg(target_os = "macos")]
unsafe extern "C-unwind" fn macos_new_window_from_dock(
    _target: &AnyObject,
    _cmd: Sel,
    _sender: *mut AnyObject,
) {
    if let Some(app) = MACOS_APP_HANDLE.get() {
        create_new_window_detached(app);
    }
}

pub(super) fn emit_workspace_file_change(
    app: &AppHandle,
    workspace_root: &Path,
    relative_path: &str,
) {
    let _ = app.emit(
        FILE_CHANGE_EVENT_NAME,
        WorkspaceFileChangeEvent {
            workspace_path: workspace_root.display().to_string(),
            relative_path: relative_path.to_string(),
        },
    );
}

pub(super) fn reveal_path(path: &Path) -> Result<()> {
    if !path.exists() {
        anyhow::bail!("path does not exist");
    }

    #[cfg(target_os = "macos")]
    {
        let status = Command::new("open")
            .arg("-R")
            .arg(path)
            .status()
            .context("unable to reveal item in Finder")?;
        if !status.success() {
            anyhow::bail!("Finder reveal failed");
        }
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        let status = Command::new("explorer")
            .arg(format!("/select,{}", path.display()))
            .status()
            .context("unable to reveal item in Explorer")?;
        if !status.success() {
            anyhow::bail!("Explorer reveal failed");
        }
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let target = if path.is_dir() {
            path
        } else {
            path.parent().unwrap_or(path)
        };
        let status = Command::new("xdg-open")
            .arg(target)
            .status()
            .context("unable to open file manager")?;
        if !status.success() {
            anyhow::bail!("file manager open failed");
        }
        return Ok(());
    }

    #[allow(unreachable_code)]
    Ok(())
}

pub(super) fn delete_installed_skill(workspace_root: &Path, skill_md: &Path) -> Result<PathBuf> {
    let skill_md = fs::canonicalize(skill_md).context("skill file does not exist")?;
    if skill_md.file_name().and_then(|name| name.to_str()) != Some("SKILL.md") {
        anyhow::bail!("can only delete a SKILL.md file");
    }

    let folder = skill_md
        .parent()
        .ok_or_else(|| anyhow::anyhow!("skill has no parent folder"))?
        .to_path_buf();
    let allowed_roots = skill_roots(workspace_root)
        .into_iter()
        .filter_map(|root| fs::canonicalize(root).ok())
        .collect::<Vec<_>>();
    let allowed = allowed_roots
        .iter()
        .any(|root| folder.parent() == Some(root.as_path()));
    if !allowed {
        anyhow::bail!("skill is outside the configured skill folders");
    }

    fs::remove_dir_all(&folder)
        .with_context(|| format!("unable to delete skill folder {}", folder.display()))?;
    Ok(folder)
}

pub(super) fn write_installed_skill(
    workspace_root: &Path,
    skill_md: &Path,
    content: &str,
) -> Result<PathBuf> {
    let skill_md = fs::canonicalize(skill_md).context("skill file does not exist")?;
    if skill_md.file_name().and_then(|name| name.to_str()) != Some("SKILL.md") {
        anyhow::bail!("can only edit a SKILL.md file");
    }

    let folder = skill_md
        .parent()
        .ok_or_else(|| anyhow::anyhow!("skill has no parent folder"))?
        .to_path_buf();
    let allowed_roots = skill_roots(workspace_root)
        .into_iter()
        .filter_map(|root| fs::canonicalize(root).ok())
        .collect::<Vec<_>>();
    let allowed = allowed_roots
        .iter()
        .any(|root| folder.parent() == Some(root.as_path()));
    if !allowed {
        anyhow::bail!("skill is outside the configured skill folders");
    }

    fs::write(&skill_md, content)
        .with_context(|| format!("unable to write {}", skill_md.display()))?;
    Ok(skill_md)
}

pub(super) fn skill_roots(workspace_root: &Path) -> Vec<PathBuf> {
    let mut roots = vec![
        workspace_root.join(".agents/skills"),
        workspace_root.join(".sinew/skills"),
    ];
    if let Some(home) = home_dir() {
        roots.push(home.join(".agents/skills"));
        roots.push(home.join(".sinew/skills"));
    }
    roots
}

pub(super) fn home_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    }

    #[cfg(not(target_os = "windows"))]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

pub(super) fn open_external_url(raw_url: &str) -> Result<()> {
    let url = raw_url.trim();
    if !is_safe_external_url(url) {
        anyhow::bail!("only http and https links can be opened");
    }

    #[cfg(target_os = "macos")]
    {
        let status = Command::new("open")
            .arg(url)
            .status()
            .context("unable to open link")?;
        if !status.success() {
            anyhow::bail!("link open failed");
        }
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        let status = Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .status()
            .context("unable to open link")?;
        if !status.success() {
            anyhow::bail!("link open failed");
        }
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let status = Command::new("xdg-open")
            .arg(url)
            .status()
            .context("unable to open link")?;
        if !status.success() {
            anyhow::bail!("link open failed");
        }
        return Ok(());
    }

    #[allow(unreachable_code)]
    Ok(())
}

pub(super) fn open_with_default_app(path: &Path) -> Result<()> {
    if !path.exists() {
        anyhow::bail!("path does not exist");
    }

    #[cfg(target_os = "macos")]
    {
        let status = Command::new("open")
            .arg(path)
            .status()
            .context("unable to open file with default application")?;
        if !status.success() {
            anyhow::bail!("default application open failed");
        }
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        // `start` is a cmd builtin; the second argument is the window title.
        let status = Command::new("cmd")
            .args(["/C", "start", ""])
            .arg(path)
            .status()
            .context("unable to open file with default application")?;
        if !status.success() {
            anyhow::bail!("default application open failed");
        }
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let status = Command::new("xdg-open")
            .arg(path)
            .status()
            .context("unable to open file with default application")?;
        if !status.success() {
            anyhow::bail!("default application open failed");
        }
        return Ok(());
    }

    #[allow(unreachable_code)]
    Ok(())
}

pub(super) fn is_safe_external_url(url: &str) -> bool {
    if url.len() > 4096 || url.chars().any(char::is_control) {
        return false;
    }
    let lower = url.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

pub(super) fn is_workspace_file_event(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Any | EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    )
}

pub(super) fn should_ignore_workspace_event_path(root: &Path, path: &Path) -> bool {
    const IGNORED: &[&str] = &[
        ".git",
        "node_modules",
        "target",
        "dist",
        "build",
        ".next",
        ".turbo",
        ".cache",
        ".idea",
        "__pycache__",
        ".pytest_cache",
        ".venv",
        "venv",
        ".mypy_cache",
        "out",
    ];

    match path.strip_prefix(root) {
        Ok(relative) => relative.components().any(|component| {
            matches!(
                component,
                Component::Normal(value)
                    if IGNORED
                        .iter()
                        .any(|ignored| value.to_string_lossy() == *ignored)
            )
        }),
        Err(_) => false,
    }
}

pub(super) fn event_relative_path(root: &Path, path: &Path) -> Option<String> {
    path.strip_prefix(root).ok().map(|relative| {
        relative
            .components()
            .filter_map(|component| match component {
                Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("/")
    })
}

pub(super) fn read_clipboard_file_paths() -> Result<Vec<String>> {
    let mut paths = read_platform_clipboard_file_paths().unwrap_or_default();
    if paths.is_empty() {
        paths = read_clipboard_text_paths().unwrap_or_default();
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}

#[cfg(target_os = "macos")]
pub(super) fn read_platform_clipboard_file_paths() -> Result<Vec<String>> {
    let script = r#"
use framework "AppKit"
use scripting additions
set pasteboard to current application's NSPasteboard's generalPasteboard()
set urls to pasteboard's readObjectsForClasses:{current application's NSURL} options:(missing value)
if urls is missing value then return ""
set output to {}
repeat with itemUrl in urls
    set itemPath to (itemUrl's |path|()) as text
    if itemPath is not "" then set end of output to itemPath
end repeat
set AppleScript's text item delimiters to linefeed
return output as text
"#;
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .context("unable to read macOS clipboard")?;
    if !output.status.success() {
        return Ok(Vec::new());
    }
    Ok(parse_clipboard_paths(&String::from_utf8_lossy(
        &output.stdout,
    )))
}

#[cfg(not(target_os = "macos"))]
pub(super) fn read_platform_clipboard_file_paths() -> Result<Vec<String>> {
    Ok(Vec::new())
}

pub(super) fn read_clipboard_text_paths() -> Result<Vec<String>> {
    let output = clipboard_text_command()
        .and_then(|mut command| command.output().ok())
        .filter(|output| output.status.success());
    let Some(output) = output else {
        return Ok(Vec::new());
    };
    Ok(parse_clipboard_paths(&String::from_utf8_lossy(
        &output.stdout,
    )))
}

pub(super) fn clipboard_text_command() -> Option<Command> {
    #[cfg(target_os = "macos")]
    {
        let command = Command::new("pbpaste");
        return Some(command);
    }
    #[cfg(target_os = "windows")]
    {
        let mut command = Command::new("powershell");
        command.args(["-NoProfile", "-Command", "Get-Clipboard"]);
        return Some(command);
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let mut command = Command::new("sh");
        command.args([
            "-c",
            "wl-paste 2>/dev/null || xclip -selection clipboard -o 2>/dev/null || xsel -b -o 2>/dev/null",
        ]);
        return Some(command);
    }
    #[allow(unreachable_code)]
    None
}

pub(super) fn parse_clipboard_paths(raw: &str) -> Vec<String> {
    raw.lines()
        .filter_map(|line| {
            let value = line.trim().trim_matches('"').trim_matches('\'');
            let value = value.strip_prefix("file://").unwrap_or(value);
            if value.is_empty() {
                return None;
            }
            let path = PathBuf::from(percent_decode_path(value));
            if path.exists() {
                Some(path.display().to_string())
            } else {
                None
            }
        })
        .collect()
}

pub(super) fn percent_decode_path(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut idx = 0;
    while idx < bytes.len() {
        if bytes[idx] == b'%' && idx + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hex_value(bytes[idx + 1]), hex_value(bytes[idx + 2])) {
                decoded.push(hi * 16 + lo);
                idx += 3;
                continue;
            }
        }
        decoded.push(bytes[idx]);
        idx += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

pub(super) fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
