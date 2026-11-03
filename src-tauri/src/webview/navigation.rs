//! Which URLs stay inside an account webview (ZapZap's `is_internal_web_url`).

use url::Url;

pub const WHATSAPP_URL: &str = "https://web.whatsapp.com/";
const ALLOWED_HOSTS: &[&str] = &[
    "web.whatsapp.com",
    "webtp.whatsapp.net",
    "static.whatsapp.net",
];
const INTERNAL_SCHEMES: &[&str] = &["blob", "data"];

pub fn is_internal(url: &Url) -> bool {
    let scheme = url.scheme().to_ascii_lowercase();
    if INTERNAL_SCHEMES.contains(&scheme.as_str()) {
        return true;
    }
    if scheme == "about" {
        return url.as_str() != "about:blank";
    }
    matches!(scheme.as_str(), "http" | "https")
        && url
            .host_str()
            .is_some_and(|h| ALLOWED_HOSTS.contains(&h.to_ascii_lowercase().as_str()))
}

/// External links may be handed to the system browser only for safe schemes.
pub fn is_safe_external(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https" | "mailto" | "tel")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn whatsapp_hosts_and_blobs_are_internal() {
        assert!(is_internal(&u("https://web.whatsapp.com/")));
        assert!(is_internal(&u("https://static.whatsapp.net/x.js")));
        assert!(is_internal(&u("blob:https://web.whatsapp.com/abc")));
        assert!(is_internal(&u("data:text/plain,hi")));
        assert!(!is_internal(&u("about:blank")));
        assert!(!is_internal(&u("https://example.com/")));
        assert!(!is_internal(&u("https://web.whatsapp.com.evil.com/")));
        assert!(!is_internal(&u("file:///etc/passwd")));
    }

    #[test]
    fn only_safe_schemes_go_external() {
        assert!(is_safe_external(&u("https://example.com")));
        assert!(!is_safe_external(&u("file:///etc/passwd")));
        assert!(!is_safe_external(&u("javascript:alert(1)")));
    }
}
