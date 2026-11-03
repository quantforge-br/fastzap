//! Minimal reader for Qt `QSettings` INI files as written by ZapZap.
//!
//! Values are unescaped like Qt's `iniUnescapedStringList` (quotes, `\` escapes,
//! comma-separated lists) and then typed like `stringToVariant` (`@@`,
//! `@Invalid()`, `@Point(x y)` and `@Variant(...)` with QDataStream payloads).

use std::collections::BTreeMap;

#[derive(Debug, Default)]
pub struct QSettingsIni {
    values: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq)]
enum IniValue {
    Single(String),
    List(Vec<String>),
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
            values.insert(full, value.to_owned());
        }
        Self { values }
    }

    pub fn contains(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.values.keys().map(String::as_str)
    }

    fn value(&self, key: &str) -> Option<IniValue> {
        self.values.get(key).map(|raw| unescape_value(raw))
    }

    /// A plain string value; typed values (`@Variant(...)` etc.) and lists yield `None`.
    pub fn string(&self, key: &str) -> Option<String> {
        match self.value(key)? {
            IniValue::Single(s) => plain_string(s),
            IniValue::List(_) => None,
        }
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
        match self.value(key)? {
            IniValue::List(items) => Some(items.into_iter().filter_map(plain_string).collect()),
            IniValue::Single(s) => {
                if let Some(bytes) = variant_bytes(&s) {
                    return Some(decode_variant_strings(&bytes));
                }
                if s.is_empty() || s == "@Invalid()" {
                    return Some(Vec::new());
                }
                Some(plain_string(s).into_iter().collect())
            }
        }
    }

    /// A `QVariantMap` of strings, as PyQt stores Python dicts.
    pub fn map(&self, key: &str) -> Option<BTreeMap<String, String>> {
        let IniValue::Single(s) = self.value(key)? else {
            return None;
        };
        let bytes = variant_bytes(&s)?;
        let mut reader = Reader::new(&bytes);
        match reader.variant()? {
            Variant::Map(entries) => Some(
                entries
                    .into_iter()
                    .filter_map(|(k, v)| Some((k, v.into_string()?)))
                    .collect(),
            ),
            _ => None,
        }
    }

    pub fn point(&self, key: &str) -> Option<(i32, i32)> {
        let IniValue::Single(s) = self.value(key)? else {
            return None;
        };
        let inner = s.strip_prefix("@Point(")?.strip_suffix(')')?;
        let mut parts = inner.split_whitespace();
        Some((parts.next()?.parse().ok()?, parts.next()?.parse().ok()?))
    }
}

/// Apply Qt's `stringToVariant` rules for strings: `@@x` is the literal `@x`,
/// other `@Type(...)` forms are typed values.
fn plain_string(s: String) -> Option<String> {
    if let Some(rest) = s.strip_prefix("@@") {
        return Some(format!("@{rest}"));
    }
    if let Some(inner) = s.strip_prefix("@String(").and_then(|r| r.strip_suffix(')')) {
        return Some(inner.to_owned());
    }
    if s.starts_with('@') && s.ends_with(')') {
        return None;
    }
    Some(s)
}

/// The QDataStream bytes of a `@Variant(...)` value (Latin-1 code points).
fn variant_bytes(s: &str) -> Option<Vec<u8>> {
    let inner = s.strip_prefix("@Variant(")?.strip_suffix(')')?;
    Some(inner.chars().map(|c| c as u32 as u8).collect())
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

fn push_code(out: &mut String, code: u32) {
    out.push(char::from_u32(code).unwrap_or(char::REPLACEMENT_CHARACTER));
}

fn chop_trailing_spaces(s: &mut String, limit: usize) {
    while s.len() > limit && (s.ends_with(' ') || s.ends_with('\t')) {
        s.pop();
    }
}

/// Port of Qt's `QSettingsPrivate::iniUnescapedStringList`.
fn unescape_value(raw: &str) -> IniValue {
    let chars: Vec<char> = raw.chars().collect();
    let mut list: Option<Vec<String>> = None;
    let mut current = String::new();
    let mut in_quotes = false;
    let mut quoted = false;
    let mut i = 0;

    let skip_spaces = |i: &mut usize| {
        while *i < chars.len() && (chars[*i] == ' ' || chars[*i] == '\t') {
            *i += 1;
        }
    };
    skip_spaces(&mut i);
    let mut chop_limit = 0;

    while i < chars.len() {
        match chars[i] {
            '\\' => {
                i += 1;
                let Some(&c) = chars.get(i) else { break };
                i += 1;
                let simple = match c {
                    'a' => Some('\u{7}'),
                    'b' => Some('\u{8}'),
                    'f' => Some('\u{c}'),
                    'n' => Some('\n'),
                    'r' => Some('\r'),
                    't' => Some('\t'),
                    'v' => Some('\u{b}'),
                    '"' | '?' | '\'' | '\\' => Some(c),
                    _ => None,
                };
                if let Some(ch) = simple {
                    current.push(ch);
                } else if c == 'x' {
                    let mut code = 0u32;
                    while let Some(d) = chars.get(i).and_then(|d| d.to_digit(16)) {
                        code = code.wrapping_mul(16).wrapping_add(d);
                        i += 1;
                    }
                    push_code(&mut current, code);
                } else if let Some(first) = c.to_digit(8) {
                    let mut code = first;
                    while let Some(d) = chars.get(i).and_then(|d| d.to_digit(8)) {
                        code = code.wrapping_mul(8).wrapping_add(d);
                        i += 1;
                    }
                    push_code(&mut current, code);
                }
                // Any other escaped character is dropped, as in Qt.
                chop_limit = current.len();
            }
            '"' => {
                i += 1;
                quoted = true;
                in_quotes = !in_quotes;
                if !in_quotes {
                    skip_spaces(&mut i);
                    chop_limit = current.len();
                }
            }
            ',' if !in_quotes => {
                if !quoted {
                    chop_trailing_spaces(&mut current, chop_limit);
                }
                list.get_or_insert_with(Vec::new)
                    .push(std::mem::take(&mut current));
                quoted = false;
                i += 1;
                skip_spaces(&mut i);
                chop_limit = 0;
            }
            c => {
                current.push(c);
                i += 1;
            }
        }
    }
    if !quoted {
        chop_trailing_spaces(&mut current, chop_limit);
    }
    match list {
        Some(mut items) => {
            items.push(current);
            IniValue::List(items)
        }
        None => IniValue::Single(current),
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Variant {
    String(Option<String>),
    StringList(Vec<String>),
    List(Vec<Variant>),
    Map(Vec<(String, Variant)>),
    Scalar(String),
    Null,
}

impl Variant {
    fn into_string(self) -> Option<String> {
        match self {
            Variant::String(s) => s,
            Variant::Scalar(s) => Some(s),
            _ => None,
        }
    }
}

/// Big-endian QDataStream reader for the QVariant types ZapZap stores.
struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let slice = self.bytes.get(self.pos..self.pos.checked_add(n)?)?;
        self.pos += n;
        Some(slice)
    }

    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }

    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_be_bytes(self.take(4)?.try_into().ok()?))
    }

    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_be_bytes(self.take(8)?.try_into().ok()?))
    }

    fn count(&mut self) -> Option<usize> {
        let n = self.u32()? as usize;
        // Every element takes at least 4 bytes; reject absurd counts early.
        (n <= self.bytes.len() / 4).then_some(n)
    }

    fn qstring(&mut self) -> Option<Option<String>> {
        let len = self.u32()?;
        if len == u32::MAX {
            return Some(None);
        }
        if len % 2 != 0 {
            return None;
        }
        let units: Vec<u16> = self
            .take(len as usize)?
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        Some(Some(String::from_utf16_lossy(&units)))
    }

    fn variant(&mut self) -> Option<Variant> {
        let type_id = self.u32()?;
        let _is_null = self.u8()?;
        Some(match type_id {
            0 => Variant::Null,
            1 => Variant::Scalar((self.u8()? != 0).to_string()),
            2 => Variant::Scalar((self.u32()? as i32).to_string()),
            3 => Variant::Scalar(self.u32()?.to_string()),
            4 => Variant::Scalar((self.u64()? as i64).to_string()),
            5 => Variant::Scalar(self.u64()?.to_string()),
            6 => Variant::Scalar(f64::from_bits(self.u64()?).to_string()),
            10 => Variant::String(self.qstring()?),
            11 => {
                let n = self.count()?;
                let mut items = Vec::with_capacity(n);
                for _ in 0..n {
                    items.push(self.qstring()?.unwrap_or_default());
                }
                Variant::StringList(items)
            }
            9 => {
                let n = self.count()?;
                let mut items = Vec::with_capacity(n);
                for _ in 0..n {
                    items.push(self.variant()?);
                }
                Variant::List(items)
            }
            8 => {
                let n = self.count()?;
                let mut entries = Vec::with_capacity(n);
                for _ in 0..n {
                    let key = self.qstring()?.unwrap_or_default();
                    entries.push((key, self.variant()?));
                }
                Variant::Map(entries)
            }
            _ => return None,
        })
    }
}

/// Strings of a serialized `QStringList`/`QVariantList`; falls back to a
/// heuristic scan for UTF-16BE strings when the payload is not understood.
fn decode_variant_strings(bytes: &[u8]) -> Vec<String> {
    match Reader::new(bytes).variant() {
        Some(Variant::StringList(items)) => items,
        Some(Variant::List(items)) => items.into_iter().filter_map(Variant::into_string).collect(),
        Some(Variant::String(Some(s))) => vec![s],
        _ => scan_utf16_strings(bytes),
    }
}

fn scan_utf16_strings(bytes: &[u8]) -> Vec<String> {
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
spellCheckLanguages=@Variant(\0\0\0\t\0\0\0\0\x1\0\0\0\n\0\0\0\0\n\0p\0t\0_\0\x42\0R)
quotedList="@Variant(\0\0\0\v\0\0\0\0\x1\0\0\0\x6\0\x61\0,\0\x62)"
floating_monitoring_panel_position=@Point(10 20)
at=@@home
escaped=C:\\Users\\me\tdir
spaced="  padded  "

[custom]
global\css\preview_map=@Variant(\0\0\0\b\0\0\0\0\x1\0\0\0\n\0\x61\0.\0\x63\0s\0s\0\0\0\n\0\0\0\0\n\0\x61\0.\0p\0n\0g)

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
    fn unescapes_like_qt() {
        let ini = QSettingsIni::parse(SAMPLE);
        assert_eq!(ini.string("system/at").unwrap(), "@home");
        assert_eq!(ini.string("system/escaped").unwrap(), "C:\\Users\\me\tdir");
        assert_eq!(ini.string("system/spaced").unwrap(), "  padded  ");
        assert_eq!(ini.list("system/at").unwrap(), ["@home"]);
    }

    #[test]
    fn decodes_variant_payloads() {
        let ini = QSettingsIni::parse(SAMPLE);
        assert_eq!(ini.list("system/spellCheckLanguages").unwrap(), ["pt_BR"]);
        assert_eq!(ini.list("system/quotedList").unwrap(), ["a,b"]);
        assert!(ini.string("system/spellCheckLanguages").is_none());
        let map = ini.map("custom/global/css/preview_map").unwrap();
        assert_eq!(map.get("a.css").map(String::as_str), Some("a.png"));
    }
}
