use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub const LANGUAGES: &[(&str, &str)] = &[
    ("auto", "Automatic"),
    ("en", "English"),
    ("de", "Deutsch"),
    ("es", "Español"),
    ("fr", "Français"),
    ("it", "Italiano"),
    ("ja", "日本語"),
    ("nl", "Nederlands"),
    ("pl", "Polski"),
    ("pt", "Português"),
    ("ru", "Русский"),
    ("zh_CN", "中文（简体）"),
];

#[derive(Debug, Clone)]
pub struct Translator {
    messages: HashMap<String, String>,
}

impl Translator {
    pub fn load(requested: &str) -> Self {
        let code = resolve_language(requested);
        if code == "en" {
            return Self {
                messages: HashMap::new(),
            };
        }

        let messages = po_path(&code)
            .and_then(|path| fs::read_to_string(path).ok())
            .map(|text| parse_po(&text))
            .unwrap_or_default();
        Self { messages }
    }

    pub fn tr(&self, message: &str) -> String {
        self.messages
            .get(message)
            .filter(|value| !value.is_empty())
            .cloned()
            .unwrap_or_else(|| message.to_string())
    }
}

pub fn resolve_language(requested: &str) -> String {
    if requested == "auto" {
        return system_language()
            .filter(|code| shipped_language(code))
            .unwrap_or_else(|| "en".to_string());
    }

    if shipped_language(requested) {
        requested.to_string()
    } else {
        "en".to_string()
    }
}

fn shipped_language(code: &str) -> bool {
    LANGUAGES
        .iter()
        .any(|(candidate, _)| *candidate == code && code != "auto")
}

fn system_language() -> Option<String> {
    for key in ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"] {
        let Some(value) = std::env::var(key).ok() else {
            continue;
        };
        if let Some(code) = normalize_language(&value) {
            return Some(code);
        }
    }
    None
}

fn normalize_language(value: &str) -> Option<String> {
    let code = value
        .split(':')
        .next()
        .unwrap_or("")
        .split('.')
        .next()
        .unwrap_or("")
        .trim();
    if code.is_empty() || code.eq_ignore_ascii_case("c") || code.eq_ignore_ascii_case("posix") {
        return None;
    }
    if code.eq_ignore_ascii_case("zh_cn") || code.eq_ignore_ascii_case("zh-CN") {
        return Some("zh_CN".to_string());
    }
    Some(code.split(['_', '-']).next().unwrap_or(code).to_lowercase())
}

fn po_path(code: &str) -> Option<PathBuf> {
    let file = format!("{code}.po");

    // Installed layout: the executable lives in <root>/bin/ (or <root>/) with
    // the po folder shipped alongside at <root>/po/.
    if let Ok(exe) = std::env::current_exe() {
        for ancestor in exe.ancestors().skip(1).take(3) {
            let candidate = ancestor.join("po").join(&file);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    // Development layout: the workspace root, two levels above this crate.
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .map(PathBuf::from)?;
    Some(root.join("po").join(file))
}

fn parse_po(text: &str) -> HashMap<String, String> {
    let mut messages = HashMap::new();
    let mut msgid = String::new();
    let mut msgstr = String::new();
    let mut section = PoSection::None;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("msgid ") {
            commit_message(&mut messages, &mut msgid, &mut msgstr);
            msgid = parse_po_string(trimmed.trim_start_matches("msgid ").trim());
            msgstr.clear();
            section = PoSection::MsgId;
        } else if trimmed.starts_with("msgstr ") {
            msgstr = parse_po_string(trimmed.trim_start_matches("msgstr ").trim());
            section = PoSection::MsgStr;
        } else if trimmed.starts_with('"') {
            match section {
                PoSection::MsgId => msgid.push_str(&parse_po_string(trimmed)),
                PoSection::MsgStr => msgstr.push_str(&parse_po_string(trimmed)),
                PoSection::None => {}
            }
        } else if trimmed.is_empty() {
            commit_message(&mut messages, &mut msgid, &mut msgstr);
            section = PoSection::None;
        }
    }
    commit_message(&mut messages, &mut msgid, &mut msgstr);
    messages
}

fn commit_message(messages: &mut HashMap<String, String>, msgid: &mut String, msgstr: &mut String) {
    if !msgid.is_empty() && !msgstr.is_empty() {
        messages.insert(std::mem::take(msgid), std::mem::take(msgstr));
    } else {
        msgid.clear();
        msgstr.clear();
    }
}

fn parse_po_string(value: &str) -> String {
    let inner = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value);
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

#[derive(Debug, Clone, Copy)]
enum PoSection {
    None,
    MsgId,
    MsgStr,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_po_entries() {
        let text = r#"
msgid "Identify"
msgstr "Identifica"

msgid ""
"No "
"reply."
msgstr ""
"Nessuna "
"risposta."
"#;
        let messages = parse_po(text);
        assert_eq!(messages.get("Identify").unwrap(), "Identifica");
        assert_eq!(messages.get("No reply.").unwrap(), "Nessuna risposta.");
    }

    #[test]
    fn normalizes_system_language() {
        assert_eq!(normalize_language("it_IT.UTF-8").unwrap(), "it");
        assert_eq!(normalize_language("zh_CN.UTF-8").unwrap(), "zh_CN");
        assert!(normalize_language("C").is_none());
    }
}
