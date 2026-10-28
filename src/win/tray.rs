//! The tray icon and its menu (`tray-icon`, Windows only).
//!
//! The `TrayIcon` is `!Send` and owns a hidden window, so it must be created
//! and dropped on the message-loop thread. Dropping it removes the icon.
//!
//! `tray-icon` has no notification API. The icon is registered with a GUID
//! (`with_guid`), and balloons and tooltips are sent with `Shell_NotifyIconW`
//! addressed by that GUID (`NIF_GUID`), see `ui.rs`.

use crate::core::APP_NAME;
use crate::core::ident::tray_guid;
use crate::core::reload::Notice;
use crate::icon;
use crate::win::ui;
use std::error::Error;
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};

/// Used only if the exe path cannot be read.
const FALLBACK_GUID: u128 = 0x4d45_4800_7d1e_4c3a_9b2f_5e6a_0c81_d3f7;

/// What the user picked in the tray menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    SetEnabled(bool),
    Reload,
    OpenFolder,
    ShowProblems,
    Quit,
}

pub struct Tray {
    icon: TrayIcon,
    guid: u128,
    enabled: CheckMenuItem,
    reload: MenuId,
    open: MenuId,
    problems: MenuId,
    quit: MenuId,
    /// Tooltip from the problems list ("Meh" or "Meh - N problems").
    base_tip: String,
}

impl Tray {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let enabled = CheckMenuItem::new("&Enabled", true, true, None);
        let reload = MenuItem::new("&Reload snippets", true, None);
        let open = MenuItem::new("&Open config folder", true, None);
        let problems = MenuItem::new("Show &problems\u{2026}", true, None);
        let quit = MenuItem::new("&Quit", true, None);
        let menu = Menu::new();
        menu.append_items(&[
            &enabled,
            &reload,
            &open,
            &problems,
            &PredefinedMenuItem::separator(),
            &quit,
        ])?;

        let guid = std::env::current_exe()
            .map(|p| tray_guid(&p.to_string_lossy()))
            .unwrap_or(FALLBACK_GUID);
        ui::delete_stale_icon(guid);
        let base_tip = APP_NAME.to_string();
        let icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_icon(Icon::from_rgba(icon::rgba(32), 32, 32)?)
            .with_tooltip(&base_tip)
            .with_guid(guid)
            .build()?;

        Ok(Tray {
            icon,
            guid,
            reload: reload.id().clone(),
            open: open.id().clone(),
            problems: problems.id().clone(),
            quit: quit.id().clone(),
            enabled,
            base_tip,
        })
    }

    /// Drains tray and menu events; returns the menu picks.
    pub fn poll(&self) -> Vec<Action> {
        // Mouse events on the icon are not used, but the channel is
        // unbounded, so drain it.
        while TrayIconEvent::receiver().try_recv().is_ok() {}
        let mut actions = Vec::new();
        while let Ok(ev) = MenuEvent::receiver().try_recv() {
            let id = ev.id;
            if id == *self.enabled.id() {
                // muda already toggled the check mark.
                actions.push(Action::SetEnabled(self.enabled.is_checked()));
            } else if id == self.reload {
                actions.push(Action::Reload);
            } else if id == self.open {
                actions.push(Action::OpenFolder);
            } else if id == self.problems {
                actions.push(Action::ShowProblems);
            } else if id == self.quit {
                actions.push(Action::Quit);
            }
        }
        actions
    }

    /// Sets the problems part of the tooltip.
    pub fn set_problems_tip(&mut self, tip: String) {
        self.base_tip = tip;
        self.refresh_tip();
    }

    /// Updates the tooltip for the current problems and enabled state.
    pub fn refresh_tip(&self) {
        let tip = if self.enabled.is_checked() {
            self.base_tip.clone()
        } else {
            format!("{} (disabled)", self.base_tip)
        };
        // tray-icon's own call addresses the icon by window and uID only
        // (no NIF_GUID), which the shell may refuse; when it succeeds it
        // also keeps the text for re-adding the icon after Explorer
        // restarts. Ours addresses the icon by GUID. Either may fail.
        let _ = self.icon.set_tooltip(Some(&tip));
        ui::set_tip(self.icon.window_handle(), self.guid, &tip);
    }

    /// Shows a balloon notification on the icon.
    pub fn notify(&self, notice: &Notice) {
        ui::show_balloon(self.icon.window_handle(), self.guid, notice);
    }
}
