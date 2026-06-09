use std::{
    env,
    io::Cursor,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{bail, Context, Result};
use directories::ProjectDirs;
use serde::{de::DeserializeOwned, Deserialize};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;

const PWSH_ENV: &str = "SINEW_PWSH_PATH";
const POWERSHELL_RELEASE_API: &str =
    "https://api.github.com/repos/PowerShell/PowerShell/releases/latest";
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(180);
const CREATE_NO_WINDOW: u32 = 0x08000000;

static INSTALL_LOCK: std::sync::OnceLock<Mutex<()>> = std::sync::OnceLock::new();
static RESOLVED_EXECUTABLE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

pub fn find_powershell_7_executable() -> Option<PathBuf> {
    if let Some(path) = RESOLVED_EXECUTABLE.get() {
        return Some(path.clone());
    }

    let path = env::var_os(PWSH_ENV)
        .map(PathBuf::from)
        .filter(|path| is_powershell_7_or_newer(path))
        .or_else(find_known_powershell_7)
        .or_else(find_powershell_7_in_path)
        .or_else(find_cached_powershell_7)?;
    remember_powershell(path)
}

pub async fn ensure_powershell_7_executable() -> Result<PathBuf> {
    if let Some(path) = find_powershell_7_executable() {
        return Ok(path);
    }

    let lock = INSTALL_LOCK.get_or_init(|| Mutex::new(()));
    let _guard = lock.lock().await;

    if let Some(path) = find_powershell_7_executable() {
        return Ok(path);
    }

    tracing::info!("PowerShell 7+ not found; installing official PowerShell runtime");
    let path = install_runtime_powershell().await?;
    remember_powershell(path).context("unable to cache resolved PowerShell executable")
}

fn remember_powershell(path: PathBuf) -> Option<PathBuf> {
    let _ = RESOLVED_EXECUTABLE.set(path.clone());
    Some(path)
}

fn find_known_powershell_7() -> Option<PathBuf> {
    known_powershell_paths()
        .into_iter()
        .find(|path| is_powershell_7_or_newer(path))
}

fn known_powershell_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for var in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(root) = env::var_os(var).map(PathBuf::from) {
            paths.push(root.join("PowerShell").join("7").join("pwsh.exe"));
        }
    }
    if let Some(root) = env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        paths.push(
            root.join("Microsoft")
                .join("PowerShell")
                .join("7")
                .join("pwsh.exe"),
        );
    }
    paths
}

fn find_powershell_7_in_path() -> Option<PathBuf> {
    let paths = env::var_os("PATH")?;
    env::split_paths(&paths)
        .map(|path| path.join("pwsh.exe"))
        .find(|path| is_powershell_7_or_newer(path))
}
fn find_cached_powershell_7() -> Option<PathBuf> {
    let root = runtime_powershell_root().ok()?;
    let entries = std::fs::read_dir(root).ok()?;
    let mut candidates = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path().join("pwsh.exe");
        if is_powershell_7_or_newer(&path) {
            candidates.push(path);
        }
    }

    candidates.sort();
    candidates.pop()
}

fn is_powershell_7_or_newer(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }

    let output = Command::new(path)
        .arg("-NoLogo")
        .arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-Command")
        .arg("$PSVersionTable.PSVersion.Major")
        .env("POWERSHELL_TELEMETRY_OPTOUT", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    let Ok(output) = output else {
        return false;
    };
    if !output.status.success() {
        return false;
    }

    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .find_map(|token| token.parse::<u32>().ok())
        .is_some_and(|major| major >= 7)
}

async fn install_runtime_powershell() -> Result<PathBuf> {
    let release: GitHubRelease = download_json(POWERSHELL_RELEASE_API)
        .await
        .context("unable to fetch latest PowerShell release metadata")?;
    let package = PowerShellPackage::from_release(release)?;
    install_release_package(package).await
}

async fn install_release_package(package: PowerShellPackage) -> Result<PathBuf> {
    let cache_root = runtime_powershell_root()?;
    tokio::fs::create_dir_all(&cache_root)
        .await
        .with_context(|| {
            format!(
                "unable to create PowerShell runtime cache at {}",
                cache_root.display()
            )
        })?;

    let install_dir = cache_root.join(package.cache_dir_name());
    let final_path = install_dir.join("pwsh.exe");
    if is_powershell_7_or_newer(&final_path) {
        return Ok(final_path);
    }

    let temp_dir = cache_root.join(format!(
        ".download-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    tokio::fs::create_dir_all(&temp_dir)
        .await
        .with_context(|| {
            format!(
                "unable to create temporary PowerShell directory at {}",
                temp_dir.display()
            )
        })?;

    let result = async {
        let hashes = download_text(&package.hashes.browser_download_url)
            .await
            .with_context(|| {
                format!(
                    "unable to download PowerShell checksum manifest from {}",
                    package.hashes.browser_download_url
                )
            })?;
        let expected_hash = sha256_for_asset(&hashes, &package.archive.name)?;
        let archive_bytes = download_bytes(&package.archive.browser_download_url)
            .await
            .with_context(|| {
                format!(
                    "unable to download PowerShell from {}",
                    package.archive.browser_download_url
                )
            })?;
        verify_sha256(&archive_bytes, &expected_hash)
            .with_context(|| format!("checksum mismatch for {}", package.archive.name))?;

        extract_powershell_zip(archive_bytes, temp_dir.clone())
            .await
            .context("unable to extract PowerShell runtime")?;

        let extracted_path = temp_dir.join("pwsh.exe");
        if !is_powershell_7_or_newer(&extracted_path) {
            bail!(
                "downloaded PowerShell runtime did not provide a working PowerShell 7+ executable"
            );
        }

        if install_dir.exists() {
            let _ = tokio::fs::remove_dir_all(&install_dir).await;
        }
        match tokio::fs::rename(&temp_dir, &install_dir).await {
            Ok(()) => {}
            Err(_) if is_powershell_7_or_newer(&final_path) => return Ok(final_path),
            Err(err) => {
                return Err(err).with_context(|| {
                    format!(
                        "unable to install PowerShell runtime at {}",
                        install_dir.display()
                    )
                });
            }
        }

        if is_powershell_7_or_newer(&final_path) {
            Ok(final_path)
        } else {
            bail!(
                "installed PowerShell runtime is not usable at {}",
                final_path.display()
            )
        }
    }
    .await;

    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    result
}

async fn extract_powershell_zip(archive_bytes: Vec<u8>, destination: PathBuf) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        let reader = Cursor::new(archive_bytes);
        let mut archive = zip::ZipArchive::new(reader).context("unable to read PowerShell zip")?;

        for index in 0..archive.len() {
            let mut file = archive
                .by_index(index)
                .with_context(|| format!("unable to read PowerShell zip entry {index}"))?;
            let Some(entry_path) = file.enclosed_name() else {
                continue;
            };
            let output_path = destination.join(entry_path);

            if file.is_dir() {
                std::fs::create_dir_all(&output_path)
                    .with_context(|| format!("unable to create {}", output_path.display()))?;
                continue;
            }

            if let Some(parent) = output_path.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("unable to create {}", parent.display()))?;
            }

            let mut output = std::fs::File::create(&output_path)
                .with_context(|| format!("unable to create {}", output_path.display()))?;
            std::io::copy(&mut file, &mut output)
                .with_context(|| format!("unable to extract {}", output_path.display()))?;
        }

        let pwsh = destination.join("pwsh.exe");
        if !pwsh.is_file() {
            bail!("PowerShell zip did not contain pwsh.exe");
        }
        Ok(())
    })
    .await
    .context("PowerShell extraction task failed")?
}

async fn download_json<T: DeserializeOwned>(url: &str) -> Result<T> {
    let bytes = download_bytes(url).await?;
    serde_json::from_slice(&bytes).with_context(|| format!("response from {url} was not JSON"))
}

async fn download_text(url: &str) -> Result<String> {
    let bytes = download_bytes(url).await?;
    String::from_utf8(bytes).with_context(|| format!("response from {url} was not UTF-8"))
}

async fn download_bytes(url: &str) -> Result<Vec<u8>> {
    let client = reqwest::Client::builder()
        .timeout(DOWNLOAD_TIMEOUT)
        .build()
        .context("unable to create HTTP client")?;
    let response = client
        .get(url)
        .header(reqwest::header::USER_AGENT, "Sinew")
        .send()
        .await
        .with_context(|| format!("request failed for {url}"))?;
    let status = response.status();
    if !status.is_success() {
        bail!("request failed for {url}: {status}");
    }
    Ok(response
        .bytes()
        .await
        .with_context(|| format!("unable to read response body from {url}"))?
        .to_vec())
}

fn runtime_powershell_root() -> Result<PathBuf> {
    let dirs = ProjectDirs::from("dev", "hyrak", "sinew")
        .context("unable to resolve local cache directory")?;
    Ok(dirs.cache_dir().join("powershell"))
}

fn sha256_for_asset(hashes: &str, asset_name: &str) -> Result<String> {
    for line in hashes.lines() {
        let mut parts = line.split_whitespace();
        let Some(hash) = parts.next() else {
            continue;
        };
        let Some(name) = parts.next() else {
            continue;
        };
        let name = name.trim_start_matches("./").trim_start_matches('*');
        if name.eq_ignore_ascii_case(asset_name) {
            return Ok(hash.to_string());
        }
    }

    bail!("PowerShell checksum manifest did not contain {asset_name}")
}

fn verify_sha256(bytes: &[u8], expected: &str) -> Result<()> {
    let actual = format!("{:x}", Sha256::digest(bytes));
    if !actual.eq_ignore_ascii_case(expected) {
        bail!("expected {expected}, got {actual}");
    }
    Ok(())
}

fn current_arch_suffix() -> Result<(&'static str, &'static str)> {
    match env::consts::ARCH {
        "x86_64" => Ok(("win-x64.zip", "win-x64")),
        "aarch64" => Ok(("win-arm64.zip", "win-arm64")),
        "x86" => Ok(("win-x86.zip", "win-x86")),
        other => bail!("automatic PowerShell 7+ install is not available for Windows/{other}"),
    }
}

fn sanitize_path_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    assets: Vec<GitHubAsset>,
}

#[derive(Clone, Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

struct PowerShellPackage {
    tag_name: String,
    arch_label: &'static str,
    archive: GitHubAsset,
    hashes: GitHubAsset,
}

impl PowerShellPackage {
    fn from_release(release: GitHubRelease) -> Result<Self> {
        let (archive_suffix, arch_label) = current_arch_suffix()?;
        let archive_suffix = archive_suffix.to_ascii_lowercase();

        let archive = release
            .assets
            .iter()
            .find(|asset| asset.name.to_ascii_lowercase().ends_with(&archive_suffix))
            .cloned()
            .with_context(|| {
                format!(
                    "latest PowerShell release {} does not include a {} archive",
                    release.tag_name, archive_suffix
                )
            })?;
        let hashes = release
            .assets
            .iter()
            .find(|asset| asset.name.eq_ignore_ascii_case("hashes.sha256"))
            .cloned()
            .context("latest PowerShell release does not include hashes.sha256")?;

        Ok(Self {
            tag_name: release.tag_name,
            arch_label,
            archive,
            hashes,
        })
    }

    fn cache_dir_name(&self) -> String {
        format!(
            "{}-{}",
            sanitize_path_component(&self.tag_name),
            self.arch_label
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_manifest_finds_asset_case_insensitively() {
        let hashes = "ABCDEF  PowerShell-7.5.4-win-x64.zip\n123456  other.zip\n";
        let hash = sha256_for_asset(hashes, "powershell-7.5.4-win-x64.zip")
            .expect("asset hash should be found");
        assert_eq!(hash, "ABCDEF");
    }

    #[test]
    fn sanitize_cache_path_component_removes_separators() {
        assert_eq!(sanitize_path_component("v7.5.4/evil"), "v7.5.4_evil");
    }
}
