//! Global proxy applied to WhatsApp Web webviews.
//!
//! The system webviews accept `http://` and `socks5://` proxies without
//! authentication; `DefaultProxy` and `NoProxy` leave the engine default
//! (the system proxy configuration).

use url::Url;

use crate::settings::{ProxyKind, ProxySettings};

pub fn webview_proxy_url(settings: &ProxySettings) -> Option<Url> {
    if !settings.enabled || settings.host.trim().is_empty() || settings.port == 0 {
        return None;
    }
    let scheme = match settings.kind {
        ProxyKind::Socks5Proxy => "socks5",
        ProxyKind::HttpProxy | ProxyKind::HttpCachingProxy => "http",
        ProxyKind::NoProxy | ProxyKind::DefaultProxy | ProxyKind::FtpCachingProxy => return None,
    };
    let host = settings.host.trim();
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_owned()
    };
    Url::parse(&format!("{scheme}://{host}:{}", settings.port)).ok()
}

/// The webviews cannot authenticate to a proxy; the UI warns when a user is set.
pub fn credentials_supported() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proxy(kind: ProxyKind, host: &str, port: u32) -> ProxySettings {
        ProxySettings {
            enabled: true,
            kind,
            host: host.into(),
            port,
            user: String::new(),
        }
    }

    #[test]
    fn builds_supported_proxy_urls() {
        assert_eq!(
            webview_proxy_url(&proxy(ProxyKind::Socks5Proxy, "127.0.0.1", 1080))
                .unwrap()
                .as_str(),
            "socks5://127.0.0.1:1080"
        );
        assert_eq!(
            webview_proxy_url(&proxy(ProxyKind::HttpProxy, "::1", 8080))
                .unwrap()
                .as_str(),
            "http://[::1]:8080/"
        );
        assert!(webview_proxy_url(&proxy(ProxyKind::DefaultProxy, "h", 1)).is_none());
        let mut disabled = proxy(ProxyKind::HttpProxy, "h", 1);
        disabled.enabled = false;
        assert!(webview_proxy_url(&disabled).is_none());
    }
}
