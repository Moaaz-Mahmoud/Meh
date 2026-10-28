//! Live-reload state (D-07, D-10).
//!
//! - [`Store`] keeps the last good snippet set: a failed load leaves it as it
//!   was (zero snippets on a first start), and the problems list is replaced
//!   on every reload.
//! - [`Debouncer`] coalesces bursts of file events: it fires once, a fixed
//!   quiet period after the last event.
//! - [`is_relevant_path`] filters watcher events to real match files.
//!
//! Time is passed in, so all of this is testable without sleeping.

use crate::core::APP_NAME;
use crate::core::config::{ConfigError, Loaded, Snippet, is_match_file_name};
use crate::core::problems::{Problem, Problems};
use std::path::Path;
use std::time::{Duration, Instant};

/// Quiet period after the last file event before reloading (D-10).
pub const RELOAD_DEBOUNCE: Duration = Duration::from_millis(250);

/// What a reload did, for the tray to report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The new set is live. `skipped` matches were left out (D-11).
    Applied { snippets: usize, skipped: usize },
    /// The load failed; the previous `kept` snippets stay live.
    KeptLastGood { kept: usize },
}

impl Outcome {
    /// Whether the user should get a notification (D-07, D-12).
    pub fn should_notify(&self) -> bool {
        match self {
            Outcome::Applied { skipped, .. } => *skipped > 0,
            Outcome::KeptLastGood { .. } => true,
        }
    }
}

/// A tray notification (D-07, D-12). Built from problem text, so it never
/// holds typed text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub title: String,
    pub body: String,
    /// An error (the reload failed) rather than a warning (snippets skipped).
    pub is_error: bool,
}

fn plural(n: usize, one: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {one}s")
    }
}

/// The notification for a reload, if it needs one. The body leads with the
/// short part, because the shell truncates long balloon text.
pub fn notice(outcome: &Outcome, problems: &Problems) -> Option<Notice> {
    match *outcome {
        Outcome::KeptLastGood { kept } => {
            let first = problems
                .iter()
                .find(|p| matches!(p, Problem::ConfigError { .. }))
                .map(|p| p.to_string());
            Some(Notice {
                title: format!("{APP_NAME} - config error"),
                body: format!(
                    "Still using the previous {}. {}",
                    plural(kept, "snippet"),
                    first.unwrap_or_default()
                ),
                is_error: true,
            })
        }
        Outcome::Applied { skipped, .. } if skipped > 0 => Some(Notice {
            title: format!("{APP_NAME} - {} skipped", plural(skipped, "snippet")),
            body: "See \"Show problems\u{2026}\" in the tray menu.".to_string(),
            is_error: false,
        }),
        Outcome::Applied { .. } => None,
    }
}

/// The notification for a standing problem ([`Problems::set_standing`]):
/// one that no reload clears, so the user is told once, when it is found.
pub fn standing_notice(problem: &Problem) -> Notice {
    let (title, is_error) = match problem {
        Problem::LiveReloadUnavailable { .. } => ("live reload is off", false),
        Problem::ConfigFolder { .. } => ("no config folder", true),
        _ => ("problem", true),
    };
    Notice {
        title: format!("{APP_NAME} - {title}"),
        body: problem.to_string(),
        is_error,
    }
}

/// Lists a standing problem and returns the notification to show, if any.
/// Only the first standing problem notifies: a later one is usually a
/// consequence (no config folder, so it cannot be watched either), and its
/// balloon would replace the one that names the cause.
pub fn add_standing(problems: &mut Problems, problem: Problem) -> Option<Notice> {
    let first = problems.standing_len() == 0;
    let notice = standing_notice(&problem);
    (problems.set_standing(problem) && first).then_some(notice)
}

/// The live snippet set.
#[derive(Debug, Default)]
pub struct Store {
    snippets: Vec<Snippet>,
}

impl Store {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn snippets(&self) -> &[Snippet] {
        &self.snippets
    }

    /// Applies a load result. On success the snippets are swapped in; on
    /// error the last good set stays. Either way `problems` is replaced with
    /// this load's problems.
    pub fn apply(
        &mut self,
        result: Result<Loaded, ConfigError>,
        problems: &mut Problems,
    ) -> Outcome {
        match result {
            Ok(loaded) => {
                let skipped = loaded.skipped.len();
                problems.replace(loaded.skipped.into_iter().map(Problem::from).collect());
                self.snippets = loaded.snippets;
                Outcome::Applied {
                    snippets: self.snippets.len(),
                    skipped,
                }
            }
            Err(err) => {
                problems.replace(vec![Problem::from(err)]);
                Outcome::KeptLastGood {
                    kept: self.snippets.len(),
                }
            }
        }
    }
}

/// Trailing-edge debouncer: [`poll`](Debouncer::poll) returns true once,
/// `delay` after the most recent [`event`](Debouncer::event).
#[derive(Debug)]
pub struct Debouncer {
    delay: Duration,
    deadline: Option<Instant>,
}

impl Debouncer {
    pub fn new(delay: Duration) -> Self {
        Debouncer {
            delay,
            deadline: None,
        }
    }

    /// Records an event at `now`, pushing the deadline back.
    pub fn event(&mut self, now: Instant) {
        self.deadline = Some(now + self.delay);
    }

    /// When the next fire is due, if armed. Useful as a wait timeout.
    pub fn deadline(&self) -> Option<Instant> {
        self.deadline
    }

    /// True exactly once when the deadline has passed; then disarms.
    pub fn poll(&mut self, now: Instant) -> bool {
        match self.deadline {
            Some(d) if now >= d => {
                self.deadline = None;
                true
            }
            _ => false,
        }
    }
}

/// Whether a watcher event for `path` should trigger a reload (D-10).
pub fn is_relevant_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(is_match_file_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::{ConfigError, Loaded, SkipReason, Skipped, Snippet, load_dir};
    use crate::core::problems::{Problem, Problems};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    fn snip(t: &str) -> Snippet {
        Snippet {
            trigger: t.into(),
            replace: "x".into(),
            word: false,
            vars: vec![],
        }
    }

    fn good(triggers: &[&str]) -> Result<Loaded, ConfigError> {
        Ok(Loaded {
            snippets: triggers.iter().map(|t| snip(t)).collect(),
            skipped: vec![],
        })
    }

    fn bad(file: &str, line: usize) -> Result<Loaded, ConfigError> {
        Err(ConfigError {
            file: PathBuf::from(file),
            line: Some(line),
            msg: "did not find expected key".into(),
        })
    }

    fn triggers(s: &Store) -> Vec<&str> {
        s.snippets().iter().map(|s| s.trigger.as_str()).collect()
    }

    #[test]
    fn first_load_ok_applies() {
        let mut store = Store::new();
        let mut problems = Problems::default();
        let mut loaded = good(&[":a", ":b"]).unwrap();
        loaded.skipped.push(Skipped {
            file: "base.yml".into(),
            trigger: Some(":r".into()),
            reason: SkipReason::UnsupportedKey("regex".into()),
        });
        let out = store.apply(Ok(loaded), &mut problems);
        assert_eq!(
            out,
            Outcome::Applied {
                snippets: 2,
                skipped: 1
            }
        );
        assert!(out.should_notify());
        assert_eq!(triggers(&store), [":a", ":b"]);
        assert_eq!(problems.len(), 1);
        assert!(matches!(
            problems.iter().next(),
            Some(Problem::SkippedSnippet { .. })
        ));
        let clean = store.apply(good(&[":a"]), &mut problems);
        assert!(!clean.should_notify());
        assert!(problems.is_empty());
    }

    #[test]
    fn bad_after_good_keeps_last_good_and_reports_first_error() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        fs::write(
            p.join("a.yml"),
            "matches:\n  - trigger: \":a\"\n    replace: \"x\"\n",
        )
        .unwrap();
        let mut store = Store::new();
        let mut problems = Problems::default();
        store.apply(load_dir(p), &mut problems);
        assert_eq!(triggers(&store), [":a"]);

        fs::write(p.join("b.yml"), "matches: [oops\n").unwrap();
        fs::write(p.join("c.yml"), "matches: {oops\n").unwrap();
        let out = store.apply(load_dir(p), &mut problems);
        assert_eq!(out, Outcome::KeptLastGood { kept: 1 });
        assert!(out.should_notify());
        assert_eq!(triggers(&store), [":a"]);
        let all: Vec<_> = problems.iter().collect();
        assert_eq!(all.len(), 1);
        match all[0] {
            Problem::ConfigError { file, line, .. } => {
                assert_eq!(file, &p.join("b.yml"));
                assert!(line.is_some());
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn bad_on_first_start_yields_zero_snippets() {
        let mut store = Store::new();
        let mut problems = Problems::default();
        let out = store.apply(bad("base.yml", 3), &mut problems);
        assert_eq!(out, Outcome::KeptLastGood { kept: 0 });
        assert!(store.snippets().is_empty());
        assert_eq!(problems.len(), 1);
    }

    #[test]
    fn good_after_bad_recovers() {
        let mut store = Store::new();
        let mut problems = Problems::default();
        store.apply(good(&[":a"]), &mut problems);
        store.apply(bad("base.yml", 2), &mut problems);
        let out = store.apply(good(&[":a", ":c"]), &mut problems);
        assert_eq!(
            out,
            Outcome::Applied {
                snippets: 2,
                skipped: 0
            }
        );
        assert_eq!(triggers(&store), [":a", ":c"]);
        assert!(problems.is_empty());
    }

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn debounce_coalesces_burst() {
        let t0 = Instant::now();
        let mut d = Debouncer::new(RELOAD_DEBOUNCE);
        assert_eq!(RELOAD_DEBOUNCE, ms(250));
        d.event(t0);
        d.event(t0 + ms(50));
        d.event(t0 + ms(100));
        assert_eq!(d.deadline(), Some(t0 + ms(350)));
        assert!(!d.poll(t0 + ms(349)));
        assert!(d.poll(t0 + ms(350)));
        assert!(!d.poll(t0 + ms(351)));
        assert!(!d.poll(t0 + ms(10_000)));
        assert_eq!(d.deadline(), None);
    }

    #[test]
    fn debounce_idle_never_fires() {
        let t0 = Instant::now();
        let mut d = Debouncer::new(ms(250));
        for n in [0, 250, 1_000, 60_000] {
            assert!(!d.poll(t0 + ms(n)));
        }
        assert_eq!(d.deadline(), None);
    }

    #[test]
    fn debounce_rearms_after_fire() {
        let t0 = Instant::now();
        let mut d = Debouncer::new(ms(250));
        d.event(t0);
        assert!(d.poll(t0 + ms(250)));
        d.event(t0 + ms(1_000));
        assert!(!d.poll(t0 + ms(1_249)));
        assert!(d.poll(t0 + ms(1_250)));
        assert!(!d.poll(t0 + ms(1_251)));
    }

    #[test]
    fn notice_for_bad_reload_names_file_line_and_kept_count() {
        let mut store = Store::new();
        let mut problems = Problems::default();
        store.apply(good(&[":a", ":b"]), &mut problems);
        let out = store.apply(bad("C:/m/base.yml", 7), &mut problems);
        let n = notice(&out, &problems).expect("a failed reload notifies");
        assert!(n.is_error);
        assert!(
            n.body.starts_with("Still using the previous 2 snippets."),
            "{}",
            n.body
        );
        assert!(
            n.body.contains("base.yml") && n.body.contains("line 7"),
            "{}",
            n.body
        );
    }

    #[test]
    fn notice_names_the_config_error_not_a_standing_problem() {
        let mut store = Store::new();
        let mut problems = Problems::default();
        problems.set_standing(Problem::LiveReloadUnavailable {
            error: "watch failed".into(),
        });
        let out = store.apply(bad("C:/m/base.yml", 7), &mut problems);
        let n = notice(&out, &problems).unwrap();
        assert!(n.body.contains("line 7"), "{}", n.body);
        assert!(!n.body.contains("watch failed"), "{}", n.body);
        assert_eq!(problems.len(), 2, "standing problem kept");
    }

    #[test]
    fn notice_for_skips_counts_and_clean_reload_is_silent() {
        let mut store = Store::new();
        let mut problems = Problems::default();
        let mut loaded = good(&[":a"]).unwrap();
        loaded.skipped.push(Skipped {
            file: "base.yml".into(),
            trigger: Some(":r".into()),
            reason: SkipReason::UnsupportedKey("regex".into()),
        });
        let out = store.apply(Ok(loaded), &mut problems);
        let n = notice(&out, &problems).expect("skips notify");
        assert!(!n.is_error);
        assert_eq!(n.title, "Meh - 1 snippet skipped");
        let out = store.apply(good(&[":a"]), &mut problems);
        assert_eq!(notice(&out, &problems), None);
        assert_eq!(out.should_notify(), notice(&out, &problems).is_some());
    }

    #[test]
    fn standing_notices_name_the_problem() {
        let live = standing_notice(&Problem::LiveReloadUnavailable {
            error: "Access is denied.".into(),
        });
        assert_eq!(live.title, "Meh - live reload is off");
        assert!(!live.is_error);
        assert!(
            live.body
                .starts_with("live reload unavailable: Access is denied.")
        );

        let folder = standing_notice(&Problem::ConfigFolder {
            path: Some(PathBuf::from("C:/x/meh/match")),
            msg: "Access is denied.".into(),
        });
        assert_eq!(folder.title, "Meh - no config folder");
        assert!(folder.is_error);
        assert!(
            folder.body.contains("C:/x/meh/match") && folder.body.contains("Access is denied."),
            "{}",
            folder.body
        );
        let unknown = standing_notice(&Problem::config_folder_unknown());
        assert!(unknown.body.contains("%APPDATA%"), "{}", unknown.body);
    }

    #[test]
    fn add_standing_notifies_only_for_the_first_new_one() {
        let mut problems = Problems::default();
        let folder = Problem::ConfigFolder {
            path: Some(PathBuf::from("C:/x/meh/match")),
            msg: "Access is denied.".into(),
        };
        let n = add_standing(&mut problems, folder.clone()).expect("first one notifies");
        assert_eq!(n.title, "Meh - no config folder");
        // The watcher then fails on the missing folder: listed, but its
        // balloon would hide the folder one.
        let watch = Problem::LiveReloadUnavailable {
            error: "not found".into(),
        };
        assert_eq!(add_standing(&mut problems, watch), None);
        assert_eq!(add_standing(&mut problems, folder), None, "already listed");
        assert_eq!(problems.len(), 2);
        assert_eq!(problems.summary().live_reload, 1);
    }

    #[test]
    fn relevant_path_accepts_yml_rejects_tmp_swp_tilde() {
        let dir = Path::new("C:/Users/u/AppData/Roaming/meh/match");
        for ok in ["base.yml", "work.YAML", "a.b.yml"] {
            assert!(is_relevant_path(&dir.join(ok)), "{ok}");
        }
        for no in [
            "base.yml~",
            "base.yml.tmp",
            "base.tmp",
            ".base.yml.swp",
            "base.yml.swp",
            ".#base.yml",
            "#base.yml#",
            "~$base.yml",
            "4913",
            ".yml",
            "notes.txt",
        ] {
            assert!(!is_relevant_path(&dir.join(no)), "{no}");
        }
        assert!(!is_relevant_path(Path::new("")));
    }
}
