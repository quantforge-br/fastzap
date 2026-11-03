//! Minimal reader for Qt `QSettings` INI files as written by ZapZap.
//!
//! Only the value shapes ZapZap stores are supported: plain strings, booleans
//! as `true`/`false`, comma-separated string lists, `@Point(x y)` and
//! `@Variant(...)` string lists (decoded heuristically).

use std::collections::BTreeMap;

#[derive(Debug, Default)]
pub struct QSettingsIni {
    values: BTreeMap<String, String>,
}

impl QSettingsIni {
    pub fn parse(text: &str) -> Self {
        let mut values = BTreeMap::new();
        let mut section = String::new();
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }
            if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                section = if name == "General" {
                    String::new()
                } else {
                    unescape_key(name)
                };
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = unescape_key(key.trim());
            let full = if section.is_empty() {
                key
            } else {
                format!("{section}/{key}")
            };
            values.insert(full, value.trim().to_owned());
        }
        Self { values }
    }

    pub fn contains(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.values.keys().map(String::as_str)
    }

    pub fn raw(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    pub fn string(&self, key: &str) -> Option<String> {
        let raw = self.raw(key)?;
        if raw.starts_with('@') {
            return None;
        }
        Some(unquote(raw))
    }

    pub fn bool(&self, key: &str) -> Option<bool> {
        match self.string(key)?.to_ascii_lowercase().as_str() {
            "true" | "1" => Some(true),
            "false" | "0" => Some(false),
            _ => None,
        }
    }

    pub fn int(&self, key: &str) -> Option<i64> {
        self.string(key)?.trim().parse().ok()
    }

    pub fn float(&self, key: &str) -> Option<f64> {
        self.string(key)?.trim().parse().ok()
    }

    pub fn list(&self, key: &str) -> Option<Vec<String>> {
        let raw = self.raw(key)?;
        if let Some(inner) = raw
            .strip_prefix("@Variant(")
            .and_then(|r| r.strip_suffix(')'))
        {
            return Some(variant_strings(&decode_escapes(inner)));
        }
        if raw == "@Invalid()" || raw.is_empty() {
            return Some(Vec::new());
        }
        Some(split_list(raw))
    }

    pub fn point(&self, key: &str) -> Option<(i32, i32)> {
        let inner = self.raw(key)?.strip_prefix("@Point(")?.strip_suffix(')')?;
        let mut parts = inner.split_whitespace();
        Some((parts.next()?.parse().ok()?, parts.next()?.parse().ok()?))
    }
}

fn unescape_key(key: &str) -> String {
    let key = key.replace('\\', "/");
    // QSettings percent-encodes non-ASCII and special key characters.
    let bytes = key.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(hex) = key.get(i + 1..i + 3)
            && let Ok(b) = u8::from_str_radix(hex, 16)
        {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn unquote(raw: &str) -> String {
    let raw = raw.trim();
    if raw.len() >= 2 && raw.starts_with('"') && raw.ends_with('"') {
        raw[1..raw.len() - 1]
            .replace("\\\"", "\"")
            .replace("\\\\", "\\")
    } else {
        raw.to_owned()
    }
}

fn split_list(raw: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for c in raw.chars() {
        match c {
            '"' => quoted = !quoted,
            ',' if !quoted => {
                items.push(current.trim().to_owned());
                current.clear();
            }
            _ => current.push(c),
        }
    }
    if !current.trim().is_empty() {
        items.push(current.trim().to_owned());
    }
    items.retain(|s| !s.is_empty());
    items
}

/// Decode Qt's INI byte escapes (`\0`, `\xNN`, `\t`, `\n`, `\r`, `\a`, `\b`, `\f`, `\v`).
fn decode_escapes(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'\\' || i + 1 >= bytes.len() {
            out.push(bytes[i]);
            i += 1;
            continue;
        }
        let next = bytes[i + 1];
        i += 2;
        match next {
            b'0' => out.push(0),
            b'a' => out.push(7),
            b'b' => out.push(8),
            b't' => out.push(9),
            b'n' => out.push(10),
            b'v' => out.push(11),
            b'f' => out.push(12),
            b'r' => out.push(13),
            b'x' => {
                let start = i;
                while i < bytes.len() && i - start < 2 && bytes[i].is_ascii_hexdigit() {
                    i += 1;
                }
                let hex = std::str::from_utf8(&bytes[start..i]).unwrap_or("0");
                out.push(u8::from_str_radix(hex, 16).unwrap_or(0));
            }
            other => out.push(other),
        }
    }
    out
}

/// Extract the UTF-16BE strings of a serialized QVariant list/QStringList.
///
/// Each string is stored as a big-endian `u32` byte length followed by UTF-16BE
/// code units, so we scan for length prefixes that are followed by a plausible
/// printable string.
fn variant_strings(bytes: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 4 <= bytes.len() {
        let len = u32::from_be_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]) as usize;
        let start = i + 4;
        if len > 0 && len % 2 == 0 && start + len <= bytes.len() {
            let units: Vec<u16> = bytes[start..start + len]
                .chunks_exact(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect();
            if let Ok(s) = String::from_utf16(&units)
                && s.chars().all(|c| !c.is_control())
            {
                out.push(s);
                i = start + len;
                continue;
            }
        }
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
[General]
QTWEBENGINE_CHROMIUM_FLAGS=--foo

[system]
theme=dark
quit_in_close=false
recent_downloads=/a/b.pdf, "/c/d, e.pdf"
spellCheckLanguages=@Variant(\0\0\0\t\0\0\0\x1\0\0\0\n\0\0\0\n\0p\0t\0_\0\x42\0R)
floating_monitoring_panel_position=@Point(10 20)

[permissions]
auto_grant\camera=true

[storage-whats]
user_agent=Linux Firefox
"#;

    #[test]
    fn parses_sections_and_types() {
        let ini = QSettingsIni::parse(SAMPLE);
        assert_eq!(ini.string("QTWEBENGINE_CHROMIUM_FLAGS").unwrap(), "--foo");
        assert_eq!(ini.string("system/theme").unwrap(), "dark");
        assert_eq!(ini.bool("system/quit_in_close"), Some(false));
        assert_eq!(ini.bool("permissions/auto_grant/camera"), Some(true));
        assert_eq!(
            ini.string("storage-whats/user_agent").unwrap(),
            "Linux Firefox"
        );
        assert_eq!(
            ini.list("system/recent_downloads").unwrap(),
            ["/a/b.pdf", "/c/d, e.pdf"]
        );
        assert_eq!(
            ini.point("system/floating_monitoring_panel_position"),
            Some((10, 20))
        );
    }

    #[test]
    fn decodes_variant_string_list() {
        let ini = QSettingsIni::parse(SAMPLE);
        assert_eq!(ini.list("system/spellCheckLanguages").unwrap(), ["pt_BR"]);
    }
}
