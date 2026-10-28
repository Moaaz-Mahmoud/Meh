//! The worker thread: owns the matcher, the snippet store and the problems
//! list, and does all `SendInput` calls. The hook only feeds it keys.

use super::{Event, KeyInput, clipboard, hook, inject};
use crate::core::APP_NAME;
use crate::core::config::{ConfigError, Loaded, load_dir};
use crate::core::expand::{plan_injection, render};
use crate::core::matcher::Matcher;
use crate::core::problems::{Problem, Problems};
use crate::core::reload::{Notice, Store, notice};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

/// What the worker tells the UI thread (tray).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiUpdate {
    /// The problems list changed: new tooltip, and maybe a notification.
    Problems {
        tooltip: String,
        notice: Option<Notice>,
    },
}

/// Shared with the tray for the "Show problems…" box.
pub type SharedProblems = Arc<Mutex<Problems>>;

pub struct Worker {
    match_dir: Option<PathBuf>,
    store: Store,
    matcher: Matcher,
    problems: SharedProblems,
    ui: Box<dyn FnMut(UiUpdate) + Send>,
}

impl Worker {
    /// `ui` is called on the worker thread; it should only post a message.
    /// `match_dir` is `None` if `%APPDATA%` could not be found.
    pub fn new(
        match_dir: Option<PathBuf>,
        problems: SharedProblems,
        ui: Box<dyn FnMut(UiUpdate) + Send>,
    ) -> Self {
        Worker {
            match_dir,
            store: Store::new(),
            matcher: Matcher::new(&[]),
            problems,
            ui,
        }
    }

    /// Handles events until `Quit` or until every sender is gone.
    pub fn run(mut self, rx: Receiver<Event>) {
        self.reload();
        while let Ok(ev) = rx.recv() {
            match ev {
                Event::Key(k) => self.key(k),
                Event::Reload => self.reload(),
                // The hook's flag is the one source of truth; keys already
                // queued are dropped in `key`, and the buffer starts fresh.
                Event::EnabledChanged => self.matcher.reset(),
                Event::WatchFailed(error) => self.watch_failed(error),
                Event::Quit => return,
            }
        }
    }

    fn reload(&mut self) {
        let result = match &self.match_dir {
            Some(dir) => load_dir(dir),
            None => Err(ConfigError {
                file: PathBuf::from("%APPDATA%"),
                line: None,
                msg: "could not find the config folder".to_string(),
            }),
        };
        self.apply(result);
    }

    fn apply(&mut self, result: Result<Loaded, ConfigError>) {
        let (tooltip, notice) = {
            let mut problems = lock(&self.problems);
            let outcome = self.store.apply(result, &mut problems);
            (problems.tooltip(), notice(&outcome, &problems))
        };
        self.matcher = Matcher::new(self.store.snippets());
        (self.ui)(UiUpdate::Problems { tooltip, notice });
    }

    /// Lists "live reload unavailable" until meh restarts, and tells the
    /// user once with a notification.
    fn watch_failed(&mut self, error: String) {
        let (tooltip, text) = {
            let mut problems = lock(&self.problems);
            let p = Problem::LiveReloadUnavailable { error };
            let text = p.to_string();
            problems.set_standing(p);
            (problems.tooltip(), text)
        };
        let notice = Notice {
            title: format!("{APP_NAME} - live reload is off"),
            body: text,
            is_error: false,
        };
        (self.ui)(UiUpdate::Problems {
            tooltip,
            notice: Some(notice),
        });
    }

    fn key(&mut self, k: KeyInput) {
        if !hook::is_enabled() {
            return;
        }
        match k {
            KeyInput::Reset => self.matcher.reset(),
            KeyInput::Backspace => self.matcher.backspace(),
            KeyInput::Char(c) => {
                // Enter and Tab come as Reset from the hook; the matcher
                // also treats '\t' / '\n' as reset-only (D-15).
                if let Some(hit) = self.matcher.push_char(c) {
                    self.expand(hit.idx, hit.erase, hit.retype);
                }
            }
        }
    }

    fn expand(&mut self, idx: usize, erase: usize, retype: Option<char>) {
        let Some(snippet) = self.store.snippets().get(idx) else {
            return;
        };
        // Until the keys are sent, the hook turns typed keys into resets.
        let busy = inject::mark_busy();
        let now = chrono::Local::now().fixed_offset();
        match render(snippet, now, &clipboard::read_text) {
            Ok(expansion) => {
                let keys = plan_injection(erase, &expansion, retype);
                inject::wait_for_modifiers_released(inject::MODIFIER_WAIT);
                inject::send(&keys);
            }
            Err(e) => {
                let tooltip = {
                    let mut problems = lock(&self.problems);
                    problems.push(Problem::expand_failed(snippet, e));
                    problems.tooltip()
                };
                (self.ui)(UiUpdate::Problems {
                    tooltip,
                    notice: None,
                });
            }
        }
        drop(busy);
        // Whatever the user types next starts fresh. Keys typed while busy
        // arrive as resets, so they cannot form a stale match.
        self.matcher.reset();
    }
}

/// Locks the problems list, recovering from a poisoned lock (the data is
/// a plain list, always in a valid state).
pub fn lock(p: &SharedProblems) -> std::sync::MutexGuard<'_, Problems> {
    p.lock().unwrap_or_else(|e| e.into_inner())
}

// `app.rs` (Windows only) moves the worker into its own thread; check that
// here, where the Linux type-check can see it.
const _: () = {
    const fn assert_send<T: Send>() {}
    assert_send::<Worker>();
    assert_send::<std::sync::mpsc::Sender<Event>>();
};
