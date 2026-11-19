use serde::Deserialize;

const LATEST_RELEASE_API_URL: &str =
    "https://api.github.com/repos/watchakorn-18k/Language-Translation-App-using-AI/releases/latest";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateInfo {
    pub latest_version: String,
    pub release_url: String,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
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

/// Checks GitHub Releases asynchronously for a newer version than `current_version`.
pub async fn check_for_update(current_version: &str) -> Option<UpdateInfo> {
    let client = reqwest::Client::new();
    let resp = client
        .get(LATEST_RELEASE_API_URL)
        .header("User-Agent", format!("language-translation-app/{current_version}"))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let release: GitHubRelease = resp.json().await.ok()?;
    if is_newer_version(current_version, &release.tag_name) {
        Some(UpdateInfo {
            latest_version: release.tag_name,
            release_url: release.html_url,
        })
    } else {
        None
    }
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
}
