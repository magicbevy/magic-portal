use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::error::{PortalError, PortalResult};
use super::paths::validate_component;
use super::progress::{CancelFlag, ProgressEvent, Reporter};
use super::version::EngineReleasesFile;
use crate::platform::{self, Os};

pub const ENGINE_OWNER: &str = "magicbevy";
pub const ENGINE_REPO: &str = "magic-bevy";
pub const VERSIONS_MAIN_URL: &str =
    "https://raw.githubusercontent.com/magicbevy/magic-bevy/main/versions.toml";
pub const VERSIONS_MASTER_URL: &str =
    "https://raw.githubusercontent.com/magicbevy/magic-bevy/master/versions.toml";
pub const RELEASES_API_URL: &str =
    "https://api.github.com/repos/magicbevy/magic-bevy/releases?per_page=100";
pub const USER_AGENT: &str = concat!("magic-portal/", env!("CARGO_PKG_VERSION"));

const MAX_TEXT_BYTES: u64 = 16 * 1024 * 1024;
const MAX_REDIRECTS: u32 = 5;

#[derive(Debug, Clone)]
pub struct GitHubAsset {
    pub name: String,
    pub url: String,
    pub size: Option<u64>,
    pub digest: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub name: Option<String>,
    pub html_url: String,
    pub zipball_url: String,
    pub tarball_url: String,
    pub assets: Vec<GitHubAsset>,
}

#[derive(Debug, Clone)]
pub struct ReleaseAsset {
    pub name: String,
    pub url: String,
    pub size: Option<u64>,
    pub digest: Option<String>,
    pub checksum_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawRelease {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    zipball_url: String,
    #[serde(default)]
    tarball_url: String,
    #[serde(default)]
    assets: Vec<RawAsset>,
}

#[derive(Debug, Deserialize)]
struct RawAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: Option<u64>,
    #[serde(default)]
    digest: Option<String>,
}

pub fn fetch_versions_toml() -> PortalResult<EngineReleasesFile> {
    let mut last_error = None;
    for url in [VERSIONS_MAIN_URL, VERSIONS_MASTER_URL] {
        match http_get_text(url, None) {
            Ok(text) => {
                return toml::from_str(&text).map_err(|error| PortalError::InvalidManifest {
                    path: PathBuf::from(url),
                    message: format!("invalid versions.toml: {error}"),
                });
            }
            Err(error) => last_error = Some((url, error)),
        }
    }

    let (url, error) = last_error.unwrap_or((
        VERSIONS_MAIN_URL,
        PortalError::Network("no versions.toml URL was attempted".into()),
    ));
    Err(PortalError::Network(format!(
        "failed to fetch versions.toml from {url}: {error}"
    )))
}

pub fn fetch_releases() -> PortalResult<Vec<GitHubRelease>> {
    let text = http_get_text(RELEASES_API_URL, Some("application/vnd.github+json"))?;
    let raw: Vec<RawRelease> =
        serde_json::from_str(&text).map_err(|error| PortalError::InvalidManifest {
            path: PathBuf::from(RELEASES_API_URL),
            message: format!("invalid GitHub releases JSON: {error}"),
        })?;

    Ok(raw
        .into_iter()
        .map(|release| GitHubRelease {
            tag_name: release.tag_name,
            name: release.name,
            html_url: release.html_url,
            zipball_url: release.zipball_url,
            tarball_url: release.tarball_url,
            assets: release
                .assets
                .into_iter()
                .map(|asset| GitHubAsset {
                    name: asset.name,
                    url: asset.browser_download_url,
                    size: asset.size,
                    digest: asset.digest,
                })
                .collect(),
        })
        .collect())
}

pub fn find_release<'a>(releases: &'a [GitHubRelease], version: &str) -> Option<&'a GitHubRelease> {
    let version_tag = format!("v{version}");
    releases
        .iter()
        .find(|release| release.tag_name == version || release.tag_name == version_tag)
        .or_else(|| {
            releases.iter().find(|release| {
                release
                    .name
                    .as_deref()
                    .is_some_and(|name| name == version || name == version_tag)
            })
        })
}

pub fn resolve_assets(
    release: &GitHubRelease,
    version: &str,
    os: Os,
) -> PortalResult<(ReleaseAsset, ReleaseAsset)> {
    let source = match find_source_asset(release, version) {
        Some(source) => source,
        None => {
            let tag_url = if release.zipball_url.is_empty() {
                release.tarball_url.clone()
            } else {
                release.zipball_url.clone()
            };
            if tag_url.is_empty() {
                return Err(PortalError::NotFound(format!(
                    "release {} has no source archive and no tag archive URL",
                    release.tag_name
                )));
            }
            ReleaseAsset {
                name: format!("magic-bevy-{version}-source.zip"),
                url: tag_url,
                size: None,
                digest: None,
                checksum_url: None,
            }
        }
    };
    let binary =
        find_binary_asset(release, version, os, platform::architecture()).ok_or_else(|| {
            PortalError::NotFound(format!(
                "release {} has no archive for {} {}",
                release.tag_name,
                os.name(),
                platform::architecture()
            ))
        })?;
    Ok((source, binary))
}

pub fn download(
    url: &str,
    label: &str,
    destination: &Path,
    max_bytes: u64,
    reporter: &mut dyn Reporter,
    cancel: &CancelFlag,
) -> PortalResult<u64> {
    ensure_https(url)?;
    ensure_trusted_url(url)?;
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|error| PortalError::io(parent, error))?;
    }

    let part = download_part_path(destination);
    let response = agent().get(url).set("User-Agent", USER_AGENT).call()?;
    let status = response.status();
    if !(200..300).contains(&status) {
        return Err(PortalError::NotFound(format!(
            "HTTP {status} while downloading {url}"
        )));
    }
    ensure_trusted_url(response.get_url())?;

    let total = response
        .header("content-length")
        .and_then(|value| value.parse::<u64>().ok());
    if total.is_some_and(|size| size > max_bytes) {
        return Err(PortalError::Install(format!(
            "download at {url} is larger than the {max_bytes} byte limit"
        )));
    }

    let mut reader = response.into_reader();
    let mut file = File::create(&part).map_err(|error| PortalError::io(&part, error))?;
    let mut buffer = [0_u8; 64 * 1024];
    let mut received = 0_u64;

    loop {
        cancel.check()?;
        let read = reader.read(&mut buffer).map_err(|error| {
            let _ = fs::remove_file(&part);
            PortalError::Network(format!("failed to download {url}: {error}"))
        })?;
        if read == 0 {
            break;
        }

        received = received.saturating_add(read as u64);
        if received > max_bytes {
            let _ = fs::remove_file(&part);
            return Err(PortalError::Install(format!(
                "download at {url} is larger than the {max_bytes} byte limit"
            )));
        }

        file.write_all(&buffer[..read]).map_err(|error| {
            let _ = fs::remove_file(&part);
            PortalError::io(&part, error)
        })?;
        reporter.report(ProgressEvent::Download {
            label: label.to_string(),
            received,
            total,
        });
    }

    file.flush()
        .map_err(|error| PortalError::io(&part, error))?;
    drop(file);
    replace_file(&part, destination)?;
    Ok(received)
}

pub fn verify_checksum(
    path: &Path,
    checksum_url: Option<&str>,
    expected_digest: Option<&str>,
    label: &str,
    reporter: &mut dyn Reporter,
) -> PortalResult<bool> {
    if let Some(expected_digest) = expected_digest {
        let expected = parse_digest(expected_digest).ok_or_else(|| {
            PortalError::Install(format!(
                "release digest for {label} is not a valid SHA-256 digest: {expected_digest}"
            ))
        })?;
        let actual = sha256_file(path)?;
        if actual != expected {
            return Err(PortalError::ChecksumMismatch {
                path: path.to_path_buf(),
            });
        }
        reporter.report(ProgressEvent::log(format!(
            "checksum ok for {label} (release digest)"
        )));
        return Ok(true);
    }

    let Some(url) = checksum_url else {
        reporter.report(ProgressEvent::log(format!(
            "no checksum published for {label}; skipping verification"
        )));
        return Ok(false);
    };

    let text = http_get_text(url, None)?;
    let expected = parse_checksum(&text).ok_or_else(|| {
        PortalError::Install(format!(
            "checksum file at {url} does not contain a SHA-256 digest"
        ))
    })?;
    let actual = sha256_file(path)?;
    if actual != expected {
        return Err(PortalError::ChecksumMismatch {
            path: path.to_path_buf(),
        });
    }

    reporter.report(ProgressEvent::log(format!("checksum ok for {label}")));
    Ok(true)
}

pub fn sha256_file(path: &Path) -> PortalResult<String> {
    let mut file = File::open(path).map_err(|error| PortalError::io(path, error))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| PortalError::io(path, error))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    let digest = hasher.finalize();
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(output, "{byte:02x}");
    }
    Ok(output)
}

pub fn parse_checksum(text: &str) -> Option<String> {
    let token = text.split_whitespace().next()?.trim();
    if token.len() != 64 || !token.chars().all(|character| character.is_ascii_hexdigit()) {
        return None;
    }
    Some(token.to_ascii_lowercase())
}

pub fn parse_digest(text: &str) -> Option<String> {
    let token = text.trim();
    let token = token.strip_prefix("sha256:").unwrap_or(token);
    let token = token.split_whitespace().next()?;
    if token.len() != 64 || !token.chars().all(|character| character.is_ascii_hexdigit()) {
        return None;
    }
    Some(token.to_ascii_lowercase())
}

pub fn release_page_url(release: &GitHubRelease) -> String {
    if release.html_url.is_empty() {
        format!(
            "https://github.com/{ENGINE_OWNER}/{ENGINE_REPO}/releases/tag/{}",
            release.tag_name
        )
    } else {
        release.html_url.clone()
    }
}

pub fn repository_url() -> String {
    format!("https://github.com/{ENGINE_OWNER}/{ENGINE_REPO}")
}

fn find_source_asset(release: &GitHubRelease, version: &str) -> Option<ReleaseAsset> {
    let exact = format!("magic-bevy-{version}-source.zip");
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name == exact)
        .or_else(|| {
            release.assets.iter().find(|asset| {
                asset.name.starts_with("magic-bevy-") && asset.name.ends_with("-source.zip")
            })
        })
        .or_else(|| {
            release
                .assets
                .iter()
                .find(|asset| asset.name.ends_with(".zip") && asset.name.contains("source"))
        })?;
    validate_component(&asset.name, "asset name").ok()?;
    Some(asset_from(release, asset))
}

fn find_binary_asset(
    release: &GitHubRelease,
    version: &str,
    os: Os,
    architecture: &str,
) -> Option<ReleaseAsset> {
    let extension = os.archive_extension();
    let exact = format!(
        "magic-bevy-{version}-{}-{architecture}.{extension}",
        os.name()
    );
    let os_token = format!("-{}-", os.name());
    let arch_token = format!("-{architecture}.");
    let arch_suffix = format!("-{architecture}.{extension}");
    let archive_suffix = format!(".{extension}");

    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name == exact)
        .or_else(|| {
            release.assets.iter().find(|asset| {
                asset.name.starts_with("magic-bevy-")
                    && asset.name.ends_with(&archive_suffix)
                    && !asset.name.contains("source")
                    && asset.name.contains(&os_token)
                    && (asset.name.contains(&arch_token) || asset.name.ends_with(&arch_suffix))
            })
        })
        .or_else(|| {
            release.assets.iter().find(|asset| {
                asset.name.ends_with(&archive_suffix)
                    && !asset.name.contains("source")
                    && asset.name.contains(&os_token)
                    && (asset.name.contains(&arch_token) || asset.name.ends_with(&arch_suffix))
            })
        })?;
    validate_component(&asset.name, "asset name").ok()?;
    Some(asset_from(release, asset))
}

fn asset_from(release: &GitHubRelease, asset: &GitHubAsset) -> ReleaseAsset {
    let checksum_url = release
        .assets
        .iter()
        .find(|candidate| {
            candidate.name == format!("{}.sha256", asset.name)
                || candidate.name == format!("{}.sha256sum", asset.name)
        })
        .map(|candidate| candidate.url.clone());

    ReleaseAsset {
        name: asset.name.clone(),
        url: asset.url.clone(),
        size: asset.size,
        digest: asset.digest.clone(),
        checksum_url,
    }
}

fn download_part_path(destination: &Path) -> PathBuf {
    let name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("download");
    destination.with_file_name(format!(".{name}.part"))
}

fn replace_file(source: &Path, destination: &Path) -> PortalResult<()> {
    #[cfg(windows)]
    if destination.exists() {
        fs::remove_file(destination).map_err(|error| PortalError::io(destination, error))?;
    }

    fs::rename(source, destination).map_err(|error| {
        let _ = fs::remove_file(source);
        PortalError::io(destination, error)
    })
}

fn http_get_text(url: &str, accept: Option<&str>) -> PortalResult<String> {
    ensure_https(url)?;
    ensure_trusted_url(url)?;

    let mut request = agent().get(url).set("User-Agent", USER_AGENT);
    if let Some(accept) = accept {
        request = request.set("Accept", accept);
    }

    let response = request.call()?;
    let status = response.status();
    if !(200..300).contains(&status) {
        return Err(PortalError::NotFound(format!(
            "HTTP {status} while fetching {url}"
        )));
    }
    ensure_trusted_url(response.get_url())?;

    let mut text = String::new();
    response
        .into_reader()
        .take(MAX_TEXT_BYTES)
        .read_to_string(&mut text)
        .map_err(|error| PortalError::Network(format!("failed to read {url}: {error}")))?;
    Ok(text)
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(20))
        .timeout_read(Duration::from_secs(120))
        .timeout_write(Duration::from_secs(120))
        .redirects(MAX_REDIRECTS)
        .build()
}

fn ensure_https(url: &str) -> PortalResult<()> {
    if !url.starts_with("https://") {
        return Err(PortalError::Unsupported(format!(
            "refusing non-HTTPS URL: {url}"
        )));
    }
    Ok(())
}

fn ensure_trusted_url(url: &str) -> PortalResult<()> {
    let host = url_host(url).ok_or_else(|| {
        PortalError::Unsupported(format!("could not determine host for URL: {url}"))
    })?;
    let trusted = host == "github.com"
        || host.ends_with(".github.com")
        || host == "githubusercontent.com"
        || host.ends_with(".githubusercontent.com");
    if trusted {
        Ok(())
    } else {
        Err(PortalError::Unsupported(format!(
            "refusing redirect to untrusted host: {host}"
        )))
    }
}

fn url_host(url: &str) -> Option<&str> {
    let rest = url.strip_prefix("https://")?;
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority.rsplit('@').next()?;
    host.split(':').next()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(name: &str) -> GitHubAsset {
        GitHubAsset {
            name: name.into(),
            url: format!("https://github.com/magicbevy/magic-bevy/releases/download/v0.1.0/{name}"),
            size: Some(10),
            digest: None,
        }
    }

    fn release(assets: Vec<GitHubAsset>) -> GitHubRelease {
        GitHubRelease {
            tag_name: "v0.1.0".into(),
            name: Some("nova".into()),
            html_url: "https://github.com/magicbevy/magic-bevy/releases/tag/v0.1.0".into(),
            zipball_url:
                "https://api.github.com/repos/magicbevy/magic-bevy/zipball/refs/tags/v0.1.0".into(),
            tarball_url:
                "https://api.github.com/repos/magicbevy/magic-bevy/tarball/refs/tags/v0.1.0".into(),
            assets,
        }
    }

    #[test]
    fn resolves_source_and_binary_assets() {
        let release = release(vec![
            asset("magic-bevy-0.1.0-source.zip"),
            asset("magic-bevy-0.1.0-linux-x86_64.tar.gz"),
            asset("magic-bevy-0.1.0-macos-aarch64.tar.gz"),
        ]);

        let (source, binary) =
            resolve_assets(&release, "0.1.0", Os::Linux).expect("resolve assets");
        assert_eq!(source.name, "magic-bevy-0.1.0-source.zip");
        assert_eq!(binary.name, "magic-bevy-0.1.0-linux-x86_64.tar.gz");
    }

    #[test]
    fn resolves_windows_zip_asset() {
        let release = release(vec![
            asset("magic-bevy-0.1.0-source.zip"),
            asset("magic-bevy-0.1.0-windows-x86_64.zip"),
        ]);

        let (_, binary) =
            resolve_assets(&release, "0.1.0", Os::Windows).expect("resolve windows assets");
        assert_eq!(binary.name, "magic-bevy-0.1.0-windows-x86_64.zip");
    }

    #[test]
    fn missing_binary_asset_is_an_error() {
        let release = release(vec![asset("magic-bevy-0.1.0-source.zip")]);
        assert!(resolve_assets(&release, "0.1.0", Os::Linux).is_err());
    }

    #[test]
    fn falls_back_to_tag_archive_when_no_source_asset_exists() {
        let release = release(vec![asset("magic-bevy-0.1.0-linux-x86_64.tar.gz")]);
        let (source, binary) =
            resolve_assets(&release, "0.1.0", Os::Linux).expect("resolve assets");
        assert_eq!(source.url, release.zipball_url);
        assert_eq!(binary.name, "magic-bevy-0.1.0-linux-x86_64.tar.gz");
    }

    #[test]
    fn finds_release_by_tag_or_name() {
        let releases = vec![release(Vec::new())];
        assert!(find_release(&releases, "0.1.0").is_some());
        assert!(find_release(&releases, "0.2.0").is_none());
    }

    #[test]
    fn parses_and_rejects_checksums() {
        let digest = "a".repeat(64);
        assert_eq!(
            parse_checksum(&format!("{digest}  magic-bevy.zip")),
            Some(digest.clone())
        );
        assert_eq!(parse_checksum("not-a-digest"), None);
        assert_eq!(parse_checksum(&"g".repeat(64)), None);
    }

    #[test]
    fn parses_release_digests() {
        let digest = "b".repeat(64);
        assert_eq!(parse_digest(&format!("sha256:{digest}")), Some(digest));
        assert_eq!(parse_digest("sha256:nope"), None);
    }

    #[test]
    fn rejects_untrusted_hosts() {
        assert!(ensure_trusted_url("https://github.com/magicbevy/magic-bevy").is_ok());
        assert!(ensure_trusted_url("https://objects.githubusercontent.com/file").is_ok());
        assert!(ensure_trusted_url("https://evil.example/file").is_err());
    }

    #[test]
    fn requires_https() {
        assert!(ensure_https("http://github.com/file").is_err());
        assert!(ensure_https("https://github.com/file").is_ok());
    }
}
