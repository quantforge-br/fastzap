//! Release check (ZapZap's `update_checker.py`): once per process, only for
//! official builds whose packaging has no updater of its own.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use ts_rs::TS;

use crate::build_info;

pub const UPDATE_AVAILABLE: &str = "update://available";
const CHECKED_PACKAGING: &[&str] = &[
    "DEB",
    "macOS",
    "Windows x86_64 (exe)",
    "Windows arm64 (exe)",
];

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    /// `YYYY-MM-DD`.
    pub published: Option<String>,
    /// Release notes on the official repository.
    pub notes_url: Option<String>,
    pub download_url: String,
}

/// The result of this process's check.
#[derive(Default)]
pub struct UpdateState {
    available: Mutex<Option<UpdateInfo>>,
}

impl UpdateState {
    pub fn get(&self) -> Option<UpdateInfo> {
        self.available.lock().unwrap().clone()
    }
}

pub fn policy_allows(channel: &str, provider: &str, repository: &str, packaging: &str) -> bool {
    channel == "Official"
        && provider == "GitHub Actions"
        && repository.eq_ignore_ascii_case(build_info::OFFICIAL_REPOSITORY)
        && CHECKED_PACKAGING.contains(&packaging)
}

#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    published_at: Option<String>,
    html_url: Option<String>,
}

fn parse_version(text: &str) -> Option<Vec<u64>> {
    let text = text.strip_prefix(['v', 'V']).unwrap_or(text);
    let mut parts = text
        .split('.')
        .map(|p| {
            (!p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
                .then(|| p.parse().ok())
                .flatten()
        })
        .collect::<Option<Vec<u64>>>()?;
    while parts.len() > 1 && parts.last() == Some(&0) {
        parts.pop();
    }
    Some(parts)
}

pub fn is_newer(current: &str, latest: &str) -> bool {
    match (parse_version(current), parse_version(latest)) {
        (Some(c), Some(l)) => c < l,
        _ => false,
    }
}

fn releases_prefix() -> String {
    format!(
        "https://github.com/{}/releases/",
        build_info::OFFICIAL_REPOSITORY
    )
}

fn notes_url(url: Option<String>) -> Option<String> {
    let url = url?;
    let parsed = url::Url::parse(&url).ok()?;
    (parsed.username().is_empty()
        && parsed.password().is_none()
        && url.starts_with(&releases_prefix()))
    .then_some(url)
}

fn evaluate(release: Release, current: &str) -> Option<UpdateInfo> {
    if release.draft || release.prerelease {
        return None;
    }
    let latest = release
        .tag_name
        .strip_prefix(['v', 'V'])
        .unwrap_or(&release.tag_name)
        .to_owned();
    if !is_newer(current, &latest) {
        return None;
    }
    Some(UpdateInfo {
        current: current.to_owned(),
        latest,
        published: release
            .published_at
            .and_then(|p| chrono::DateTime::parse_from_rfc3339(&p).ok())
            .map(|d| d.date_naive().to_string()),
        notes_url: notes_url(release.html_url),
        download_url: format!("{}latest", releases_prefix()),
    })
}

async fn fetch_latest(current: &str) -> anyhow::Result<Option<UpdateInfo>> {
    let release: Release = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .user_agent(format!("FastZap/{current}"))
        .build()?
        .get(format!(
            "https://api.github.com/repos/{}/releases/latest",
            build_info::OFFICIAL_REPOSITORY
        ))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(evaluate(release, current))
}

/// Check once in the background; failures are silent.
pub fn start<R: Runtime>(app: &AppHandle<R>) {
    if !policy_allows(
        build_info::CHANNEL,
        build_info::PROVIDER,
        build_info::REPOSITORY,
        build_info::PACKAGING,
    ) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        match fetch_latest(env!("CARGO_PKG_VERSION")).await {
            Ok(Some(info)) => {
                *app.state::<UpdateState>().available.lock().unwrap() = Some(info.clone());
                let _ = app.emit(UPDATE_AVAILABLE, info);
            }
            Ok(None) => {}
            Err(e) => log::debug!("update check failed: {e}"),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str) -> Release {
        Release {
            tag_name: tag.into(),
            draft: false,
            prerelease: false,
            published_at: Some("2026-10-01T12:00:00Z".into()),
            html_url: Some(format!("{}tag/{tag}", releases_prefix())),
        }
    }

    #[test]
    fn only_official_self_updating_builds_check() {
        let repo = build_info::OFFICIAL_REPOSITORY.to_uppercase();
        assert!(policy_allows("Official", "GitHub Actions", &repo, "DEB"));
        assert!(!policy_allows(
            "Official",
            "GitHub Actions",
            &repo,
            "Flatpak"
        ));
        assert!(!policy_allows(
            "Official",
            "GitHub Actions",
            "fork/fastzap",
            "DEB"
        ));
        assert!(!policy_allows("Unknown", "GitHub Actions", &repo, "DEB"));
        assert!(!policy_allows("Official", "", &repo, "DEB"));
    }

    #[test]
    fn versions_compare_numerically() {
        assert!(is_newer("0.1.0", "0.1.1"));
        assert!(is_newer("0.9", "0.10"));
        assert!(!is_newer("1.0", "1.0.0"));
        assert!(!is_newer("1.2.0", "v1.1.9"));
        assert!(!is_newer("1.0", "1.0-beta"));
    }

    #[test]
    fn releases_are_validated() {
        let info = evaluate(release("v0.2.0"), "0.1.0").unwrap();
        assert_eq!(info.latest, "0.2.0");
        assert_eq!(info.published.as_deref(), Some("2026-10-01"));
        assert!(info.notes_url.unwrap().ends_with("/releases/tag/v0.2.0"));
        assert!(evaluate(release("v0.1.0"), "0.1.0").is_none());

        let mut pre = release("v9.0.0");
        pre.prerelease = true;
        assert!(evaluate(pre, "0.1.0").is_none());
        let mut foreign = release("v9.0.0");
        foreign.html_url = Some("https://evil.example/releases/x".into());
        assert!(evaluate(foreign, "0.1.0").unwrap().notes_url.is_none());
        let mut userinfo = release("v9.0.0");
        userinfo.html_url = Some(format!(
            "https://x@github.com/{}/releases/x",
            build_info::OFFICIAL_REPOSITORY
        ));
        assert!(evaluate(userinfo, "0.1.0").unwrap().notes_url.is_none());
    }
}
