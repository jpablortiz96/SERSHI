//! Operating-system integration: system tray and global shortcut.
//!
//! REQUIRES_WINDOWS_VALIDATION: tray visibility, menu actions, shortcut
//! registration and conflict behaviour have only been compiled for Windows.

use std::sync::Mutex;

use sershi_core::ipc::{FeatureStatus, IntegrationStatus, ShortcutStatus, TrayLabels};
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::surfaces;

/// The shortcut that summons SERSHI from anywhere.
///
/// Ctrl+Alt+Space rather than the more obvious alternatives:
/// - Alt+Space opens the Windows window menu (and PowerToys Run).
/// - Ctrl+Shift+Space is taken by VS Code (parameter hints) and Excel
///   (select all); a global registration would silently break both.
///
/// Kept as a typed setting so it can become user-configurable with the
/// settings store (v0.1).
pub const DEFAULT_SUMMON_SHORTCUT: &str = "Ctrl+Alt+Space";

const TRAY_ID: &str = "sershi";

/// Tray menu items, kept so their labels can follow the UI language.
struct TrayItems {
    open: MenuItem<Wry>,
    hide: MenuItem<Wry>,
    quit: MenuItem<Wry>,
}

/// What actually got enabled at start-up (managed state).
pub struct Integration {
    tray: Mutex<Option<TrayItems>>,
    shortcut: Mutex<FeatureStatus>,
}

impl Integration {
    pub fn status(&self) -> IntegrationStatus {
        let tray = match self.tray.lock() {
            Ok(guard) if guard.is_some() => FeatureStatus::Active,
            _ => FeatureStatus::Unavailable,
        };
        let shortcut = self
            .shortcut
            .lock()
            .map(|s| *s)
            .unwrap_or(FeatureStatus::Unavailable);
        IntegrationStatus {
            tray,
            shortcut: ShortcutStatus {
                accelerator: DEFAULT_SUMMON_SHORTCUT.to_owned(),
                status: shortcut,
            },
            single_instance: FeatureStatus::Active,
            start_with_windows: FeatureStatus::Planned,
        }
    }

    /// Relabels the tray menu. Labels are validated by the caller.
    pub fn set_tray_labels(&self, labels: &TrayLabels) {
        if let Ok(guard) = self.tray.lock()
            && let Some(items) = guard.as_ref()
        {
            let _ = items.open.set_text(&labels.open);
            let _ = items.hide.set_text(&labels.hide);
            let _ = items.quit.set_text(&labels.quit);
        }
    }
}

/// Sets up the tray and the global shortcut. Failures leave SERSHI running
/// and are reported through [`Integration::status`].
pub fn setup(app: &AppHandle) -> Integration {
    let tray = match create_tray(app) {
        Ok(items) => Some(items),
        Err(error) => {
            eprintln!("SERSHI: system tray unavailable: {error}");
            None
        }
    };
    let shortcut = register_shortcut(app);
    Integration {
        tray: Mutex::new(tray),
        shortcut: Mutex::new(shortcut),
    }
}

fn create_tray(app: &AppHandle) -> tauri::Result<TrayItems> {
    // English until the UI sends labels in the user's language.
    let open = MenuItem::with_id(app, "open", "Open SERSHI", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "hide", "Hide SERSHI", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit SERSHI", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open, &hide, &separator, &quit])?;
    let icon = Image::from_bytes(include_bytes!("../icons/32x32.png"))?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip("SERSHI")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => surfaces::show_all(app),
            "hide" => surfaces::hide_all(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                surfaces::summon(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(TrayItems { open, hide, quit })
}

fn register_shortcut(app: &AppHandle) -> FeatureStatus {
    let Ok(shortcut) = DEFAULT_SUMMON_SHORTCUT.parse::<Shortcut>() else {
        return FeatureStatus::Unavailable;
    };
    let result = app
        .global_shortcut()
        .on_shortcut(shortcut, |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                surfaces::summon(app);
            }
        });
    match result {
        Ok(()) => FeatureStatus::Active,
        Err(error) => {
            // Usually another application owns the combination.
            eprintln!("SERSHI: global shortcut {DEFAULT_SUMMON_SHORTCUT} unavailable: {error}");
            FeatureStatus::Unavailable
        }
    }
}

/// The managed integration state, if set up.
pub fn state(app: &AppHandle) -> Option<tauri::State<'_, Integration>> {
    app.try_state::<Integration>()
}
