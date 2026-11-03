//! Clipboard helpers for "paste as plain text" (ZapZap's `clipboard_table.py`).

/// Read the clipboard as plain text, converting a copied spreadsheet table
/// into one line per row with a single space between columns.
pub fn read_plain_text() -> Option<String> {
    let mut clipboard = arboard::Clipboard::new().ok()?;
    if let Ok(html) = clipboard.get().html()
        && let Some(text) = table_as_plain_text(&html)
    {
        return Some(text);
    }
    clipboard.get_text().ok()
}

/// Convert the first top-level `<table>` of `html`. Returns `None` when there
/// is no table or the document is too large to parse on the UI thread.
pub fn table_as_plain_text(html: &str) -> Option<String> {
    if html.len() > 2_000_000 || !html.to_ascii_lowercase().contains("<table") {
        return None;
    }
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Option<Vec<String>> = None;
    let mut cell: Option<String> = None;
    let mut depth = 0usize;
    let mut seen = false;

    let mut rest = html;
    while !rest.is_empty() {
        match rest.find('<') {
            Some(0) => {
                // An unterminated tag ends the document, as in HTMLParser.
                let Some(close) = rest.find('>') else {
                    break;
                };
                let tag = &rest[1..close];
                rest = &rest[close + 1..];
                let closing = tag.starts_with('/');
                let name: String = tag
                    .trim_start_matches('/')
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric())
                    .collect::<String>()
                    .to_ascii_lowercase();
                match (name.as_str(), closing) {
                    ("table", false) => {
                        if !seen {
                            seen = true;
                            depth = 1;
                        } else if depth > 0 {
                            depth += 1;
                        }
                    }
                    ("table", true) => depth = depth.saturating_sub(1),
                    _ if depth != 1 => {}
                    ("tr", false) if row.is_none() => row = Some(Vec::new()),
                    ("td" | "th", false) if row.is_some() => cell = Some(String::new()),
                    ("br", false) => {
                        if let Some(c) = cell.as_mut() {
                            c.push('\n');
                        }
                    }
                    ("td" | "th", true) => {
                        if let (Some(c), Some(r)) = (cell.take(), row.as_mut()) {
                            r.push(normalize_cell(&c));
                        }
                    }
                    ("tr", true) => {
                        if let Some(r) = row.take() {
                            rows.push(r);
                        }
                    }
                    _ => {}
                }
            }
            Some(next) => {
                if depth == 1
                    && let Some(c) = cell.as_mut()
                {
                    c.push_str(&decode_entities(&rest[..next]));
                }
                rest = &rest[next..];
            }
            None => {
                if depth == 1
                    && let Some(c) = cell.as_mut()
                {
                    c.push_str(&decode_entities(rest));
                }
                rest = "";
            }
        }
    }
    if rows.is_empty() {
        return None;
    }
    Some(
        rows.iter()
            .map(|r| r.join(" "))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

fn normalize_cell(text: &str) -> String {
    let text = text.replace('\u{a0}', " ");
    let mut out = String::new();
    let mut pending_space = false;
    for c in text.chars() {
        if matches!(c, ' ' | '\t' | '\r' | '\x0c' | '\x0b') {
            pending_space = true;
        } else {
            if pending_space && !out.is_empty() && !out.ends_with('\n') && c != '\n' {
                out.push(' ');
            }
            pending_space = false;
            out.push(c);
        }
    }
    out.trim().to_owned()
}

fn decode_entities(text: &str) -> String {
    html_escape::decode_html_entities(text).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_first_table_rows() {
        let html = "<html><table><tr><td>a&nbsp;1</td><td>b</td></tr><tr></tr>\
                    <tr><th>c<br>d</th></tr></table><table><tr><td>x</td></tr></table>";
        assert_eq!(table_as_plain_text(html).unwrap(), "a 1 b\n\nc\nd");
    }

    #[test]
    fn nested_tables_are_ignored() {
        let html = "<table><tr><td>a<table><tr><td>inner</td></tr></table></td></tr></table>";
        assert_eq!(table_as_plain_text(html).unwrap(), "a");
    }

    #[test]
    fn decodes_named_and_numeric_entities() {
        let html = "<table><tr><td>&#8364;10 &euro; &#x41;&amp;&lt;</td></tr></table>";
        assert_eq!(table_as_plain_text(html).unwrap(), "€10 € A&<");
    }

    #[test]
    fn malformed_tags_do_not_panic() {
        let html = "<table><tr><td>x</td></tr></table><";
        assert_eq!(table_as_plain_text(html).unwrap(), "x");
        let html = "<table><tr><td>x</td></tr><té";
        assert_eq!(table_as_plain_text(html).unwrap(), "x");
    }

    #[test]
    fn non_tables_return_none() {
        assert!(table_as_plain_text("<p>hi</p>").is_none());
    }
}
