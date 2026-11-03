//! User-Agent presets, identical to ZapZap's `WebView.USER_AGENTS`.

pub const DEFAULT_KEY: &str = "Default";

#[cfg(target_os = "windows")]
const PLATFORM_DEFAULT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
#[cfg(target_os = "macos")]
const PLATFORM_DEFAULT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const PLATFORM_DEFAULT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

pub const USER_AGENTS: &[(&str, &str)] = &[
    (DEFAULT_KEY, PLATFORM_DEFAULT),
    (
        "Windows Chrome",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    ),
    (
        "Windows Firefox",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:121.0) Gecko/20100101 Firefox/121.0",
    ),
    (
        "Windows Edge",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 Edg/120.0.0.0",
    ),
    (
        "Mac Safari",
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.2.1 Safari/605.1.15",
    ),
    (
        "Mac Chrome",
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    ),
    (
        "Linux Firefox",
        "Mozilla/5.0 (X11; Linux x86_64; rv:121.0) Gecko/20100101 Firefox/121.0",
    ),
    (
        "Linux Chrome",
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    ),
];

/// Resolve a preset key to its User-Agent string, falling back to the default.
pub fn resolve(key: &str) -> &'static str {
    USER_AGENTS
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, ua)| *ua)
        .unwrap_or(PLATFORM_DEFAULT)
}

pub fn keys() -> Vec<&'static str> {
    USER_AGENTS.iter().map(|(k, _)| *k).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_key_falls_back_to_default() {
        assert_eq!(resolve("nope"), resolve(DEFAULT_KEY));
        assert!(resolve("Linux Firefox").contains("Firefox/121.0"));
    }
}
