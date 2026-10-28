//! Espanso-style YAML match files (a v1 subset).
//!
//! Rules (D-05, D-07, D-11):
//! - Snippets come from `<config_dir>/match/*.yml|*.yaml`, read in file-name
//!   order, ignoring case (`a.yml` before `B.yml`), in plain character order
//!   (`10.yml` before `9.yml`).
//! - A YAML syntax or shape error fails the whole load ([`ConfigError`]), so the
//!   caller can keep the last good set.
//! - A match that uses a feature v1 lacks is skipped and reported as a
//!   [`Skipped`] entry; every other match in the file still loads.
//! - Unknown top-level keys are ignored.
//!
//! Nothing here touches the real config folder unless the caller passes it.

use serde::Deserialize;
use serde_norway::{Mapping, Value};
use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The first-run sample `base.yml`. CI ships the same file in the zip.
pub const SAMPLE_BASE_YML: &str = include_str!("../../assets/base.sample.yml");

/// Name of the sample file written on first run.
pub const SAMPLE_FILE_NAME: &str = "base.yml";

/// One trigger and what it expands to. A match with `triggers: [a, b]` becomes
/// one `Snippet` per trigger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snippet {
    pub trigger: String,
    pub replace: String,
    /// Fire only on a word boundary (D-08).
    pub word: bool,
    pub vars: Vec<Var>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Var {
    pub name: String,
    pub kind: VarKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VarKind {
    /// `type: date`. `format` is chrono strftime; `None` means RFC 2822.
    /// `offset` is seconds added to now.
    Date { format: Option<String>, offset: i64 },
    /// `type: clipboard`: current clipboard text.
    Clipboard,
}

/// Why a match was not loaded. Holds only config-derived names, never typed text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipReason {
    /// The match uses a key v1 does not support (`regex`, `form`, ...).
    UnsupportedKey(String),
    /// A variable has a `type` v1 does not support (`shell`, `echo`, ...).
    UnsupportedVarType(String),
    /// A date variable uses a parameter v1 does not support (`locale`, `tz`).
    UnsupportedVarParam(String),
    /// The trigger was already defined earlier; the first one wins.
    DuplicateTrigger,
    /// `trigger: ""` or an empty entry in `triggers`.
    EmptyTrigger,
    /// Neither `trigger` nor `triggers` is present.
    MissingTrigger,
    /// No `replace` text.
    MissingReplace,
    /// The trigger contains a tab, a line break or another control
    /// character (D-15; none of them can be typed into a trigger). Only that
    /// trigger is skipped.
    TriggerHasControlChar,
    /// The replacement contains a tab (D-15).
    ReplaceHasTab,
    /// A date variable's `format` would type a tab (a tab or `%t`, D-15).
    DateFormatHasTab,
    /// A known key has the wrong kind of value (the key name is given).
    BadValue(&'static str),
    /// A text key holds an unquoted number or boolean (`trigger: 1.10`).
    /// YAML would change its text (`1.1`), so it must be quoted.
    NeedsQuotes(&'static str),
}

impl fmt::Display for SkipReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SkipReason::UnsupportedKey(k) => write!(f, "unsupported key `{k}`"),
            SkipReason::UnsupportedVarType(t) => write!(f, "unsupported variable type `{t}`"),
            SkipReason::UnsupportedVarParam(p) => {
                write!(f, "unsupported date parameter `{p}`")
            }
            SkipReason::DuplicateTrigger => f.write_str("duplicate trigger (first one kept)"),
            SkipReason::EmptyTrigger => f.write_str("empty trigger"),
            SkipReason::MissingTrigger => f.write_str("no `trigger` or `triggers`"),
            SkipReason::MissingReplace => f.write_str("no `replace`"),
            SkipReason::TriggerHasControlChar => {
                f.write_str("triggers cannot contain a tab, a line break or a control character")
            }
            SkipReason::ReplaceHasTab => f.write_str("the replacement cannot contain a tab"),
            SkipReason::DateFormatHasTab => {
                f.write_str("a date `format` cannot contain a tab or `%t`")
            }
            SkipReason::BadValue(k) => write!(f, "invalid value for `{k}`"),
            SkipReason::NeedsQuotes(k) => write!(
                f,
                "`{k}` is a number or true/false; quote this value (\"...\") to use it as text"
            ),
        }
    }
}

/// A match (or one trigger of it) that was skipped while loading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    pub file: PathBuf,
    /// The trigger, if the match had one.
    pub trigger: Option<String>,
    pub reason: SkipReason,
}

/// A file that could not be read or parsed. `line` is 1-based.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    pub file: PathBuf,
    pub line: Option<usize>,
    pub msg: String,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.file.display())?;
        if let Some(line) = self.line {
            write!(f, ":{line}")?;
        }
        write!(f, ": {}", self.msg)
    }
}

impl std::error::Error for ConfigError {}

/// The result of a successful load: usable snippets plus skip warnings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Loaded {
    pub snippets: Vec<Snippet>,
    pub skipped: Vec<Skipped>,
}

/// `%APPDATA%\meh` on Windows (`dirs::config_dir()` + `meh`).
pub fn config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("meh"))
}

/// The folder holding match files, inside a config dir.
pub fn match_dir(config_dir: &Path) -> PathBuf {
    config_dir.join("match")
}

/// True for a file name meh loads: `*.yml` / `*.yaml` (any case). Hidden
/// files and editor temp/lock files (`.x.yml.swp`, `x.yml~`, `#x.yml#`,
/// `~$x.yml`, `.#x.yml`) do not qualify.
pub fn is_match_file_name(name: &str) -> bool {
    if name.starts_with(['.', '~', '#']) {
        return false;
    }
    match name.rsplit_once('.') {
        Some((stem, ext)) => {
            !stem.is_empty()
                && (ext.eq_ignore_ascii_case("yml") || ext.eq_ignore_ascii_case("yaml"))
        }
        None => false,
    }
}

/// Writes the sample `base.yml` on first run, i.e. when `match_dir` does not
/// exist yet. Never overwrites anything (D-05). Returns whether it wrote.
pub fn ensure_sample(match_dir: &Path) -> io::Result<bool> {
    if match_dir.exists() {
        return Ok(false);
    }
    fs::create_dir_all(match_dir)?;
    let path = match_dir.join(SAMPLE_FILE_NAME);
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(mut f) => {
            io::Write::write_all(&mut f, SAMPLE_BASE_YML.as_bytes())?;
            Ok(true)
        }
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(false),
        Err(e) => Err(e),
    }
}

/// Loads every match file in `dir`, in case-insensitive file-name order. All-or-nothing: the
/// first unreadable or unparsable file makes the whole load an error.
/// A missing folder loads as zero snippets.
pub fn load_dir(dir: &Path) -> Result<Loaded, ConfigError> {
    let io_err = |file: &Path, e: io::Error| ConfigError {
        file: file.to_path_buf(),
        line: None,
        msg: e.to_string(),
    };
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Loaded::default()),
        Err(e) => return Err(io_err(dir, e)),
    };
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| io_err(dir, e))?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !is_match_file_name(name) {
            continue;
        }
        let path = entry.path();
        if path.is_file() {
            files.push((name.to_string(), path));
        }
    }
    files.sort_by(|(a, _), (b, _)| file_order(a, b));

    let mut loaded = Loaded::default();
    for (_, path) in files {
        let src = fs::read_to_string(&path).map_err(|e| io_err(&path, e))?;
        parse_into(&mut loaded, &path, &src)?;
    }
    Ok(loaded)
}

/// Load order of match files: by lowercased name in plain character order
/// (so `10.yml` before `9.yml`, unlike Explorer's natural sort), then by
/// exact name so the order is total.
fn file_order(a: &str, b: &str) -> std::cmp::Ordering {
    a.to_lowercase()
        .cmp(&b.to_lowercase())
        .then_with(|| a.cmp(b))
}

#[derive(Deserialize)]
struct RawFile {
    /// Entries are checked one by one, so a bad entry skips only itself.
    #[serde(default)]
    matches: Option<Vec<Value>>,
}

/// Parses one file's text and appends its snippets and skip warnings to
/// `loaded`. On error, `loaded` is left unchanged.
pub fn parse_into(loaded: &mut Loaded, file: &Path, src: &str) -> Result<(), ConfigError> {
    let src = src.strip_prefix('\u{feff}').unwrap_or(src);
    let raw: RawFile = serde_norway::from_str(src).map_err(|e| ConfigError {
        file: file.to_path_buf(),
        line: e.location().map(|l| l.line()),
        msg: e.to_string(),
    })?;

    let mut seen: HashSet<String> = loaded.snippets.iter().map(|s| s.trigger.clone()).collect();
    for entry in raw.matches.unwrap_or_default() {
        let mut skip = |trigger: Option<String>, reason: SkipReason| {
            loaded.skipped.push(Skipped {
                file: file.to_path_buf(),
                trigger,
                reason,
            })
        };
        let Some(m) = entry.as_mapping() else {
            skip(None, SkipReason::BadValue("matches"));
            continue;
        };
        let parsed = match parse_match(m) {
            Ok(p) => p,
            Err((trigger, reason)) => {
                skip(trigger, reason);
                continue;
            }
        };
        for trigger in parsed.triggers {
            let trigger = match trigger {
                Ok(t) => t,
                Err((t, reason)) => {
                    skip(t, reason);
                    continue;
                }
            };
            if trigger.is_empty() {
                skip(None, SkipReason::EmptyTrigger);
            } else if trigger.chars().any(is_untypeable_in_trigger) {
                skip(
                    Some(escape_controls(&trigger)),
                    SkipReason::TriggerHasControlChar,
                );
            } else if !seen.insert(trigger.clone()) {
                skip(Some(trigger), SkipReason::DuplicateTrigger);
            } else {
                loaded.snippets.push(Snippet {
                    trigger,
                    replace: parsed.replace.clone(),
                    word: parsed.word,
                    vars: parsed.vars.clone(),
                });
            }
        }
    }
    Ok(())
}

/// D-15: a trigger may not contain a tab or a line break. Other control
/// characters (VT, FF, NEL, ...) can never be typed either, since the hook
/// turns them into resets, and U+2028/U+2029 are line breaks.
fn is_untypeable_in_trigger(c: char) -> bool {
    c.is_control() || matches!(c, '\u{2028}' | '\u{2029}')
}

/// Shows tabs and line breaks as `\t`, `\n`, `\r` and other untypeable
/// characters as `\u{..}`, so a reported trigger stays on one line of the
/// problems box.
fn escape_controls(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c if is_untypeable_in_trigger(c) => out.push_str(&c.escape_unicode().to_string()),
            c => out.push(c),
        }
    }
    out
}

struct ParsedMatch {
    /// One result per trigger entry: a bad entry skips only itself (D-15).
    triggers: Vec<Result<String, SkipErr>>,
    replace: String,
    word: bool,
    vars: Vec<Var>,
}

/// Keys v1 understands.
const SUPPORTED_KEYS: &[&str] = &["trigger", "triggers", "replace", "word", "vars"];
/// Keys that only affect espanso's search bar; harmless to ignore.
const IGNORED_KEYS: &[&str] = &["label", "search_terms"];

fn get<'a>(m: &'a Mapping, key: &str) -> Option<&'a Value> {
    m.get(key).filter(|v| !v.is_null())
}

/// Scalars (string, number, bool) as text. Used only where the exact text
/// does not matter (variable names, triggers named in a problem).
fn scalar_str(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// The text of a `trigger`, `triggers` entry or `replace`. Only YAML strings
/// qualify: an unquoted number or boolean has already lost its source text
/// (`1.10` parses as 1.1, `1e3` as 1000.0, and the YAML library keeps no
/// copy of the original), so it is rejected with a hint to quote it.
fn text_value(v: &Value, key: &'static str) -> Result<String, SkipReason> {
    match v {
        Value::String(s) => Ok(s.clone()),
        Value::Number(_) | Value::Bool(_) => Err(SkipReason::NeedsQuotes(key)),
        _ => Err(SkipReason::BadValue(key)),
    }
}

type SkipErr = (Option<String>, SkipReason);

/// A trigger as text for a problem: YAML's reading of an unquoted number
/// or boolean (`123`, `true`) is close enough to find the entry. Empty is
/// `None`, and line breaks are escaped.
fn trigger_label(v: &Value) -> Option<String> {
    scalar_str(v)
        .filter(|t| !t.is_empty())
        .map(|t| escape_controls(&t))
}

/// One `trigger` value or `triggers` entry. A null entry counts as empty.
fn trigger_entry(v: &Value, key: &'static str) -> Result<String, SkipErr> {
    if v.is_null() {
        return Ok(String::new());
    }
    text_value(v, key).map_err(|reason| (trigger_label(v), reason))
}

fn parse_match(m: &Mapping) -> Result<ParsedMatch, SkipErr> {
    // Best-effort trigger for reporting, before anything else can fail.
    let first_trigger = get(m, "trigger").and_then(trigger_label).or_else(|| {
        get(m, "triggers")
            .and_then(Value::as_sequence)
            .and_then(|s| s.iter().find_map(trigger_label))
    });
    let fail = |reason: SkipReason| (first_trigger.clone(), reason);

    for key in m.keys() {
        let Some(key) = key.as_str() else {
            return Err(fail(SkipReason::BadValue("match key")));
        };
        if !SUPPORTED_KEYS.contains(&key) && !IGNORED_KEYS.contains(&key) {
            return Err(fail(SkipReason::UnsupportedKey(key.to_string())));
        }
    }

    let mut triggers = Vec::new();
    if let Some(t) = get(m, "trigger") {
        triggers.push(trigger_entry(t, "trigger"));
    }
    if let Some(ts) = get(m, "triggers") {
        let seq = ts
            .as_sequence()
            .ok_or_else(|| fail(SkipReason::BadValue("triggers")))?;
        triggers.extend(seq.iter().map(|t| trigger_entry(t, "triggers")));
    }
    if triggers.is_empty() {
        return Err(fail(SkipReason::MissingTrigger));
    }

    let replace = match get(m, "replace") {
        Some(r) => text_value(r, "replace").map_err(fail)?,
        None => return Err(fail(SkipReason::MissingReplace)),
    };
    if replace.contains('\t') {
        return Err(fail(SkipReason::ReplaceHasTab));
    }

    let word = match get(m, "word") {
        None => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => return Err(fail(SkipReason::BadValue("word"))),
    };

    let mut vars = Vec::new();
    if let Some(vs) = get(m, "vars") {
        let seq = vs
            .as_sequence()
            .ok_or_else(|| fail(SkipReason::BadValue("vars")))?;
        for v in seq {
            vars.push(parse_var(v).map_err(fail)?);
        }
    }

    Ok(ParsedMatch {
        triggers,
        replace,
        word,
        vars,
    })
}

fn parse_var(v: &Value) -> Result<Var, SkipReason> {
    let m = v.as_mapping().ok_or(SkipReason::BadValue("vars"))?;
    let name = get(m, "name")
        .and_then(scalar_str)
        .filter(|n| !n.is_empty())
        .ok_or(SkipReason::BadValue("vars.name"))?;
    let ty = get(m, "type")
        .and_then(Value::as_str)
        .ok_or(SkipReason::BadValue("vars.type"))?;
    let params = match get(m, "params") {
        None => None,
        Some(p) => Some(p.as_mapping().ok_or(SkipReason::BadValue("vars.params"))?),
    };
    let kind = match ty {
        "date" => {
            let mut format = None;
            let mut offset = 0;
            for (k, val) in params.into_iter().flatten() {
                match k.as_str() {
                    Some("format") => {
                        let f = val.as_str().ok_or(SkipReason::BadValue("params.format"))?;
                        if date_format_types_tab(f) {
                            return Err(SkipReason::DateFormatHasTab);
                        }
                        format = Some(f.to_string());
                    }
                    Some("offset") => {
                        offset = val.as_i64().ok_or(SkipReason::BadValue("params.offset"))?;
                    }
                    Some(other) => return Err(SkipReason::UnsupportedVarParam(other.to_string())),
                    None => return Err(SkipReason::BadValue("vars.params")),
                }
            }
            VarKind::Date { format, offset }
        }
        "clipboard" => VarKind::Clipboard,
        other => return Err(SkipReason::UnsupportedVarType(other.to_string())),
    };
    Ok(Var { name, kind })
}

/// Whether a strftime `format` would type a tab (D-15): a literal tab, or
/// `%t` with any flags. Checked by formatting a fixed date, so the answer
/// is chrono's own. An invalid format is reported when it is expanded.
fn date_format_types_tab(format: &str) -> bool {
    use std::fmt::Write as _;
    let mut out = String::new();
    let when = chrono::DateTime::UNIX_EPOCH.fixed_offset();
    write!(out, "{}", when.format(format)).is_ok() && out.contains('\t')
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn parse(src: &str) -> Loaded {
        let mut loaded = Loaded::default();
        parse_into(&mut loaded, Path::new("base.yml"), src).expect("valid yaml");
        loaded
    }

    fn triggers(l: &Loaded) -> Vec<&str> {
        l.snippets.iter().map(|s| s.trigger.as_str()).collect()
    }

    #[test]
    fn parses_trigger_and_triggers_list() {
        let l = parse(
            r#"
matches:
  - trigger: ":hi"
    replace: "Hello"
  - triggers: [":ty", ":thx"]
    replace: "Thank you"
"#,
        );
        assert_eq!(triggers(&l), [":hi", ":ty", ":thx"]);
        assert_eq!(l.snippets[0].replace, "Hello");
        assert_eq!(l.snippets[1].replace, "Thank you");
        assert_eq!(l.snippets[2].replace, "Thank you");
        assert!(l.skipped.is_empty());
    }

    #[test]
    fn word_defaults_false_and_true_parses() {
        let l = parse(
            r#"
matches:
  - trigger: "a"
    replace: "x"
  - trigger: "brb"
    replace: "be right back"
    word: true
"#,
        );
        assert!(!l.snippets[0].word);
        assert!(l.snippets[1].word);
    }

    #[test]
    fn block_scalar_keeps_newlines() {
        let l = parse("matches:\n  - trigger: \":sig\"\n    replace: |\n      Best,\n      Name\n");
        assert_eq!(l.snippets[0].replace, "Best,\nName\n");
    }

    #[test]
    fn date_var_format_and_offset_parsed() {
        let l = parse(
            r#"
matches:
  - trigger: ":tm"
    replace: "{{d}}"
    vars:
      - name: d
        type: date
        params:
          format: "%Y-%m-%d"
          offset: -86400
  - trigger: ":now"
    replace: "{{n}}"
    vars:
      - name: n
        type: date
"#,
        );
        assert!(l.skipped.is_empty(), "{:?}", l.skipped);
        assert_eq!(
            l.snippets[0].vars,
            [Var {
                name: "d".into(),
                kind: VarKind::Date {
                    format: Some("%Y-%m-%d".into()),
                    offset: -86400
                }
            }]
        );
        assert_eq!(
            l.snippets[1].vars[0].kind,
            VarKind::Date {
                format: None,
                offset: 0
            }
        );
    }

    #[test]
    fn clipboard_var_parsed() {
        let l = parse(
            r#"
matches:
  - trigger: ":a"
    replace: "<a href='{{c}}'>$|$</a>"
    vars:
      - name: c
        type: clipboard
"#,
        );
        assert_eq!(
            l.snippets[0].vars,
            [Var {
                name: "c".into(),
                kind: VarKind::Clipboard
            }]
        );
    }

    #[test]
    fn unsupported_match_keys_skip_only_that_match_and_warn() {
        let l = parse(
            r#"
matches:
  - trigger: ":ok1"
    replace: "one"
  - regex: ":(?P<x>\\d+)"
    replace: "{{x}}"
  - trigger: ":form"
    form: "Hi [[name]]"
  - trigger: ":sh"
    replace: "x"
    shell: "echo hi"
  - trigger: "alh"
    replace: "although"
    propagate_case: true
  - trigger: ":ok2"
    replace: "two"
"#,
        );
        assert_eq!(triggers(&l), [":ok1", ":ok2"]);
        let reasons: Vec<_> = l.skipped.iter().map(|s| s.reason.clone()).collect();
        assert_eq!(
            reasons,
            [
                SkipReason::UnsupportedKey("regex".into()),
                SkipReason::UnsupportedKey("form".into()),
                SkipReason::UnsupportedKey("shell".into()),
                SkipReason::UnsupportedKey("propagate_case".into()),
            ]
        );
        assert_eq!(l.skipped[0].trigger, None);
        assert_eq!(l.skipped[1].trigger.as_deref(), Some(":form"));
        assert_eq!(l.skipped[0].file, Path::new("base.yml"));
    }

    #[test]
    fn unsupported_var_type_skips_match() {
        let l = parse(
            r#"
matches:
  - trigger: ":ip"
    replace: "{{ip}}"
    vars:
      - name: ip
        type: shell
        params: { cmd: "curl ifconfig.me" }
  - trigger: ":ok"
    replace: "ok"
"#,
        );
        assert_eq!(triggers(&l), [":ok"]);
        assert_eq!(l.skipped.len(), 1);
        assert_eq!(l.skipped[0].trigger.as_deref(), Some(":ip"));
        assert_eq!(
            l.skipped[0].reason,
            SkipReason::UnsupportedVarType("shell".into())
        );
    }

    #[test]
    fn unknown_top_level_key_ignored() {
        let l = parse(
            r#"
global_vars:
  - name: x
    type: echo
imports: ["other.yml"]
something_else: 3
matches:
  - trigger: ":hi"
    replace: "Hello"
"#,
        );
        assert_eq!(triggers(&l), [":hi"]);
        assert!(l.skipped.is_empty());
    }

    #[test]
    fn invalid_yaml_is_err_with_line() {
        let mut l = Loaded::default();
        let err = parse_into(
            &mut l,
            Path::new("bad.yml"),
            "matches:\n  - trigger: \":a\"\n    replace: \"x\"\n   bad: [unclosed\n",
        )
        .unwrap_err();
        assert_eq!(err.file, Path::new("bad.yml"));
        let line = err.line.expect("line number");
        assert!((3..=5).contains(&line), "line {line}");
        assert!(!err.msg.is_empty());
        assert!(l.snippets.is_empty());
    }

    #[test]
    fn duplicate_trigger_keeps_first_and_warns() {
        let l = parse(
            r#"
matches:
  - trigger: ":hi"
    replace: "first"
  - triggers: [":hi", ":hey"]
    replace: "second"
"#,
        );
        assert_eq!(triggers(&l), [":hi", ":hey"]);
        assert_eq!(l.snippets[0].replace, "first");
        assert_eq!(l.skipped.len(), 1);
        assert_eq!(l.skipped[0].trigger.as_deref(), Some(":hi"));
        assert_eq!(l.skipped[0].reason, SkipReason::DuplicateTrigger);
    }

    #[test]
    fn empty_trigger_skipped() {
        let l = parse(
            r#"
matches:
  - trigger: ""
    replace: "x"
  - triggers: ["", ":b"]
    replace: "y"
  - replace: "no trigger"
"#,
        );
        assert_eq!(triggers(&l), [":b"]);
        let reasons: Vec<_> = l.skipped.iter().map(|s| s.reason.clone()).collect();
        assert_eq!(
            reasons,
            [
                SkipReason::EmptyTrigger,
                SkipReason::EmptyTrigger,
                SkipReason::MissingTrigger
            ]
        );
    }

    #[test]
    fn trigger_with_tab_or_newline_skips_only_that_entry() {
        // D-15: such a trigger is not loaded; the other triggers of the
        // same `triggers:` list still are.
        let l = parse(
            "matches:\n  - triggers: [\":ok\", \"a\\tb\", \"c\\nd\", \"e\\rf\", \":ok2\"]\n    replace: \"x\"\n  - trigger: \"g\\th\"\n    replace: \"y\"\n  - trigger: \":z\"\n    replace: \"z\"\n",
        );
        assert_eq!(triggers(&l), [":ok", ":ok2", ":z"]);
        assert_eq!(l.skipped.len(), 4, "{:?}", l.skipped);
        assert!(
            l.skipped
                .iter()
                .all(|s| s.reason == SkipReason::TriggerHasControlChar),
            "{:?}",
            l.skipped
        );
        // Reported escaped, so the problems box stays one line per problem.
        assert_eq!(l.skipped[0].trigger.as_deref(), Some("a\\tb"));
        assert_eq!(l.skipped[1].trigger.as_deref(), Some("c\\nd"));
    }

    #[test]
    fn trigger_with_any_control_or_line_separator_skips_only_that_entry() {
        // NEL, VT, FF and other control characters can never be typed (the
        // hook turns them into resets), and U+2028/U+2029 are line breaks.
        let l = parse(
            "matches:\n  - triggers: [\":ok\", \"a\\x0bb\", \"a\\x0cb\", \"a\\x01b\", \"a\\x7fb\", \"a\\Nb\", \"a\\Lb\", \"a\\Pb\", \":ok2\"]\n    replace: \"x\"\n",
        );
        assert_eq!(triggers(&l), [":ok", ":ok2"]);
        let skipped: Vec<_> = l
            .skipped
            .iter()
            .map(|s| (s.trigger.as_deref(), s.reason.clone()))
            .collect();
        let r = SkipReason::TriggerHasControlChar;
        assert_eq!(
            skipped,
            [
                (Some("a\\u{b}b"), r.clone()),
                (Some("a\\u{c}b"), r.clone()),
                (Some("a\\u{1}b"), r.clone()),
                (Some("a\\u{7f}b"), r.clone()),
                (Some("a\\u{85}b"), r.clone()),
                (Some("a\\u{2028}b"), r.clone()),
                (Some("a\\u{2029}b"), r),
            ]
        );
    }

    #[test]
    fn replace_with_tab_skips_snippet() {
        let l = parse(
            "matches:\n  - triggers: [\":t1\", \":t2\"]\n    replace: \"a\\tb\"\n  - trigger: \":nl\"\n    replace: \"line1\\nline2\"\n",
        );
        // Newlines in the replacement are fine (D-15); tabs are not.
        assert_eq!(triggers(&l), [":nl"]);
        assert_eq!(l.snippets[0].replace, "line1\nline2");
        assert_eq!(l.skipped.len(), 1, "{:?}", l.skipped);
        assert_eq!(l.skipped[0].trigger.as_deref(), Some(":t1"));
        assert_eq!(l.skipped[0].reason, SkipReason::ReplaceHasTab);
    }

    #[test]
    fn date_format_that_types_a_tab_skips_snippet() {
        // D-15: tab is not allowed anywhere, so a date format that would type
        // one (a literal tab or `%t`) skips the snippet. `%%t` is a literal
        // `%t`, and `%n` is a line break, which is allowed. (Flagged forms
        // such as `%-t` are invalid in chrono and fail at expansion.)
        let src = "matches:\n  - trigger: \":d1\"\n    replace: \"{{d}}\"\n    vars:\n      - name: d\n        type: date\n        params: { format: \"%d\\t%m\" }\n  - trigger: \":d2\"\n    replace: \"{{d}}\"\n    vars:\n      - name: d\n        type: date\n        params: { format: \"%d%t%m\" }\n  - trigger: \":ok\"\n    replace: \"{{d}}\"\n    vars:\n      - name: d\n        type: date\n        params: { format: \"%d%%t%n%m\" }\n";
        let l = parse(src);
        assert_eq!(triggers(&l), [":ok"]);
        let skipped: Vec<_> = l
            .skipped
            .iter()
            .map(|s| (s.trigger.as_deref(), s.reason.clone()))
            .collect();
        assert_eq!(
            skipped,
            [
                (Some(":d1"), SkipReason::DateFormatHasTab),
                (Some(":d2"), SkipReason::DateFormatHasTab),
            ]
        );
    }

    #[test]
    fn non_mapping_match_entry_skips_only_that_entry() {
        let l = parse(
            r#"
matches:
  - trigger: ":a"
    replace: "x"
  - ":oops"
  - [1, 2]
  - 42
  - trigger: ":b"
    replace: "y"
"#,
        );
        assert_eq!(triggers(&l), [":a", ":b"]);
        assert_eq!(l.skipped.len(), 3, "{:?}", l.skipped);
        for s in &l.skipped {
            assert_eq!(s.reason, SkipReason::BadValue("matches"));
            assert_eq!(s.trigger, None);
        }
    }

    #[test]
    fn unquoted_number_or_bool_text_is_skipped_with_quote_hint() {
        // YAML would turn `1.10` into 1.1 and `1e3` into 1000.0, so such
        // values are not guessed at: the user is asked to quote them. The
        // problem shows the value as YAML read it, to find the entry.
        let l = parse(
            r#"
matches:
  - trigger: 123
    replace: "x"
  - trigger: ":n"
    replace: 1e3
  - triggers: [":ok", true]
    replace: "y"
  - trigger: ":q"
    replace: "1e3"
  - trigger: "2.50"
    replace: "yes"
  - trigger: 7
    replace: "z"
    regex: "x"
"#,
        );
        assert_eq!(triggers(&l), [":ok", ":q", "2.50"]);
        assert_eq!(l.snippets[1].replace, "1e3");
        assert_eq!(l.snippets[2].replace, "yes");
        let skipped: Vec<_> = l
            .skipped
            .iter()
            .map(|s| (s.trigger.as_deref(), s.reason.clone()))
            .collect();
        assert_eq!(
            skipped,
            [
                (Some("123"), SkipReason::NeedsQuotes("trigger")),
                (Some(":n"), SkipReason::NeedsQuotes("replace")),
                (Some("true"), SkipReason::NeedsQuotes("triggers")),
                (Some("7"), SkipReason::UnsupportedKey("regex".into())),
            ]
        );
        assert!(
            SkipReason::NeedsQuotes("replace")
                .to_string()
                .contains("quote this value")
        );
    }

    #[test]
    fn non_text_entry_in_triggers_skips_only_that_entry() {
        // Per entry, as for a tab or line break (D-15).
        let l = parse(
            r#"
matches:
  - triggers: [":a", 12, ~, ":b", [1], false, ":c"]
    replace: "x"
  - triggers: [12, ~]
    replace: "y"
"#,
        );
        assert_eq!(triggers(&l), [":a", ":b", ":c"]);
        let skipped: Vec<_> = l
            .skipped
            .iter()
            .map(|s| (s.trigger.as_deref(), s.reason.clone()))
            .collect();
        assert_eq!(
            skipped,
            [
                (Some("12"), SkipReason::NeedsQuotes("triggers")),
                (None, SkipReason::EmptyTrigger),
                (None, SkipReason::BadValue("triggers")),
                (Some("false"), SkipReason::NeedsQuotes("triggers")),
                (Some("12"), SkipReason::NeedsQuotes("triggers")),
                (None, SkipReason::EmptyTrigger),
            ]
        );
        // A bad snippet is still reported once, not per trigger entry.
        let l = parse("matches:\n  - triggers: [12, \":d\"]\n    replace: 5\n");
        assert!(l.snippets.is_empty());
        assert_eq!(l.skipped.len(), 1, "{:?}", l.skipped);
        assert_eq!(l.skipped[0].trigger.as_deref(), Some("12"));
        assert_eq!(l.skipped[0].reason, SkipReason::NeedsQuotes("replace"));
    }

    #[test]
    fn load_dir_reads_only_yml_yaml_sorted() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        let m = |t: &str| format!("matches:\n  - trigger: \"{t}\"\n    replace: \"x\"\n");
        fs::write(p.join("b.yaml"), m(":b")).unwrap();
        fs::write(p.join("a.yml"), m(":a")).unwrap();
        fs::write(p.join("c.txt"), m(":c")).unwrap();
        fs::write(p.join("d.yml.swp"), "not: [yaml").unwrap();
        fs::write(p.join("e.yml~"), "not: [yaml").unwrap();
        fs::write(p.join(".hidden.yml"), "not: [yaml").unwrap();
        fs::create_dir(p.join("sub.yml")).unwrap();
        fs::write(p.join("sub.yml").join("z.yml"), m(":z")).unwrap();
        let l = load_dir(p).unwrap();
        assert_eq!(triggers(&l), [":a", ":b"]);
    }

    #[test]
    fn load_dir_sorts_file_names_case_insensitively() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        fs::write(
            p.join("a.yml"),
            "matches:\n  - trigger: \":x\"\n    replace: \"from a\"\n",
        )
        .unwrap();
        fs::write(
            p.join("B.yml"),
            "matches:\n  - trigger: \":x\"\n    replace: \"from B\"\n",
        )
        .unwrap();
        let l = load_dir(p).unwrap();
        assert_eq!(l.snippets.len(), 1);
        assert_eq!(l.snippets[0].replace, "from a", "a.yml sorts before B.yml");
        assert_eq!(l.skipped[0].file, p.join("B.yml"));
    }

    #[test]
    fn file_order_key_breaks_ties_by_exact_name() {
        let mut names = vec!["b.yml", "A.yml", "a.yml", "B.yml", "c.yml"];
        names.sort_by(|x, y| file_order(x, y));
        assert_eq!(names, ["A.yml", "a.yml", "B.yml", "b.yml", "c.yml"]);
    }

    #[test]
    fn load_dir_any_bad_file_is_err() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        fs::write(
            p.join("a.yml"),
            "matches:\n  - trigger: \":a\"\n    replace: \"x\"\n",
        )
        .unwrap();
        fs::write(p.join("b.yml"), "matches: [unclosed\n").unwrap();
        fs::write(p.join("c.yml"), "matches: {oops\n").unwrap();
        let err = load_dir(p).unwrap_err();
        assert_eq!(err.file, p.join("b.yml"), "first bad file in sorted order");
    }

    #[test]
    fn sample_base_yml_parses_with_no_warnings() {
        let l = parse(SAMPLE_BASE_YML);
        assert!(l.skipped.is_empty(), "{:?}", l.skipped);
        assert!(l.snippets.iter().any(|s| s.word), "has a word trigger");
        assert!(
            l.snippets.iter().any(|s| s
                .vars
                .iter()
                .any(|v| matches!(v.kind, VarKind::Date { .. }))),
            "has a date var"
        );
        assert!(
            l.snippets.iter().any(|s| s.replace.contains('\n')),
            "has multi-line text"
        );
        assert!(
            l.snippets.iter().any(|s| s.replace.contains("$|$")),
            "has a cursor marker"
        );
        assert!(l.snippets.iter().any(|s| s.trigger == ":date"));
    }

    #[test]
    fn ensure_sample_creates_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let match_dir = dir.path().join("meh").join("match");
        assert!(ensure_sample(&match_dir).unwrap());
        let written = fs::read_to_string(match_dir.join("base.yml")).unwrap();
        assert_eq!(written, SAMPLE_BASE_YML);
        // Second call: already there, nothing to do.
        assert!(!ensure_sample(&match_dir).unwrap());
    }

    #[test]
    fn ensure_sample_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let match_dir = dir.path().join("match");
        fs::create_dir_all(&match_dir).unwrap();
        fs::write(match_dir.join("base.yml"), "matches: []\n").unwrap();
        assert!(!ensure_sample(&match_dir).unwrap());
        assert_eq!(
            fs::read_to_string(match_dir.join("base.yml")).unwrap(),
            "matches: []\n"
        );
        // A user who keeps only other match files does not get base.yml back.
        fs::remove_file(match_dir.join("base.yml")).unwrap();
        fs::write(match_dir.join("work.yml"), "matches: []\n").unwrap();
        assert!(!ensure_sample(&match_dir).unwrap());
        assert!(!match_dir.join("base.yml").exists());
    }

    #[test]
    fn config_dir_ends_with_meh() {
        if let Some(d) = config_dir() {
            assert!(d.ends_with("meh"));
            assert!(match_dir(&d).ends_with(Path::new("meh").join("match")));
        }
    }

    #[test]
    fn utf8_bom_is_accepted() {
        let l = parse("\u{feff}matches:\n  - trigger: \":e\"\n    replace: \"\u{20ac}\"\n");
        assert_eq!(triggers(&l), [":e"]);
        assert_eq!(l.snippets[0].replace, "\u{20ac}");
    }

    #[test]
    fn empty_or_comment_only_file_is_ok() {
        assert!(parse("").snippets.is_empty());
        assert!(parse("# nothing yet\n").snippets.is_empty());
        assert!(parse("matches:\n").snippets.is_empty());
    }

    #[test]
    fn load_dir_missing_dir_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let l = load_dir(&dir.path().join("nope")).unwrap();
        assert!(l.snippets.is_empty() && l.skipped.is_empty());
    }

    #[test]
    fn duplicate_across_files_keeps_first_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        fs::write(
            p.join("a.yml"),
            "matches:\n  - trigger: \":x\"\n    replace: \"from a\"\n",
        )
        .unwrap();
        fs::write(
            p.join("b.yml"),
            "matches:\n  - trigger: \":x\"\n    replace: \"from b\"\n",
        )
        .unwrap();
        let l = load_dir(p).unwrap();
        assert_eq!(l.snippets.len(), 1);
        assert_eq!(l.snippets[0].replace, "from a");
        assert_eq!(l.skipped[0].file, p.join("b.yml"));
        assert_eq!(l.skipped[0].reason, SkipReason::DuplicateTrigger);
    }
}
