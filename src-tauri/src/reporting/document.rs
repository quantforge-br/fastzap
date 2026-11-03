//! Report documents (ZapZap's schema, `application: "fastzap"`) and their
//! English Markdown for GitHub issues.

use serde::Deserialize;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use ts_rs::TS;

use super::sanitize;
use crate::build_info;

pub const SCHEMA_VERSION: u32 = 1;
pub const MANUAL: &str = "manual_problem";
pub const CRASH: &str = "automatic_crash";
pub const UNEXPECTED_SHUTDOWN: &str = "unexpected_shutdown";

/// Stable ids and their English labels.
pub const CATEGORIES: &[(&str, &str)] = &[
    ("closed_unexpectedly", "FastZap closed unexpectedly"),
    ("feature_not_working", "Something is not working"),
    ("notifications", "Notification problem"),
    ("audio_video", "Audio or video problem"),
    ("files", "File problem"),
    ("visual", "Visual problem"),
    ("other", "Other"),
];
pub const FREQUENCIES: &[(&str, &str)] = &[
    ("always", "Always"),
    ("sometimes", "Sometimes"),
    ("once", "It happened once"),
    ("unknown", "I don't know"),
];

const MAX_LOGS: usize = 16_000;
const MAX_DETAILS: usize = 24_000;

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct ProblemForm {
    pub category: String,
    pub description: String,
    pub expected: String,
    pub frequency: String,
    pub include_system: bool,
    pub include_error: bool,
    pub include_logs: bool,
}

pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn tail(text: &str, max: usize) -> String {
    let start = text.len().saturating_sub(max);
    let start = (start..=text.len())
        .find(|i| text.is_char_boundary(*i))
        .unwrap_or(text.len());
    text[start..].to_owned()
}

fn read_trimmed(path: &str) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

fn os_release(path: &str) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    text.lines()
        .find_map(|l| l.strip_prefix("PRETTY_NAME="))
        .map(|v| v.trim_matches('"').to_owned())
}

pub fn system_information() -> Value {
    let (os, source) = if let Some(host) = os_release("/run/host/os-release") {
        (host, "host os-release")
    } else if let Some(local) = os_release("/etc/os-release") {
        (local, "os-release")
    } else {
        (std::env::consts::OS.to_owned(), "platform API")
    };
    json!({
        "fastzap_version": env!("CARGO_PKG_VERSION"),
        "package_type": build_info::PACKAGING,
        "build_channel": build_info::CHANNEL,
        "kernel_version": read_trimmed("/proc/sys/kernel/osrelease").unwrap_or_default(),
        "operating_system": os,
        "operating_system_source": source,
        "desktop_environment": std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
        "session_type": std::env::var("XDG_SESSION_TYPE").unwrap_or_default(),
        "architecture": std::env::consts::ARCH,
        "webview_version": tauri::webview_version().unwrap_or_default(),
        "flatpak": crate::platform::is_flatpak(),
    })
}

/// Error details of a panic.
pub fn error_information(kind: &str, message: &str, details: &str) -> Value {
    json!({
        "type": kind,
        "message": message,
        "details": tail(details, MAX_DETAILS),
    })
}

pub fn fingerprint(parts: &[&str]) -> String {
    hex::encode(Sha256::digest(parts.join("\n").as_bytes()))
}

fn base(report_type: &str) -> Map<String, Value> {
    let mut doc = Map::new();
    doc.insert("schema_version".into(), SCHEMA_VERSION.into());
    doc.insert("application".into(), "fastzap".into());
    doc.insert("report_type".into(), report_type.into());
    doc.insert("timestamp".into(), now().into());
    doc
}

pub fn manual(form: &ProblemForm, error: Option<Value>, logs: &str) -> Value {
    let pick = |list: &[(&str, &str)], v: &str, fallback: &str| {
        if list.iter().any(|(id, _)| *id == v) {
            v.to_owned()
        } else {
            fallback.to_owned()
        }
    };
    let mut doc = base(MANUAL);
    doc.insert(
        "problem_category".into(),
        pick(CATEGORIES, &form.category, "other").into(),
    );
    doc.insert("user_description".into(), form.description.trim().into());
    doc.insert("expected_behavior".into(), form.expected.trim().into());
    doc.insert(
        "frequency".into(),
        pick(FREQUENCIES, &form.frequency, "unknown").into(),
    );
    if form.include_system {
        doc.insert("system_information".into(), system_information());
    }
    if form.include_error
        && let Some(error) = error
    {
        doc.insert("error_information".into(), error);
    }
    if form.include_logs && !logs.trim().is_empty() {
        doc.insert("log_scope".into(), "current-session".into());
        doc.insert("sanitized_logs".into(), tail(logs, MAX_LOGS).into());
    }
    sanitize::value(&Value::Object(doc))
}

pub fn crash(error: Value, fingerprint: String) -> Value {
    let mut doc = base(CRASH);
    doc.insert("severity".into(), "fatal_crash".into());
    doc.insert("failure_scope".into(), "current-session-panic".into());
    doc.insert("system_information".into(), system_information());
    doc.insert("error_information".into(), error);
    doc.insert("fingerprint".into(), fingerprint.into());
    sanitize::value(&Value::Object(doc))
}

/// A previous session that left its marker behind. Its own system
/// information is used, never the current runtime's.
pub fn unexpected_shutdown(marker: &Value) -> Value {
    let mut doc = base(UNEXPECTED_SHUTDOWN);
    doc.insert("severity".into(), "unexpected_termination".into());
    doc.insert("failure_scope".into(), "previous-session".into());
    doc.insert("termination_evidence".into(), "unclean-marker-only".into());
    doc.insert(
        "investigation_note".into(),
        "An unclean marker does not distinguish crash, forced kill or power loss.".into(),
    );
    if let Some(info) = marker.get("system_information") {
        doc.insert("system_information".into(), info.clone());
    }
    doc.insert(
        "session_information".into(),
        json!({
            "session_id": marker.get("session_id"),
            "started_at": marker.get("started_at"),
            "phase": marker.get("phase"),
        }),
    );
    doc.insert(
        "error_information".into(),
        json!({
            "type": "UnexpectedTermination",
            "message": "The previous FastZap process did not finish cleanly.",
        }),
    );
    sanitize::value(&Value::Object(doc))
}

fn label<'a>(list: &[(&str, &'a str)], id: &str) -> Option<&'a str> {
    list.iter().find(|(k, _)| *k == id).map(|(_, v)| *v)
}

fn str_of<'a>(doc: &'a Value, key: &str) -> Option<&'a str> {
    doc.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
}

pub fn issue_title(doc: &Value) -> String {
    match str_of(doc, "report_type") {
        Some(CRASH | UNEXPECTED_SHUTDOWN) => format!(
            "Unexpected closing: {}",
            doc.pointer("/error_information/type")
                .and_then(Value::as_str)
                .unwrap_or("FastZap")
        ),
        _ => str_of(doc, "problem_category")
            .and_then(|c| label(CATEGORIES, c))
            .map(|l| format!("Problem report: {l}"))
            .unwrap_or_else(|| "Problem report".into()),
    }
}

pub fn issue_url(doc: &Value) -> String {
    let title: String = url::form_urlencoded::byte_serialize(issue_title(doc).as_bytes()).collect();
    format!(
        "https://github.com/{}/issues/new?title={title}",
        build_info::OFFICIAL_REPOSITORY
    )
}

fn pre(text: &str) -> String {
    format!("<pre>{}</pre>", html_escape::encode_text(text))
}

/// The issue body; English only, like ZapZap's.
pub fn markdown(doc: &Value) -> String {
    let mut out = vec!["## Problem report".to_owned()];
    let mut section = |title: &str, body: String| {
        if !body.trim().is_empty() {
            out.push(format!("### {title}\n\n{body}"));
        }
    };
    let category = str_of(doc, "problem_category")
        .and_then(|c| label(CATEGORIES, c))
        .unwrap_or(match str_of(doc, "report_type") {
            Some(MANUAL) | None => "",
            _ => "Unexpected closing",
        });
    section("Problem type", category.into());
    section(
        "What happened",
        str_of(doc, "user_description").unwrap_or("").into(),
    );
    section(
        "Expected behavior",
        str_of(doc, "expected_behavior").unwrap_or("").into(),
    );
    section(
        "Frequency",
        str_of(doc, "frequency")
            .and_then(|f| label(FREQUENCIES, f))
            .unwrap_or("")
            .into(),
    );
    if let Some(info) = doc.get("system_information").and_then(Value::as_object) {
        let rows = [
            ("FastZap", "fastzap_version"),
            ("Package", "package_type"),
            ("Channel", "build_channel"),
            ("Kernel", "kernel_version"),
            ("Operating system", "operating_system"),
            ("OS information source", "operating_system_source"),
            ("Desktop", "desktop_environment"),
            ("Session", "session_type"),
            ("Architecture", "architecture"),
            ("Web view", "webview_version"),
            ("Flatpak", "flatpak"),
        ];
        let body = rows
            .iter()
            .filter_map(|(label, key)| {
                let value = match info.get(*key)? {
                    Value::String(s) if !s.is_empty() => s.clone(),
                    Value::Bool(b) => b.to_string(),
                    _ => return None,
                };
                Some(format!("- **{label}:** {value}"))
            })
            .collect::<Vec<_>>()
            .join("\n");
        section("Environment", body);
    }
    if let Some(session) = doc.get("session_information") {
        let body = ["session_id", "started_at", "phase"]
            .iter()
            .filter_map(|k| Some(format!("- **{k}:** {}", session.get(*k)?.as_str()?)))
            .collect::<Vec<_>>()
            .join("\n");
        section("Failed session", body);
    }
    if let Some(scope) = str_of(doc, "failure_scope") {
        let mut body = format!("**Scope:** {scope}");
        if let Some(evidence) = str_of(doc, "termination_evidence") {
            body.push_str(&format!("\n**Evidence:** {evidence}"));
        }
        if let Some(note) = str_of(doc, "investigation_note") {
            body.push_str(&format!("\n\n{note}"));
        }
        section("Failure evidence", body);
    }
    if let Some(error) = doc.get("error_information") {
        let mut body = String::new();
        for (label, key) in [("Type", "type"), ("Message", "message")] {
            if let Some(v) = error.get(key).and_then(Value::as_str) {
                body.push_str(&format!("**{label}:** {v}\n\n"));
            }
        }
        if let Some(details) = error.get("details").and_then(Value::as_str) {
            body.push_str(&pre(details));
        }
        out.push(format!(
            "<details><summary>Technical error information</summary>\n\n{}\n\n</details>",
            body.trim_end()
        ));
    }
    if let Some(logs) = str_of(doc, "sanitized_logs") {
        out.push(format!(
            "<details><summary>Sanitized logs</summary>\n\nWarnings and errors of the current session; not evidence of a failure.\n\n{}\n\n</details>",
            pre(logs)
        ));
    }
    if let Some(fp) = str_of(doc, "fingerprint") {
        out.push(format!("**Error fingerprint:** `{fp}`"));
    }
    out.push(
        "### Additional information needed\n\nProvide reproduction steps, observed event time and timezone, and comparisons with a browser or another environment. Do not attach private conversations, QR codes or media."
            .into(),
    );
    out.push("---\nPrepared locally by FastZap after review by the user.".into());
    out.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form() -> ProblemForm {
        ProblemForm {
            category: "notifications".into(),
            description: "No sound for user@example.com".into(),
            expected: "A sound".into(),
            frequency: "always".into(),
            include_system: true,
            include_error: true,
            include_logs: true,
        }
    }

    #[test]
    fn manual_reports_are_sanitized_and_rendered() {
        let doc = manual(&form(), None, "WARN something at /home/bob/file");
        assert_eq!(doc["report_type"], MANUAL);
        assert_eq!(doc["user_description"], "No sound for [removed]");
        assert!(
            doc["sanitized_logs"]
                .as_str()
                .unwrap()
                .contains("$HOME/file")
        );
        assert_eq!(
            doc["system_information"]["fastzap_version"],
            env!("CARGO_PKG_VERSION")
        );
        let md = markdown(&doc);
        assert!(md.starts_with("## Problem report"));
        assert!(md.contains("### Problem type\n\nNotification problem"));
        assert!(md.contains("### Frequency\n\nAlways"));
        assert!(md.contains("- **FastZap:**"));
        assert!(md.ends_with("Prepared locally by FastZap after review by the user."));
        assert_eq!(issue_title(&doc), "Problem report: Notification problem");
        assert!(
            issue_url(&doc).ends_with("/issues/new?title=Problem+report%3A+Notification+problem")
        );
    }

    #[test]
    fn unknown_ids_fall_back_and_options_are_honored() {
        let mut f = form();
        f.category = "<script>".into();
        f.frequency = "never".into();
        f.include_system = false;
        f.include_logs = false;
        let doc = manual(&f, Some(error_information("E", "m", "d")), "log");
        assert_eq!(doc["problem_category"], "other");
        assert_eq!(doc["frequency"], "unknown");
        assert!(doc.get("system_information").is_none());
        assert!(doc.get("sanitized_logs").is_none());
        assert_eq!(doc["error_information"]["type"], "E");
    }

    #[test]
    fn crash_reports_escape_details() {
        let doc = crash(
            error_information("panic", "boom", "<b>at main</b>"),
            fingerprint(&["a"]),
        );
        let md = markdown(&doc);
        assert!(md.contains("&lt;b&gt;at main&lt;/b&gt;"));
        assert!(md.contains("### Problem type\n\nUnexpected closing"));
        assert_eq!(issue_title(&doc), "Unexpected closing: panic");
        let marker = json!({"session_id": "ab", "phase": "running", "system_information": {"fastzap_version": "0.0.9"}});
        let previous = unexpected_shutdown(&marker);
        assert_eq!(previous["system_information"]["fastzap_version"], "0.0.9");
        assert!(markdown(&previous).contains("### Failed session"));
    }
}
