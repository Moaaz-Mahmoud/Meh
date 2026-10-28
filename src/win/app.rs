//! Startup and the main thread (Windows only).
//!
//! The main thread runs the message loop that services the keyboard and
//! mouse hooks and the tray window; a 100 ms timer polls the tray menu and
//! updates from the worker. The worker thread owns matching and injection
//! (see `worker.rs`). meh writes nothing to disk except the first-run
//! sample config (D-12).

use crate::core::problems::Problem;
use crate::core::{APP_NAME, config};
use crate::win::tray::{Action, Tray};
use crate::win::ui::{self, BoxKind};
use crate::win::worker::{self, SharedProblems, UiUpdate, Worker};
use crate::win::{Event, hook, single_instance, watch};
use std::sync::mpsc;
use std::thread;

const TICK_MS: u32 = 100;

pub fn run() {
    // A second launch exits quietly (the first one keeps running).
    let Some(_instance) = single_instance::acquire() else {
        return;
    };

    let match_dir = config::config_dir().map(|d| config::match_dir(&d));
    // Never overwrites (D-05). A failure is listed under "Show problems…"
    // and notified once the tray is up (D-18); it is sent to the worker
    // below, before the watcher can fail on the same missing folder.
    let folder_problem = match &match_dir {
        Some(dir) => config::ensure_sample(dir)
            .err()
            .map(|e| Problem::ConfigFolder {
                path: Some(dir.clone()),
                msg: e.to_string(),
            }),
        None => Some(Problem::config_folder_unknown()),
    };

    let mut tray = match Tray::new() {
        Ok(t) => t,
        Err(e) => {
            ui::message_box(
                APP_NAME,
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
                APP_NAME,
                &format!("meh could not start.\n\n{e}"),
                BoxKind::Error,
            );
            return;
        }
    };

    if let Some(p) = folder_problem {
        let _ = tx.send(Event::Standing(p));
    }

    let hooks = match hook::install(tx.clone()) {
        Ok(h) => h,
        Err(e) => {
            ui::message_box(
                APP_NAME,
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
            let _ = tx.send(Event::Standing(Problem::LiveReloadUnavailable {
                error: e.to_string(),
            }));
            None
        }
        // No config folder: already listed above.
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
                    let _ = tx.send(Event::EnabledChanged);
                    tray.refresh_tip();
                }
                Action::Reload => {
                    let _ = tx.send(Event::Reload);
                }
                // Runs on its own thread: never block the hook thread.
                Action::OpenFolder => ui::open_folder_in_background(
                    match_dir.clone(),
                    APP_NAME,
                    "Could not open the config folder (%APPDATA%\\meh\\match).".to_string(),
                ),
                Action::ShowProblems => {
                    let report = worker::lock(&problems).report();
                    // Modal, on this thread: fine for the hooks, see
                    // `ui::message_box`.
                    let title = format!("{APP_NAME} - problems");
                    ui::message_box(&title, &report, BoxKind::Info);
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
