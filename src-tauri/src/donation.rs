//! Donation destinations (ZapZap's `features/donation/model.py`). FastZap is
//! a port of ZapZap, so support goes to ZapZap's author; the UI only names a
//! method and the backend opens its fixed URL.

pub const WEBSITE: &str = "https://rtosta.com/zapzap/";

const METHODS: &[(&str, &str)] = &[
    ("github_sponsors", "https://github.com/sponsors/rafatosta"),
    ("pix", "https://nubank.com.br/pagar/3c3r2/LS2hiJJKzv"),
    (
        "paypal",
        "https://www.paypal.com/donate/?business=E7R4BVR45GRC2&no_recurring=0&item_name=ZapZap+-+Whatsapp+Desktop+for+linux%0AAn+unofficial+WhatsApp+desktop+application+written+in+Pyqt6+%2B+PyQt6-WebEngine.&currency_code=USD",
    ),
    ("wise", "https://wise.com/pay/me/rafaelt2487"),
    ("kofi", "https://ko-fi.com/rafaeltosta"),
];

/// The URL of a method id; unknown ids are refused.
pub fn url(method: &str) -> Option<&'static str> {
    METHODS
        .iter()
        .find(|(id, _)| *id == method)
        .map(|(_, url)| *url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_known_methods_resolve() {
        assert!(url("pix").unwrap().starts_with("https://"));
        assert_eq!(url("kofi"), Some("https://ko-fi.com/rafaeltosta"));
        assert_eq!(url("https://evil.example"), None);
        assert!(METHODS.iter().all(|(_, u)| u.starts_with("https://")));
    }
}
