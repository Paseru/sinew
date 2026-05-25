use std::{env, path::PathBuf};

const RG_ENV: &str = "CLAAKECODE_RG_PATH";

pub(crate) fn ripgrep_executable() -> PathBuf {
    env::var_os(RG_ENV)
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .or_else(find_bundled_ripgrep)
        .or_else(find_ripgrep_in_path)
        .or_else(|| existing_path("/opt/homebrew/bin/rg"))
        .or_else(|| existing_path("/usr/local/bin/rg"))
        .unwrap_or_else(|| PathBuf::from(platform_executable_name()))
}

fn find_bundled_ripgrep() -> Option<PathBuf> {
    let exe = env::current_exe().ok()?;
    let exe_dir = exe.parent()?;
    let mut roots = vec![exe_dir.to_path_buf(), exe_dir.join("resources")];

    if let Some(parent) = exe_dir.parent() {
        roots.push(parent.join("Resources"));
        roots.push(parent.join("resources"));
        if let Some(grandparent) = parent.parent() {
            roots.push(grandparent.join("Resources"));
            roots.push(grandparent.join("resources"));
        }
    }

    for root in roots {
        for name in bundled_file_names() {
            let direct = root.join(name);
            if direct.is_file() {
                return Some(direct);
            }
            let nested = root.join("binaries").join(name);
            if nested.is_file() {
                return Some(nested);
            }
        }
    }

    None
}

fn find_ripgrep_in_path() -> Option<PathBuf> {
    executable_names()
        .into_iter()
        .find_map(find_executable_in_path)
}

fn find_executable_in_path(name: &str) -> Option<PathBuf> {
    let paths = env::var_os("PATH")?;
    env::split_paths(&paths)
        .map(|path| path.join(name))
        .find(|path| path.is_file())
}

fn existing_path(path: &str) -> Option<PathBuf> {
    let path = PathBuf::from(path);
    path.is_file().then_some(path)
}

fn executable_names() -> Vec<&'static str> {
    #[cfg(windows)]
    {
        vec!["rg.exe", "rg"]
    }
    #[cfg(not(windows))]
    {
        vec!["rg"]
    }
}

fn bundled_file_names() -> Vec<&'static str> {
    let mut names = vec![platform_executable_name()];
    if let Some(sidecar) = platform_sidecar_name() {
        names.push(sidecar);
    }
    #[cfg(target_os = "macos")]
    {
        names.push("rg-universal-apple-darwin");
    }
    names
}

fn platform_executable_name() -> &'static str {
    #[cfg(windows)]
    {
        "rg.exe"
    }
    #[cfg(not(windows))]
    {
        "rg"
    }
}

fn platform_sidecar_name() -> Option<&'static str> {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        Some("rg-x86_64-pc-windows-msvc.exe")
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        Some("rg-aarch64-apple-darwin")
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        Some("rg-x86_64-apple-darwin")
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        Some("rg-x86_64-unknown-linux-gnu")
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        Some("rg-aarch64-unknown-linux-gnu")
    }
    #[cfg(not(any(
        all(windows, target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64")
    )))]
    {
        None
    }
}
