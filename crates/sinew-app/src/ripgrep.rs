use std::{
    env,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{bail, Context, Result};
use directories::ProjectDirs;
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;

const RG_ENV: &str = "SINEW_RG_PATH";
const RG_VERSION: &str = "14.1.1";
const RG_RELEASE_BASE: &str = "https://github.com/BurntSushi/ripgrep/releases/download";
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(60);

static DOWNLOAD_LOCK: std::sync::OnceLock<Mutex<()>> = std::sync::OnceLock::new();

pub(crate) fn ripgrep_executable() -> PathBuf {
    find_existing_ripgrep().unwrap_or_else(|| PathBuf::from(platform_executable_name()))
}

pub(crate) async fn ensure_ripgrep_executable() -> Result<PathBuf> {
    if let Some(path) = find_existing_ripgrep() {
        return Ok(path);
    }

    let lock = DOWNLOAD_LOCK.get_or_init(|| Mutex::new(()));
    let _guard = lock.lock().await;

    if let Some(path) = find_existing_ripgrep() {
        return Ok(path);
    }

    let package = platform_release_package()?;
    install_runtime_ripgrep(package).await
}

fn find_existing_ripgrep() -> Option<PathBuf> {
    env::var_os(RG_ENV)
        .map(PathBuf::from)
        .filter(|path| is_executable_file(path))
        .or_else(find_bundled_ripgrep)
        .or_else(find_ripgrep_in_path)
        .or_else(|| existing_path("/opt/homebrew/bin/rg"))
        .or_else(|| existing_path("/usr/local/bin/rg"))
        .or_else(find_cached_ripgrep)
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
            if is_executable_file(&direct) {
                return Some(direct);
            }
            let nested = root.join("binaries").join(name);
            if is_executable_file(&nested) {
                return Some(nested);
            }
        }
    }

    None
}

fn find_cached_ripgrep() -> Option<PathBuf> {
    let package = platform_release_package().ok()?;
    let path = runtime_ripgrep_dir().ok()?.join(package.cache_name);
    is_executable_file(&path).then_some(path)
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
        .find(|path| is_executable_file(path))
}

fn existing_path(path: &str) -> Option<PathBuf> {
    let path = PathBuf::from(path);
    is_executable_file(&path).then_some(path)
}

async fn install_runtime_ripgrep(package: RipgrepPackage) -> Result<PathBuf> {
    let cache_dir = runtime_ripgrep_dir()?;
    tokio::fs::create_dir_all(&cache_dir)
        .await
        .with_context(|| format!("unable to create ripgrep cache at {}", cache_dir.display()))?;

    let final_path = cache_dir.join(package.cache_name);
    if is_executable_file(&final_path) {
        return Ok(final_path);
    }

    let temp_dir = cache_dir.join(format!(
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
                "unable to create temporary ripgrep directory at {}",
                temp_dir.display()
            )
        })?;

    let result = async {
        let archive_name = package.archive_name();
        let archive_url = format!("{RG_RELEASE_BASE}/{RG_VERSION}/{archive_name}");
        let checksum_url = format!("{archive_url}.sha256");
        let archive_path = temp_dir.join(&archive_name);

        let expected_checksum = download_text(&checksum_url)
            .await
            .with_context(|| format!("unable to download ripgrep checksum from {checksum_url}"))?;
        let archive_bytes = download_bytes(&archive_url)
            .await
            .with_context(|| format!("unable to download ripgrep from {archive_url}"))?;
        verify_sha256(&archive_bytes, &expected_checksum)
            .with_context(|| format!("checksum mismatch for {archive_name}"))?;
        tokio::fs::write(&archive_path, archive_bytes)
            .await
            .with_context(|| {
                format!(
                    "unable to write ripgrep archive to {}",
                    archive_path.display()
                )
            })?;

        let extracted_path = extract_archive(archive_path, temp_dir.clone(), package)
            .await
            .context("unable to extract ripgrep archive")?;
        set_executable_permissions(&extracted_path)
            .with_context(|| format!("unable to mark {} executable", extracted_path.display()))?;

        if is_executable_file(&final_path) {
            return Ok(final_path);
        }
        if final_path.exists() {
            let _ = tokio::fs::remove_file(&final_path).await;
        }
        match tokio::fs::rename(&extracted_path, &final_path).await {
            Ok(()) => Ok(final_path),
            Err(_) if is_executable_file(&final_path) => Ok(final_path),
            Err(err) => Err(err)
                .with_context(|| format!("unable to install ripgrep at {}", final_path.display())),
        }
    }
    .await;

    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    result
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

async fn download_text(url: &str) -> Result<String> {
    let bytes = download_bytes(url).await?;
    String::from_utf8(bytes).with_context(|| format!("response from {url} was not UTF-8"))
}

fn verify_sha256(bytes: &[u8], checksum_text: &str) -> Result<()> {
    let expected = checksum_text
        .split_whitespace()
        .next()
        .context("checksum file was empty")?;
    let actual = format!("{:x}", Sha256::digest(bytes));
    if !actual.eq_ignore_ascii_case(expected) {
        bail!("expected {expected}, got {actual}");
    }
    Ok(())
}

async fn extract_archive(
    archive_path: PathBuf,
    extract_dir: PathBuf,
    package: RipgrepPackage,
) -> Result<PathBuf> {
    tokio::task::spawn_blocking(move || match package.archive_kind {
        #[cfg(not(windows))]
        ArchiveKind::TarGz => extract_tar_gz(&archive_path, &extract_dir, package),
        #[cfg(windows)]
        ArchiveKind::Zip => extract_zip(&archive_path, &extract_dir, package),
    })
    .await
    .context("ripgrep extraction task failed")?
}

#[cfg(not(windows))]
fn extract_tar_gz(
    archive_path: &Path,
    extract_dir: &Path,
    package: RipgrepPackage,
) -> Result<PathBuf> {
    let archive_file = std::fs::File::open(archive_path)
        .with_context(|| format!("unable to open {}", archive_path.display()))?;
    let decoder = flate2::read::GzDecoder::new(archive_file);
    let mut archive = tar::Archive::new(decoder);
    let expected_path = package.expected_archive_path();
    let output_path = extract_dir.join(package.executable);

    for entry in archive.entries().context("unable to read tar entries")? {
        let mut entry = entry.context("unable to read tar entry")?;
        let entry_path = entry.path().context("tar entry has invalid path")?;
        if entry_path.as_ref() == expected_path.as_path() {
            entry
                .unpack(&output_path)
                .with_context(|| format!("unable to unpack {}", expected_path.display()))?;
            return Ok(output_path);
        }
    }

    bail!(
        "ripgrep archive did not contain executable: {}",
        expected_path.display()
    )
}

#[cfg(windows)]
fn extract_zip(
    archive_path: &Path,
    extract_dir: &Path,
    package: RipgrepPackage,
) -> Result<PathBuf> {
    let archive_file = std::fs::File::open(archive_path)
        .with_context(|| format!("unable to open {}", archive_path.display()))?;
    let mut archive = zip::ZipArchive::new(archive_file).context("unable to read zip archive")?;
    let expected_path = package.expected_archive_path();
    let output_path = extract_dir.join(package.executable);

    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .with_context(|| format!("unable to read zip entry {index}"))?;
        let Some(entry_path) = file.enclosed_name() else {
            continue;
        };
        if entry_path != expected_path {
            continue;
        }

        let mut output = std::fs::File::create(&output_path)
            .with_context(|| format!("unable to create {}", output_path.display()))?;
        std::io::copy(&mut file, &mut output)
            .with_context(|| format!("unable to extract {}", expected_path.display()))?;
        return Ok(output_path);
    }

    bail!(
        "ripgrep archive did not contain executable: {}",
        expected_path.display()
    )
}

fn runtime_ripgrep_dir() -> Result<PathBuf> {
    let dirs = ProjectDirs::from("dev", "hyrak", "sinew")
        .context("unable to resolve local cache directory")?;
    Ok(dirs.cache_dir().join("ripgrep").join(RG_VERSION))
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

#[derive(Clone, Copy)]
struct RipgrepPackage {
    archive_triple: &'static str,
    cache_name: &'static str,
    executable: &'static str,
    archive_ext: &'static str,
    archive_kind: ArchiveKind,
}

impl RipgrepPackage {
    fn archive_name(self) -> String {
        format!(
            "ripgrep-{RG_VERSION}-{}.{ext}",
            self.archive_triple,
            ext = self.archive_ext
        )
    }

    fn expected_archive_path(self) -> PathBuf {
        PathBuf::from(format!("ripgrep-{RG_VERSION}-{}", self.archive_triple)).join(self.executable)
    }
}

#[derive(Clone, Copy)]
enum ArchiveKind {
    #[cfg(not(windows))]
    TarGz,
    #[cfg(windows)]
    Zip,
}

fn platform_release_package() -> Result<RipgrepPackage> {
    platform_release_package_inner().ok_or_else(|| {
        anyhow::anyhow!(
            "automatic ripgrep install is not available for {}/{}; install ripgrep manually or set {RG_ENV}",
            env::consts::OS,
            env::consts::ARCH
        )
    })
}

fn platform_release_package_inner() -> Option<RipgrepPackage> {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        Some(RipgrepPackage {
            archive_triple: "x86_64-pc-windows-msvc",
            cache_name: "rg-x86_64-pc-windows-msvc.exe",
            executable: "rg.exe",
            archive_ext: "zip",
            archive_kind: ArchiveKind::Zip,
        })
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        Some(RipgrepPackage {
            archive_triple: "aarch64-apple-darwin",
            cache_name: "rg-aarch64-apple-darwin",
            executable: "rg",
            archive_ext: "tar.gz",
            archive_kind: ArchiveKind::TarGz,
        })
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        Some(RipgrepPackage {
            archive_triple: "x86_64-apple-darwin",
            cache_name: "rg-x86_64-apple-darwin",
            executable: "rg",
            archive_ext: "tar.gz",
            archive_kind: ArchiveKind::TarGz,
        })
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        Some(RipgrepPackage {
            archive_triple: "x86_64-unknown-linux-musl",
            cache_name: "rg-x86_64-unknown-linux-gnu",
            executable: "rg",
            archive_ext: "tar.gz",
            archive_kind: ArchiveKind::TarGz,
        })
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        Some(RipgrepPackage {
            archive_triple: "aarch64-unknown-linux-gnu",
            cache_name: "rg-aarch64-unknown-linux-gnu",
            executable: "rg",
            archive_ext: "tar.gz",
            archive_kind: ArchiveKind::TarGz,
        })
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

fn is_executable_file(path: &Path) -> bool {
    path.is_file() && has_execute_permission(path)
}

#[cfg(unix)]
fn has_execute_permission(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    path.metadata()
        .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn has_execute_permission(_path: &Path) -> bool {
    true
}

#[cfg(unix)]
fn set_executable_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_executable_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_package_matches_current_executable_name() {
        let Ok(package) = platform_release_package() else {
            return;
        };

        assert_eq!(package.executable, platform_executable_name());
        assert!(package
            .archive_name()
            .starts_with(&format!("ripgrep-{RG_VERSION}-")));
    }

    #[test]
    fn checksum_verification_accepts_sha256_files() {
        let bytes = b"ripgrep";
        let checksum = format!("{:x}  archive.tar.gz", Sha256::digest(bytes));
        verify_sha256(bytes, &checksum).expect("checksum should verify");
    }
}
