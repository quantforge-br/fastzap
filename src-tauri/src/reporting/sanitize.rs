//! ZapZap's report sanitizer: personal paths, URLs, addresses, identifiers
//! and secrets are removed before anything is stored or shown.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

const REMOVED: &str = "[removed]";

static HOME_PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(^|[^\w])(/home/[^/\s]+|/Users/[^/\s]+|[A-Za-z]:\\Users\\[^\\\s]+)").unwrap()
});
static URL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"https?://[^\s<>"]+"#).unwrap());
/// `(prefix, pattern)` pairs; the prefix group stands in for a look-behind.
static PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        // email
        r"(^|[^\w.+-])[\w.+-]+@[\w.-]+\.[A-Za-z]{2,}\b",
        // WhatsApp chat id
        r"(?i)()\b\d{6,20}@(?:c|g|s\.whatsapp)\.(?:us|net)\b",
        // GitHub token
        r"()\b(?:gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,})\b",
        // credential assignments, quoted or not (`password=…`, `"token": "…"`,
        // `Authorization: Basic …`)
        r#"(?i)()["']?\b[\w-]*(?:passw(?:or)?d|secret|token|api[_-]?key|authorization|credential)[\w-]*["']?\s*[:=]\s*(?:"[^"]*"|'[^']*'|(?:basic|bearer|digest)\s+\S+|[^\s,;&}\]]+)"#,
        // bearer / token / authorization values
        r"(?i)()\b(?:bearer|token|authorization|apikey)\s*[:=]?\s*[A-Za-z0-9._~+/=-]{8,}",
        // cookies
        r"(?i)()\b(?:set-cookie|cookie)\s*[:=]\s*[^\r\n;]+(?:;[^\r\n]*)?",
        // uuid
        r"()\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[1-5][0-9a-fA-F]{3}-[89abAB][0-9a-fA-F]{3}-[0-9a-fA-F]{12}\b",
        // phone numbers
        r"(^|[^\w])(?:\+?\d[\s().-]*){7,14}\d\b",
    ]
    .iter()
    .map(|p| Regex::new(p).unwrap())
    .collect()
});
static HEX_ID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[0-9a-f]{32,64}$").unwrap());
static VERSION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d+(\.\d+){1,4}([-+.][\w.]+)?$").unwrap());
static ISO_TIME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:\d{2})$").unwrap()
});

const SECRET_KEYS: &[&str] = &[
    "cookie",
    "password",
    "secret",
    "token",
    "authorization",
    "apikey",
    "localstorage",
    "sessionstorage",
];

pub fn text(input: &str) -> String {
    let mut out = input.replace('\0', "");
    if let Some(home) = dirs::home_dir().and_then(|h| h.to_str().map(str::to_owned))
        && home.len() > 1
    {
        out = out.replace(&home, "$HOME");
    }
    out = HOME_PATH.replace_all(&out, "${1}$$HOME").into_owned();
    out = URL
        .replace_all(&out, |caps: &regex::Captures| {
            match url::Url::parse(&caps[0]) {
                Ok(url) => format!(
                    "{}://{}/{REMOVED}",
                    url.scheme(),
                    url.host_str().unwrap_or("")
                ),
                Err(_) => "[removed URL]".into(),
            }
        })
        .into_owned();
    for pattern in PATTERNS.iter() {
        out = pattern
            .replace_all(&out, format!("${{1}}{REMOVED}").as_str())
            .into_owned();
    }
    out
}

fn kept_verbatim(key: &str, value: &str) -> bool {
    (matches!(key, "session_id" | "fingerprint") && HEX_ID.is_match(value))
        || (key.ends_with("version") && VERSION.is_match(value))
        || (matches!(key, "started_at" | "timestamp" | "created_at") && ISO_TIME.is_match(value))
}

/// Sanitize a JSON document: secret-looking keys are dropped and strings go
/// through [`text`] unless they are well-formed ids, versions or times.
pub fn value(input: &Value) -> Value {
    match input {
        Value::String(s) => Value::String(text(s)),
        Value::Array(items) => Value::Array(items.iter().map(value).collect()),
        Value::Object(map) => Value::Object(
            map.iter()
                .filter(|(k, _)| {
                    let k = k.to_lowercase();
                    !SECRET_KEYS.iter().any(|s| k.contains(s))
                })
                .map(|(k, v)| {
                    let clean = match v {
                        Value::String(s) if kept_verbatim(k, s) => v.clone(),
                        other => value(other),
                    };
                    (k.clone(), clean)
                })
                .collect(),
        ),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_assignments_are_removed() {
        for leak in [
            "Authorization: Basic dXNlcjpwYXNzd29yZA==",
            "proxy_password=hunter2",
            r#"{"token":"abcdefghijklmno"}"#,
            "api-key: 'abc def'",
            "secret = s3cr3t-value",
        ] {
            let out = text(leak);
            for secret in [
                "dXNlcjpwYXNzd29yZA",
                "hunter2",
                "abcdefghijklmno",
                "abc def",
                "s3cr3t",
            ] {
                assert!(!out.contains(secret), "{leak:?} -> {out:?}");
            }
        }
        assert_eq!(text("tokens are fine"), "tokens are fine");
    }
    use serde_json::json;

    #[test]
    fn removes_personal_data() {
        let input = "user@example.com wrote from /home/alice/x and https://web.whatsapp.com/send?phone=5511999999999 \
                     5511999999999@c.us call +55 (11) 99999-9999 ghp_abcdefghijklmnopqrstuvwx \
                     Authorization: Bearer abcdefgh12345678 id 123e4567-e89b-42d3-a456-426614174000";
        let out = text(input);
        for leaked in [
            "user@example.com",
            "alice",
            "phone=",
            "5511999999999",
            "99999-9999",
            "ghp_",
            "abcdefgh12345678",
            "123e4567",
        ] {
            assert!(!out.contains(leaked), "{leaked} leaked in {out}");
        }
        assert!(out.contains("$HOME/x"));
        assert!(out.contains("https://web.whatsapp.com/[removed]"));
    }

    #[test]
    fn keeps_ordinary_text_and_numbers() {
        assert_eq!(text("Error 404 at line 12"), "Error 404 at line 12");
    }

    #[test]
    fn documents_drop_secret_keys_and_keep_ids() {
        let doc = json!({
            "fastzap_version": "0.1.0",
            "timestamp": "2026-11-03T10:00:00Z",
            "session_id": "0123456789abcdef0123456789abcdef",
            "proxy_password": "hunter2",
            "nested": [{"note": "mail me at a@b.io", "cookie": "x"}],
        });
        let clean = value(&doc);
        assert_eq!(clean["fastzap_version"], "0.1.0");
        assert_eq!(clean["timestamp"], "2026-11-03T10:00:00Z");
        assert_eq!(clean["session_id"], "0123456789abcdef0123456789abcdef");
        assert!(clean.get("proxy_password").is_none());
        assert_eq!(clean["nested"][0]["note"], "mail me at [removed]");
        assert!(clean["nested"][0].get("cookie").is_none());
    }
}
