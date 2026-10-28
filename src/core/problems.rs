//! In-memory list of problems (D-12: no log file).
//!
//! Every variant is built from config- or expansion-derived values: file
//! paths, line numbers, YAML parser messages, trigger names and variable
//! names from the user's own config, plus OS error messages for the file
//! watcher and the config folder. None carries typed text or buffer
//! contents.

use crate::core::APP_NAME;
use crate::core::config::{ConfigError, SkipReason, Skipped, Snippet};
use crate::core::expand::ExpandError;
use std::fmt;
use std::path::PathBuf;

/// Cap on stored problems; older expansion failures are dropped first.
pub const MAX_PROBLEMS: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// A match file could not be read or parsed; nothing from this reload
    /// was applied (D-07). `msg` is the YAML parser's message.
    ConfigError {
        file: PathBuf,
        line: Option<usize>,
        msg: String,
    },
    /// A match was skipped while loading (D-11).
    SkippedSnippet {
        file: PathBuf,
        trigger: Option<String>,
        reason: SkipReason,
    },
    /// A trigger fired but its expansion failed, so nothing was typed.
    ExpandFailed {
        trigger: String,
        reason: ExpandError,
    },
    /// The match folder could not be watched, so saving a file does not
    /// reload (D-10); the tray "Reload snippets" still works. `error` is the
    /// watcher's (OS) error message.
    LiveReloadUnavailable { error: String },
    /// The config folder could not be created at startup (D-18), or
    /// `%APPDATA%` could not be found (`path` is `None`). `msg` is the OS
    /// error message.
    ConfigFolder { path: Option<PathBuf>, msg: String },
}

impl Problem {
    /// An expansion failure for `snippet`; records its trigger only.
    pub fn expand_failed(snippet: &Snippet, reason: ExpandError) -> Self {
        Problem::ExpandFailed {
            trigger: snippet.trigger.clone(),
            reason,
        }
    }

    /// `%APPDATA%` (the config folder's parent) could not be found.
    pub fn config_folder_unknown() -> Self {
        Problem::ConfigFolder {
            path: None,
            msg: "%APPDATA% is not set".to_string(),
        }
    }
}

impl From<ConfigError> for Problem {
    fn from(e: ConfigError) -> Self {
        Problem::ConfigError {
            file: e.file,
            line: e.line,
            msg: e.msg,
        }
    }
}

impl From<Skipped> for Problem {
    fn from(s: Skipped) -> Self {
        Problem::SkippedSnippet {
            file: s.file,
            trigger: s.trigger,
            reason: s.reason,
        }
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Problem::ConfigError { file, line, msg } => {
                write!(f, "Config error in {}", file.display())?;
                if let Some(line) = line {
                    write!(f, " (line {line})")?;
                }
                write!(f, ": {msg}")
            }
            Problem::SkippedSnippet {
                file,
                trigger,
                reason,
            } => {
                f.write_str("Skipped ")?;
                match trigger {
                    Some(t) => write!(f, "`{t}`")?,
                    None => f.write_str("a snippet")?,
                }
                write!(f, " in {}: {reason}", file.display())
            }
            Problem::ExpandFailed { trigger, reason } => {
                write!(f, "Could not expand `{trigger}`: {reason}")
            }
            Problem::LiveReloadUnavailable { error } => {
                write!(
                    f,
                    "live reload unavailable: {error} (use \"Reload snippets\" after saving)"
                )
            }
            Problem::ConfigFolder {
                path: Some(path),
                msg,
            } => write!(
                f,
                "Could not create the config folder {}: {msg}",
                path.display()
            ),
            Problem::ConfigFolder { path: None, msg } => write!(
                f,
                "Could not find the config folder ({msg}), so no snippets are loaded"
            ),
        }
    }
}

/// Counts by kind, for the tray tooltip and notifications.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Summary {
    pub config_errors: usize,
    pub skipped: usize,
    pub expand_failed: usize,
    pub live_reload: usize,
    pub config_folder: usize,
}

impl Summary {
    pub fn total(&self) -> usize {
        self.config_errors
            + self.skipped
            + self.expand_failed
            + self.live_reload
            + self.config_folder
    }
}

/// The current problems. Replaced on every reload; expansion failures are
/// added as they happen. Standing problems (the file watcher, the config
/// folder) are listed first and survive reloads.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Problems {
    standing: Vec<Problem>,
    items: Vec<Problem>,
}

impl Problems {
    /// Replaces the list (a reload happened). Standing problems stay.
    pub fn replace(&mut self, items: Vec<Problem>) {
        self.items = items;
        self.items.truncate(MAX_PROBLEMS);
    }

    /// Adds a problem that no reload clears, unless one of the same kind is
    /// already there. Returns whether it was added.
    pub fn set_standing(&mut self, p: Problem) -> bool {
        let same_kind = |q: &Problem| std::mem::discriminant(q) == std::mem::discriminant(&p);
        if self.standing.iter().any(same_kind) {
            return false;
        }
        self.standing.push(p);
        true
    }

    /// The number of standing problems.
    pub fn standing_len(&self) -> usize {
        self.standing.len()
    }

    /// Adds one problem unless an identical one is already listed.
    pub fn push(&mut self, p: Problem) {
        if self.items.contains(&p) {
            return;
        }
        if self.items.len() >= MAX_PROBLEMS {
            let oldest_runtime = self
                .items
                .iter()
                .position(|p| matches!(p, Problem::ExpandFailed { .. }));
            match oldest_runtime {
                Some(i) => {
                    self.items.remove(i);
                }
                None => return,
            }
        }
        self.items.push(p);
    }

    pub fn len(&self) -> usize {
        self.standing.len() + self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = &Problem> {
        self.standing.iter().chain(&self.items)
    }

    pub fn summary(&self) -> Summary {
        let mut s = Summary::default();
        for p in self.iter() {
            match p {
                Problem::ConfigError { .. } => s.config_errors += 1,
                Problem::SkippedSnippet { .. } => s.skipped += 1,
                Problem::ExpandFailed { .. } => s.expand_failed += 1,
                Problem::LiveReloadUnavailable { .. } => s.live_reload += 1,
                Problem::ConfigFolder { .. } => s.config_folder += 1,
            }
        }
        s
    }

    /// Tray tooltip: "Meh" or "Meh - N problem(s)".
    pub fn tooltip(&self) -> String {
        match self.len() {
            0 => APP_NAME.to_string(),
            1 => format!("{APP_NAME} - 1 problem"),
            n => format!("{APP_NAME} - {n} problems"),
        }
    }

    /// One problem per line, for the "Show problems..." message box.
    pub fn report(&self) -> String {
        if self.is_empty() {
            return "No problems.".to_string();
        }
        self.iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::{ConfigError, SkipReason, Skipped, Snippet, Var, VarKind};
    use crate::core::expand::{ExpandError, render};
    use crate::core::matcher::Matcher;
    use std::path::PathBuf;

    fn cfg_err(file: &str) -> Problem {
        Problem::from(ConfigError {
            file: PathBuf::from(file),
            line: Some(4),
            msg: "did not find expected '-' indicator".into(),
        })
    }

    fn skipped(trigger: &str) -> Problem {
        Problem::from(Skipped {
            file: PathBuf::from("base.yml"),
            trigger: Some(trigger.into()),
            reason: SkipReason::UnsupportedKey("regex".into()),
        })
    }

    #[test]
    fn problems_replaced_on_reload() {
        let mut p = Problems::default();
        p.replace(vec![cfg_err("a.yml"), skipped(":r")]);
        p.push(Problem::ExpandFailed {
            trigger: ":c".into(),
            reason: ExpandError::ClipboardUnavailable { var: "c".into() },
        });
        assert_eq!(p.len(), 3);
        p.replace(vec![skipped(":s")]);
        assert_eq!(p.iter().cloned().collect::<Vec<_>>(), [skipped(":s")]);
        p.replace(vec![]);
        assert!(p.is_empty());
    }

    #[test]
    fn push_dedupes_and_is_bounded() {
        let mut p = Problems::default();
        let f = |n: usize| Problem::ExpandFailed {
            trigger: format!(":t{n}"),
            reason: ExpandError::BadDateFormat { var: "d".into() },
        };
        p.push(f(0));
        p.push(f(0));
        assert_eq!(p.len(), 1);
        for n in 0..10 * MAX_PROBLEMS {
            p.push(f(n));
        }
        assert_eq!(p.len(), MAX_PROBLEMS);
    }

    #[test]
    fn problem_text_has_no_buffer_content() {
        // API shape: every variant is built from config- or expansion-derived
        // types only. This match has no wildcard, so a new variant fails to
        // compile here until it is reviewed against D-12.
        fn shape(p: &Problem) {
            match p {
                Problem::ConfigError { file, line, msg } => {
                    let _: (&PathBuf, &Option<usize>, &String) = (file, line, msg);
                }
                Problem::SkippedSnippet {
                    file,
                    trigger,
                    reason,
                } => {
                    let _: (&PathBuf, &Option<String>, &SkipReason) = (file, trigger, reason);
                }
                Problem::ExpandFailed { trigger, reason } => {
                    let _: (&String, &ExpandError) = (trigger, reason);
                }
                // An OS error message from the file watcher; never typed text.
                Problem::LiveReloadUnavailable { error } => {
                    let _: &String = error;
                }
                // The config folder path and an OS error message.
                Problem::ConfigFolder { path, msg } => {
                    let _: (&Option<PathBuf>, &String) = (path, msg);
                }
            }
        }

        // Behavior: type secret text, hit a trigger whose expansion fails,
        // and report it. Only the trigger and var name may show up.
        let snippet = Snippet {
            trigger: ":c".into(),
            replace: "{{clip}}".into(),
            word: false,
            vars: vec![Var {
                name: "clip".into(),
                kind: VarKind::Clipboard,
            }],
        };
        let snippets = [snippet];
        let mut m = Matcher::new(&snippets);
        let hit = "my password hunter2 :c"
            .chars()
            .find_map(|c| m.push_char(c))
            .unwrap();
        let s = &snippets[hit.idx];
        let err = render(s, chrono::Local::now().fixed_offset(), &|| None).unwrap_err();
        let problem = Problem::expand_failed(s, err);
        shape(&problem);
        let mut p = Problems::default();
        p.push(problem);
        let text = format!("{}\n{}", p.report(), p.tooltip());
        for secret in ["password", "hunter2", "my "] {
            assert!(!text.contains(secret), "{text}");
        }
        assert!(text.contains(":c") && text.contains("clip"), "{text}");
    }

    #[test]
    fn summary_counts() {
        let mut p = Problems::default();
        assert_eq!(p.summary(), Summary::default());
        assert_eq!(p.tooltip(), "Meh");
        assert_eq!(p.report(), "No problems.");
        p.replace(vec![cfg_err("a.yml"), skipped(":r"), skipped(":s")]);
        p.push(Problem::ExpandFailed {
            trigger: ":c".into(),
            reason: ExpandError::ClipboardUnavailable { var: "c".into() },
        });
        assert_eq!(
            p.summary(),
            Summary {
                config_errors: 1,
                skipped: 2,
                expand_failed: 1,
                live_reload: 0,
                config_folder: 0,
            }
        );
        assert_eq!(p.tooltip(), "Meh - 4 problems");
        p.replace(vec![skipped(":r")]);
        assert_eq!(p.tooltip(), "Meh - 1 problem");
        let report = p.report();
        assert!(report.contains("base.yml"), "{report}");
        assert!(
            report.contains(":r") && report.contains("regex"),
            "{report}"
        );
    }

    #[test]
    fn live_reload_problem_survives_reloads() {
        let mut p = Problems::default();
        p.set_standing(Problem::LiveReloadUnavailable {
            error: "Access is denied. (os error 5)".into(),
        });
        assert_eq!(p.len(), 1);
        assert_eq!(p.tooltip(), "Meh - 1 problem");
        p.replace(vec![skipped(":r")]);
        assert_eq!(p.len(), 2);
        assert_eq!(p.summary().live_reload, 1);
        p.replace(vec![]);
        assert_eq!(p.len(), 1);
        let report = p.report();
        assert!(
            report.starts_with("live reload unavailable: Access is denied."),
            "{report}"
        );
        // Setting it again does not duplicate it.
        p.set_standing(Problem::LiveReloadUnavailable { error: "x".into() });
        assert_eq!(p.len(), 1);
    }

    #[test]
    fn config_folder_problem_names_the_folder_and_survives_reloads() {
        let mut p = Problems::default();
        p.set_standing(Problem::ConfigFolder {
            path: Some(PathBuf::from(r"C:\Users\me\AppData\Roaming\meh\match")),
            msg: "Access is denied. (os error 5)".into(),
        });
        p.set_standing(Problem::LiveReloadUnavailable {
            error: "watch failed".into(),
        });
        p.replace(vec![skipped(":r")]);
        p.replace(vec![]);
        assert_eq!(p.len(), 2);
        assert_eq!(p.summary().config_folder, 1);
        assert_eq!(p.summary().total(), 2);
        assert_eq!(p.tooltip(), "Meh - 2 problems");
        let report = p.report();
        assert!(
            report.starts_with(
                "Could not create the config folder C:\\Users\\me\\AppData\\Roaming\\meh\\match: Access is denied. (os error 5)"
            ),
            "{report}"
        );
        // Setting it again does not duplicate it.
        p.set_standing(Problem::ConfigFolder {
            path: None,
            msg: "x".into(),
        });
        assert_eq!(p.len(), 2);
    }

    #[test]
    fn config_folder_problem_without_a_folder() {
        let p = Problem::config_folder_unknown();
        assert_eq!(
            p.to_string(),
            "Could not find the config folder (%APPDATA% is not set), so no snippets are loaded"
        );
    }

    #[test]
    fn config_error_display_has_file_and_line() {
        let text = cfg_err("C:/x/base.yml").to_string();
        assert!(text.contains("base.yml") && text.contains('4'), "{text}");
    }
}
