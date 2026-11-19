use serde::Deserialize;
use std::path::{Path, PathBuf};

const LATEST_RELEASE_API_URL: &str =
    "https://api.github.com/repos/watchakorn-18k/Language-Translation-App-using-AI/releases/latest";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateInfo {
    pub latest_version: String,
    pub release_url: String,
    pub download_url: Option<String>,
    pub asset_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    assets: Vec<ReleaseAsset>,
}

/// Parses a semver-like string (`v6.3.0`, `6.3`, `v6.10.2-beta`) into `(major, minor, patch)`.
pub fn parse_semver(raw: &str) -> Option<(u64, u64, u64)> {
    let cleaned = raw.trim().trim_start_matches('v').trim_start_matches('V');
    let core = cleaned.split('-').next()?.split('+').next()?;
    let mut parts = core.split('.');
    let major: u64 = parts.next()?.parse().ok()?;
    let minor: u64 = parts.next().unwrap_or("0").parse().ok()?;
    let patch: u64 = parts.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

/// Returns `true` if `latest` is strictly newer than `current`.
pub fn is_newer_version(current: &str, latest: &str) -> bool {
    match (parse_semver(current), parse_semver(latest)) {
        (Some(c), Some(l)) => l > c,
        _ => false,
    }
}

/// Picks the best release asset for the given `(os, arch)` pair, skipping `.sha256` checksums.
pub fn select_release_asset<'a>(
    assets: &'a [ReleaseAsset],
    os: &str,
    arch: &str,
) -> Option<&'a ReleaseAsset> {
    let non_checksum: Vec<&ReleaseAsset> = assets
        .iter()
        .filter(|a| !a.name.ends_with(".sha256"))
        .collect();

    match os {
        "macos" => {
            // Prefer universal or matching-arch .dmg, then .tar.gz
            non_checksum
                .iter()
                .copied()
                .find(|a| a.name.ends_with(".dmg") && a.name.contains("universal"))
                .or_else(|| {
                    non_checksum.iter().copied().find(|a| {
                        a.name.ends_with(".dmg")
                            && (a.name.contains(arch)
                                || (arch == "aarch64" && a.name.contains("arm64")))
                    })
                })
                .or_else(|| {
                    non_checksum
                        .iter()
                        .copied()
                        .find(|a| a.name.ends_with(".dmg"))
                })
                .or_else(|| {
                    non_checksum
                        .iter()
                        .copied()
                        .find(|a| a.name.contains("macos") && a.name.ends_with(".tar.gz"))
                })
        }
        "windows" => non_checksum
            .iter()
            .copied()
            .find(|a| a.name.ends_with(".msi"))
            .or_else(|| {
                non_checksum
                    .iter()
                    .copied()
                    .find(|a| a.name.contains("windows") && a.name.ends_with(".zip"))
            }),
        "linux" => non_checksum
            .iter()
            .copied()
            .find(|a| a.name.ends_with(".deb") || a.name.ends_with(".AppImage"))
            .or_else(|| {
                non_checksum
                    .iter()
                    .copied()
                    .find(|a| a.name.contains("linux") && a.name.ends_with(".tar.gz"))
            }),
        _ => None,
    }
}

/// Checks GitHub Releases asynchronously for a newer version than `current_version`.
pub async fn check_for_update(current_version: &str) -> Option<UpdateInfo> {
    let client = reqwest::Client::new();
    let resp = client
        .get(LATEST_RELEASE_API_URL)
        .header(
            "User-Agent",
            format!("language-translation-app/{current_version}"),
        )
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let release: GitHubRelease = resp.json().await.ok()?;
    if !is_newer_version(current_version, &release.tag_name) {
        return None;
    }

    let chosen = select_release_asset(
        &release.assets,
        std::env::consts::OS,
        std::env::consts::ARCH,
    );
    Some(UpdateInfo {
        latest_version: release.tag_name,
        release_url: release.html_url,
        download_url: chosen.map(|a| a.browser_download_url.clone()),
        asset_name: chosen.map(|a| a.name.clone()),
    })
}

/// Downloads a release asset into `dest_dir` and returns the saved file path.
pub async fn download_release_asset(
    url: &str,
    asset_name: &str,
    dest_dir: &Path,
) -> Result<PathBuf, String> {
    let client = reqwest::Client::new();
    let resp = client
        .get(url)
        .header("User-Agent", "language-translation-app")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Download failed with HTTP {}", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    std::fs::create_dir_all(dest_dir).map_err(|e| e.to_string())?;
    let target = dest_dir.join(asset_name);
    std::fs::write(&target, &bytes).map_err(|e| e.to_string())?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_semver_versions_accurately() {
        assert!(is_newer_version("6.3.0", "v6.4.0"));
        assert!(is_newer_version("v6.3.0", "v7.0.0"));
        assert!(is_newer_version("6.9.0", "v6.10.0"));
        assert!(!is_newer_version("6.3.0", "v6.3.0"));
        assert!(!is_newer_version("6.3.0", "v6.2.9"));
        assert!(!is_newer_version("6.3.0", "invalid"));
    }

    #[test]
    fn parses_two_part_and_prerelease_tags() {
        assert_eq!(parse_semver("v6.4"), Some((6, 4, 0)));
        assert_eq!(parse_semver("v6.4.1-rc1"), Some((6, 4, 1)));
    }

    #[test]
    fn selects_matching_platform_release_asset() {
        let assets = vec![
            ReleaseAsset {
                name: "Translate-App-with-AI-macos-universal.dmg.sha256".into(),
                browser_download_url: "https://example/dmg.sha256".into(),
            },
            ReleaseAsset {
                name: "Translate-App-with-AI-macos-universal.dmg".into(),
                browser_download_url: "https://example/dmg".into(),
            },
            ReleaseAsset {
                name: "language-translation-app-windows-x86_64.zip".into(),
                browser_download_url: "https://example/win.zip".into(),
            },
            ReleaseAsset {
                name: "language-translation-app-linux-amd64.deb".into(),
                browser_download_url: "https://example/linux.deb".into(),
            },
        ];

        assert_eq!(
            select_release_asset(&assets, "macos", "aarch64").map(|a| a.name.as_str()),
            Some("Translate-App-with-AI-macos-universal.dmg")
        );
        assert_eq!(
            select_release_asset(&assets, "windows", "x86_64").map(|a| a.name.as_str()),
            Some("language-translation-app-windows-x86_64.zip")
        );
        assert_eq!(
            select_release_asset(&assets, "linux", "x86_64").map(|a| a.name.as_str()),
            Some("language-translation-app-linux-amd64.deb")
        );
    }
}
