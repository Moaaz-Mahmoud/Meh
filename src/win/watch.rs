//! Live reload (D-10): watch the match folder and send [`Event::Reload`]
//! a quiet period after the last relevant change.

use super::Event;
use crate::core::reload::{Debouncer, RELOAD_DEBOUNCE, is_relevant_path};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

/// Keeps the watcher alive; dropping it stops watching.
pub struct Watch {
    _watcher: RecommendedWatcher,
}

/// Watches `dir` (non-recursive) and sends `Event::Reload` to `tx`.
pub fn start(dir: &Path, tx: Sender<Event>) -> notify::Result<Watch> {
    let (raw_tx, raw_rx) = mpsc::channel::<notify::Result<notify::Event>>();
    let mut watcher = notify::recommended_watcher(raw_tx)?;
    watcher.watch(dir, RecursiveMode::NonRecursive)?;

    thread::Builder::new()
        .name("meh-watch".into())
        .spawn(move || {
            let mut debounce = Debouncer::new(RELOAD_DEBOUNCE);
            loop {
                let wait = debounce.deadline().map_or(Duration::from_secs(3600), |d| {
                    d.saturating_duration_since(Instant::now())
                });
                match raw_rx.recv_timeout(wait) {
                    Ok(Ok(ev)) => {
                        if !ev.kind.is_access() && ev.paths.iter().any(|p| is_relevant_path(p)) {
                            debounce.event(Instant::now());
                        }
                    }
                    // A watcher error (e.g. overflow): reload to be safe.
                    Ok(Err(_)) => debounce.event(Instant::now()),
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => return,
                }
                if debounce.poll(Instant::now()) && tx.send(Event::Reload).is_err() {
                    return;
                }
            }
        })
        .map_err(notify::Error::io)?;

    Ok(Watch { _watcher: watcher })
}
