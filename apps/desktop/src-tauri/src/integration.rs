//! Operating-system integration: system tray and global shortcut.
//!
//! REQUIRES_WINDOWS_VALIDATION: tray visibility, menu actions, shortcut
//! registration and conflict behaviour have only been compiled for Windows.

use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use sershi_core::ipc::{FeatureStatus, IntegrationStatus, TrayLabels};
use sershi_core::shortcut::{
    Accelerator, DEFAULT_SHORTCUT, ShortcutBinding, ShortcutChange, ShortcutRegistrar, Unavailable,
};
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::surfaces;

/// If the Command Center never configures a shortcut (e.g. its WebView
/// failed to load), SERSHI registers its default after this delay.
const DEFAULT_SHORTCUT_GRACE: Duration = Duration::from_secs(4);

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
    shortcut: Mutex<ShortcutBinding>,
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
            .map(|b| b.status())
            .unwrap_or_else(|_| ShortcutBinding::default().status());
        IntegrationStatus {
            tray,
            shortcut,
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

/// Sets up the tray. The global shortcut is configured by the Command
/// Center with the user's stored choice (or SERSHI's default) right after it
/// loads; see [`schedule_default_shortcut`]. Failures leave SERSHI running
/// and are reported through [`Integration::status`].
pub fn setup(app: &AppHandle) -> Integration {
    let tray = match create_tray(app) {
        Ok(items) => Some(items),
        Err(error) => {
            eprintln!("SERSHI: system tray unavailable: {error}");
            None
        }
    };
    Integration {
        tray: Mutex::new(tray),
        shortcut: Mutex::new(ShortcutBinding::default()),
    }
}

/// Registers SERSHI's default shortcut if the Command Center has not
/// configured one shortly after start-up. This is the documented default,
/// not a substitute for an unavailable choice: if the user's shortcut was
/// requested and refused, nothing else is registered in its place.
pub fn schedule_default_shortcut(app: &AppHandle) {
    let app = app.clone();
    thread::spawn(move || {
        thread::sleep(DEFAULT_SHORTCUT_GRACE);
        let configured = state(&app)
            .and_then(|i| i.shortcut.lock().ok().map(|b| b.is_configured()))
            .unwrap_or(true);
        if !configured {
            apply_shortcut(&app, DEFAULT_SHORTCUT);
        }
    });
}

/// Tries to make `requested` the summon shortcut. The previous shortcut is
/// released only if the new one registered, so a conflict never leaves
/// SERSHI without its working shortcut. Invocation only: no capability,
/// permission or approval is attached to a shortcut.
pub fn apply_shortcut(app: &AppHandle, requested: &str) -> Option<ShortcutChange> {
    let integration = state(app)?;
    let mut binding = integration.shortcut.lock().ok()?;
    let change = binding.apply(requested, &mut PluginRegistrar { app });
    if change.result == sershi_core::shortcut::ShortcutResult::Unavailable {
        eprintln!(
            "SERSHI: global shortcut {} unavailable (another application owns it)",
            change.requested.as_deref().unwrap_or("?")
        );
    }
    Some(change)
}

/// Registers accelerators with the global-shortcut plugin.
struct PluginRegistrar<'a> {
    app: &'a AppHandle,
}

impl ShortcutRegistrar for PluginRegistrar<'_> {
    fn register(&mut self, accelerator: &Accelerator) -> Result<(), Unavailable> {
        let shortcut: Shortcut = accelerator.canonical().parse().map_err(|_| Unavailable)?;
        self.app
            .global_shortcut()
            .on_shortcut(shortcut, |app, _shortcut, event| {
                if event.state == ShortcutState::Pressed {
                    surfaces::summon(app);
                }
            })
            .map_err(|_| Unavailable)
    }

    fn unregister(&mut self, accelerator: &Accelerator) {
        if let Ok(shortcut) = accelerator.canonical().parse::<Shortcut>() {
            let _ = self.app.global_shortcut().unregister(shortcut);
        }
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
            "quit" => surfaces::quit(app),
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

/// The managed integration state, if set up.
pub fn state(app: &AppHandle) -> Option<tauri::State<'_, Integration>> {
    app.try_state::<Integration>()
}
