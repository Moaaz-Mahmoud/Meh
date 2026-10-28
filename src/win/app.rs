//! Startup and the main thread (Windows only).
//!
//! The main thread runs the message loop that services the keyboard and
//! mouse hooks and the tray window; a 100 ms timer polls the tray menu and
//! updates from the worker. The worker thread owns matching and injection
//! (see `worker.rs`). meh writes nothing to disk except the first-run
//! sample config (D-12).

use crate::core::config;
use crate::win::tray::{Action, Tray};
use crate::win::ui::{self, BoxKind};
use crate::win::worker::{self, SharedProblems, UiUpdate, Worker};
use crate::win::{Event, hook, single_instance, watch};
use std::sync::mpsc;
use std::thread;

const TITLE: &str = "Meh";
const TICK_MS: u32 = 100;

pub fn run() {
    // A second launch exits quietly (the first one keeps running).
    let Some(_instance) = single_instance::acquire() else {
        return;
    };

    let match_dir = config::config_dir().map(|d| config::match_dir(&d));
    if let Some(dir) = &match_dir {
        // Never overwrites (D-05). On failure the load below reports it.
        let _ = config::ensure_sample(dir);
    }

    let mut tray = match Tray::new() {
        Ok(t) => t,
        Err(e) => {
            ui::message_box(
                TITLE,
                &format!("meh could not create its tray icon and will exit.\n\n{e}"),
                BoxKind::Error,
            );
            return;
        }
    };

    let (tx, rx) = mpsc::channel::<Event>();
    let (ui_tx, ui_rx) = mpsc::channel::<UiUpdate>();
    let problems = SharedProblems::default();
    let worker = Worker::new(
        match_dir.clone(),
        problems.clone(),
        Box::new(move |u| {
            let _ = ui_tx.send(u);
        }),
    );
    let worker_thread = match thread::Builder::new()
        .name("meh-worker".into())
        .spawn(move || worker.run(rx))
    {
        Ok(h) => h,
        Err(e) => {
            ui::message_box(
                TITLE,
                &format!("meh could not start.\n\n{e}"),
                BoxKind::Error,
            );
            return;
        }
    };

    let hooks = match hook::install(tx.clone()) {
        Ok(h) => h,
        Err(e) => {
            ui::message_box(
                TITLE,
                &format!("meh could not install its keyboard hook and will exit.\n\n{e}"),
                BoxKind::Error,
            );
            let _ = tx.send(Event::Quit);
            let _ = worker_thread.join();
            return;
        }
    };

    // Live reload (D-10). If watching fails, the tray "Reload" still works,
    // and the failure is listed under "Show problems…".
    let _watch = match match_dir
        .as_deref()
        .map(|dir| watch::start(dir, tx.clone()))
    {
        Some(Ok(w)) => Some(w),
        Some(Err(e)) => {
            let _ = tx.send(Event::WatchFailed(e.to_string()));
            None
        }
        // No config folder: the load already reports that.
        None => None,
    };

    ui::message_loop(TICK_MS, || {
        while let Ok(update) = ui_rx.try_recv() {
            match update {
                UiUpdate::Problems { tooltip, notice } => {
                    tray.set_problems_tip(tooltip);
                    if let Some(n) = notice {
                        tray.notify(&n);
                    }
                }
            }
        }
        for action in tray.poll() {
            match action {
                Action::SetEnabled(on) => {
                    hook::set_enabled(on);
                    let _ = tx.send(Event::SetEnabled(on));
                    tray.refresh_tip();
                }
                Action::Reload => {
                    let _ = tx.send(Event::Reload);
                }
                // Runs on its own thread: never block the hook thread.
                Action::OpenFolder => ui::open_folder_in_background(
                    match_dir.clone(),
                    TITLE,
                    "Could not open the config folder (%APPDATA%\\meh\\match).".to_string(),
                ),
                Action::ShowProblems => {
                    let report = worker::lock(&problems).report();
                    // Modal, on this thread: fine for the hooks, see
                    // `ui::message_box`.
                    ui::message_box("Meh - problems", &report, BoxKind::Info);
                }
                Action::Quit => ui::quit(),
            }
        }
    });

    // Remove the hooks and the icon before the worker goes away.
    drop(hooks);
    drop(tray);
    let _ = tx.send(Event::Quit);
    let _ = worker_thread.join();
}
