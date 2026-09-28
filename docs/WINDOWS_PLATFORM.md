# Windows platform

SERSHI is **Windows-first**. Most development happens in Linux cloud environments,
so this document separates what has been validated where, and gives the exact
steps to validate on a physical Windows machine.

> A green Linux test run proves portable logic only. Anything listed below as
> **REQUIRES_WINDOWS_VALIDATION** stays that way until someone completes the manual
> checklist on Windows hardware and records the result here.

## Validation status

| Capability                                         | Implemented in                               | Linux (cloud)                       | Windows CI            | Windows hardware                 |
| -------------------------------------------------- | -------------------------------------------- | ----------------------------------- | --------------------- | -------------------------------- |
| Domain logic (state, policy, tools, activity)      | `sershi-core`                                | ✅ tested                            | ✅ tests run in CI     | n/a (portable)                   |
| System telemetry (OS, CPU, RAM, uptime)            | `sershi-platform::system_info` (`sysinfo`)   | ✅ integration test + live app       | ✅ integration test    | REQUIRES_WINDOWS_VALIDATION      |
| Tauri shell compiles                               | `apps/desktop/src-tauri`                     | ✅                                   | ✅ clippy build        | —                                |
| Command Center window (frameless, custom chrome, drag, minimize/maximize/hide) | `tauri.conf.json`, `TitleBar.tsx` | ✅ ran under Xvfb (no window manager) | —                 | REQUIRES_WINDOWS_VALIDATION      |
| Companion: transparent, borderless window          | `tauri.conf.json`                            | ⚠️ renders on a 32-bit visual; true transparency needs a compositor, not present under Xvfb | — | REQUIRES_WINDOWS_VALIDATION |
| Companion: always-on-top, hidden from taskbar      | `tauri.conf.json`                            | ⚠️ not observable without a window manager | —              | REQUIRES_WINDOWS_VALIDATION      |
| Companion placement above the taskbar (work area, DPI scaling, multi-monitor) | `surfaces.rs::place_companion` | ✅ single 1600×1000 screen at scale 1 | —         | REQUIRES_WINDOWS_VALIDATION      |
| Companion drag vs click                            | `Companion.tsx`                              | ⚠️ click verified; drag needs a window manager | —         | REQUIRES_WINDOWS_VALIDATION      |
| Close Command Center hides (× and OS close; app keeps running) | `surfaces.rs`, `TitleBar.tsx`     | ⚠️ × wired to `hide_command_center`; not exercised without a window manager | — | REQUIRES_WINDOWS_VALIDATION |
| Application discovery (built-ins, `shell:AppsFolder`, Start Menu `.lnk`, App Paths) | `sershi-platform/src/windows/` | ✅ catalog, de-duplication and resolution tested with fakes; native code not runnable | ✅ compiled + clippy (also cross-checked from Linux with `--target x86_64-pc-windows-msvc`) | REQUIRES_WINDOWS_VALIDATION |
| Open application (`CreateProcessW`, `IApplicationActivationManager`, `ShellExecuteExW` for `ms-settings:`/installer shortcuts) | `windows/launch.rs`, `windows/packaged.rs` | ✅ pipeline tested with fakes; off Windows the tool is denied as unsupported (verified in the running app) | ✅ compiled | REQUIRES_WINDOWS_VALIDATION |
| Close application (`WM_CLOSE` to matching top-level windows) | `windows/close.rs`                   | ✅ confirmation flow tested with fakes; dialog rendered under Xvfb | ✅ compiled | REQUIRES_WINDOWS_VALIDATION |
| System tray (Open / Hide / Quit, localized labels, left-click summons) | `src-tauri/src/integration.rs` | ⚠️ created under Xvfb (reported Active); menu not exercisable without a panel | ✅ compiled | REQUIRES_WINDOWS_VALIDATION |
| Global shortcut `Ctrl+Alt+Space` (summon + focus input) | `integration.rs`, `surfaces.rs::summon` | ⚠️ registered under X11 (reported Active); key press not exercised | ✅ compiled | REQUIRES_WINDOWS_VALIDATION |
| Focus when summoned (foreground-lock fallback: taskbar flash) | `surfaces.rs::show_command_center` | ⚠️ not observable without a window manager | — | REQUIRES_WINDOWS_VALIDATION |
| Single instance (second launch shows the existing windows) | `tauri-plugin-single-instance`   | ⚠️ not exercised                     | ✅ compiled            | REQUIRES_WINDOWS_VALIDATION      |
| Strict CSP with production build                   | `tauri.conf.json`                            | ✅ release binary ran under Xvfb (WebKitGTK) | —                     | REQUIRES_WINDOWS_VALIDATION      |
| Interface language: automatic detection from the Windows display language | `i18n/detect.ts` (`navigator.languages`) | ⚠️ container has no pt_BR/es locales; validated in Chromium with a pt-BR locale | — | REQUIRES_WINDOWS_VALIDATION |
| Interface language: persistence across restarts and sync between windows | `i18n/preferences.ts`, `i18n/store.ts` (localStorage) | ✅ Tauri app restarted under Xvfb kept Español | — | REQUIRES_WINDOWS_VALIDATION |
| NSIS installer                                     | `bundle.targets`                             | —                                   | —                     | REQUIRES_WINDOWS_VALIDATION      |
| Battery, notifications, credentials, microphone, screen capture, start with Windows | Not implemented | — | — | Planned (see ROADMAP) |

## Prerequisites (development)

1. **Windows 10 (1809+) or Windows 11**, x64.
2. **Microsoft C++ Build Tools** — Visual Studio 2022 Build Tools with the
   "Desktop development with C++" workload (MSVC + Windows SDK).
3. **WebView2 Runtime** — preinstalled on Windows 11 and current Windows 10.
4. **Rust** (stable, MSVC toolchain): install from <https://rustup.rs>, then
   `rustup default stable-msvc`.
5. **Node.js 22 LTS** (≥ 22.12): <https://nodejs.org>.
6. **pnpm** via Corepack: `corepack enable` (the repo pins `pnpm@10.33.0`).
7. **Git** for Windows.

End users will need none of this — the target is a single `SERSHI-Setup.exe`
(v0.9).

## Clone, check and run (PowerShell)

```powershell
git clone https://github.com/jpablortiz96/SERSHI.git
cd SERSHI
git checkout claude/prompt-1-windows-operator  # until merged into main

corepack enable
pnpm install

# Automated checks
pnpm typecheck
pnpm lint
pnpm test
pnpm build
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# Launch SERSHI (first build takes a few minutes)
pnpm dev
```

Optional installer build (unsigned, local use only):

```powershell
pnpm tauri build --bundles nsis
# Output: target\release\bundle\nsis\SERSHI_0.0.1_x64-setup.exe
```

## Manual validation checklist

Record the Windows version, display scaling and monitor setup. For each item note
Pass / Fail / Notes, then update the status table above.

**Launch**

1. `pnpm dev` opens two windows: the Command Center (centred, frameless, dark, no
   white flash) and the companion (bottom-right, just above the taskbar).
2. Task Manager shows one `sershi-desktop.exe` process (plus WebView2 processes).

**Companion**

3. The companion background is fully transparent — only the glowing core is
   visible; no black or white square.
4. It stays above other windows (open a maximised browser over it).
5. It does not appear in the taskbar or Alt+Tab.
6. Clicking it brings the Command Center to the front and the state label shows
   "Attending"; both windows turn pale ice.
7. Press-and-drag moves the companion smoothly; releasing after a drag does **not**
   open the Command Center.
8. At 125% / 150% display scaling, the companion is still fully visible above the
   taskbar. With the taskbar on the left/top, it still sits inside the work area.
9. Idle CPU for the companion (Task Manager, Command Center hidden) stays low
   (target < 1%); note the observed value.

**Command Center**

10. Drag the window by the title bar; minimise and maximise buttons work.
11. The × button hides the window; the companion and tray icon remain; clicking
    the companion shows it again. "Quit SERSHI" in Settings exits the app.
12. Telemetry shows the correct Windows edition (e.g. "Windows 11 Pro"), CPU model,
    thread count, memory total and a plausible CPU % (compare with Task Manager).
13. Type `How much RAM am I using?` → the core goes violet → blue → mint, a reply
    with `system.get_memory` appears, Activity shows three entries, and after ~2.4 s
    both windows return to Ready. The companion changes colour in sync.
14. Type `Open Notepad` → Notepad opens (full application checklist below).
15. Keyboard: `/` focuses input, `↑` recalls, `Esc` clears, `Ctrl+2` opens Activity.
16. Settings → Developer · State preview: each state previews in both windows;
    "Live" returns to the real state.
17. Windows "Show animations" off (Settings → Accessibility → Visual effects): loops
    stop; states remain distinguishable by colour and label.
18. Narrator reads the state label and SERSHI's replies.

**Language** (see [LOCALIZATION.md](LOCALIZATION.md))

19a. With Windows in English and no saved preference, SERSHI starts in English.
19b. Settings → General → Language → **Español**: the Command Center updates
     immediately (navigation, Home, rails, command bar hints, earlier replies).
19c. The companion follows: Narrator reads it as "Abrir el Centro de control de
     SERSHI. Estado: Listo." and the status follows the state. (REQUIRES_WINDOWS_VALIDATION: cross-window sync via
     the `storage` event.)
19d. Switch to **Português**: no overflow or clipped text in navigation, Settings
     rows, the language picker, the state preview grid, telemetry and activity.
19e. Quit SERSHI (Settings → About → Quit) and relaunch: Português is still
     selected.
19f. Choose **Automatic**: the interface follows the Windows display language.
     Change Windows' display language to Español (or Português), sign out and in,
     relaunch SERSHI: it starts in that language. Any other language → English.
     (REQUIRES_WINDOWS_VALIDATION: WebView2 language reporting.)

**Applications, tray and shortcut (Prompt 1)**

Use an app you have installed in place of Spotify (e.g. Chrome or VS Code) if
needed; note which.

_Catalog_

A1. Launch SERSHI. Settings → Windows integration → Installed applications shows
    a count and "Scanned at … in … ms" within a few seconds (warm-up runs 5 s
    after launch). **Record the scan duration.**
A2. Press Refresh: the count updates; note the duration again.
A3. Developer build: Settings → Developer → Discovered applications lists at
    least Calculator, Notepad, File Explorer and Settings, with no file paths
    shown anywhere.
A4. Installed third-party apps (Spotify, Chrome, VS Code, …) are listed once
    each (no duplicates from Start Menu + App Paths).

_Open_

A5. `Open Spotify` → the core goes Thinking → Planning → Executing → Success;
    Spotify opens; reply "Opened Spotify."; Activity shows "Opened Spotify".
A6. Switch to Español: `Abre Spotify` → reply "Abrí Spotify.".
A7. Switch to Português: `Abra o Spotify` → reply "Abri Spotify.".
A8. `Open Notepad`, `Abre la calculadora`, `Abrir configuración`,
    `Open File Explorer` each open the right built-in app.
A9. `Open Google` (or another prefix shared by several apps) → an
    "which one?" reply with candidate buttons; nothing launches until one is
    clicked.
A10. `Open Microsoft Paint` on a machine without it, or `Open Zzyzx` → honest
     "couldn't find" reply; nothing launches; Task Manager shows no new process.
A11. An app that requires administrator rights (if available) → SERSHI says it
     needs administrator rights; **no UAC prompt appears** from SERSHI itself.

_Close_

A12. With Spotify open: `Close Spotify` → a confirmation dialog "Close Spotify?"
     with a countdown; Cancel has focus; the core is amber ("Waiting for you").
     Nothing closes while the dialog is shown.
A13. Press **Cancel** (or Esc) → Spotify stays open; reply "Cancelled. Nothing
     was changed."; Activity "Cancelled: Close application · Spotify".
A14. Repeat and press **Close Spotify** → Spotify closes gracefully (it may ask
     to save) **or** SERSHI answers "can't close Spotify safely yet". Record
     which.
A15. Open Notepad, type text, `Close Notepad`, approve → Notepad asks to save
     (proves `WM_CLOSE`, not a forced kill).
A16. `Close Calculator` (packaged app hosted by ApplicationFrameHost) → record
     whether it closes or reports "can't close safely yet".
A17. `Close File Explorer` → "can't close File Explorer safely yet" without a
     confirmation; the taskbar is unaffected.
A18. `Close Spotify` while Spotify is not running → "Spotify isn't running."
     without a confirmation.
A19. Ask to close, wait 90 s without answering → the dialog closes, reply "This
     request expired…", Activity "Approval expired".

_Tray and window lifecycle_

A20. Close the Command Center with × → the window hides; the SERSHI icon remains
     in the notification area; the companion remains.
A21. Tray right-click → **Open SERSHI** shows the Command Center; **Hide SERSHI**
     hides both windows; left-click on the icon summons the Command Center.
A22. Launch SERSHI a second time (Start menu or `pnpm tauri dev` binary) → no
     second tray icon; the existing windows come forward.
A23. Tray → **Quit SERSHI** exits; Task Manager shows no `sershi-desktop.exe`.

_Shortcut_

A24. Open Chrome (or any app) in the foreground and press **Ctrl+Alt+Space** →
     SERSHI comes forward and the command input has keyboard focus (type
     immediately). If Windows refuses focus, the taskbar button flashes instead
     — record which.
A25. With the shortcut taken by another app, start SERSHI → Settings → Windows
     integration shows the shortcut as unavailable; SERSHI still runs.

_Companion_

A26. The companion stays above other windows, a click opens the Command Center,
     and a drag does not.

_DPI_

A27. Record the current display scale; repeat A5, A12 and A26 at 125% and 150%
     if practical (dialog and companion fully visible, no clipped text).

_Localization_

A28. Español: tray menu (Abrir / Ocultar SERSHI, Salir de SERSHI), confirmation dialog,
     Settings → Integración con Windows are translated.
A29. Português: same surfaces translated; no overflow or clipped text in the
     dialog, Settings rows and candidate buttons.

**Production build**

19. `pnpm tauri build --bundles nsis`, install, launch from the Start menu: no
    console window; UI identical to dev (confirms the production CSP loads fonts and
    styles).
20. Uninstall removes the app cleanly.

## Windows validation record

Copy this block for each validation session. Do not mark an item PASS without
running it on Windows hardware.

```text
Windows version:      (e.g. Windows 11 Pro 24H2, build 26100.xxxx)
Display scale:        (e.g. 150%)
Monitor setup:        (single / multiple — if single: NOT_TESTED_MULTI_MONITOR)
Validation date:      (YYYY-MM-DD)
Commit:               (git rev-parse HEAD)
Catalog scan time:    (ms, first scan / refresh)
Results:              1–20, 19a–19f, A1–A29: PASS / FAIL / NOT_TESTED each
Notes:
```

| Date | Windows | Scale | Monitors | Commit | Result | Notes |
| ---- | ------- | ----- | -------- | ------ | ------ | ----- |
| _none yet_ | — | — | — | — | NOT_TESTED | No physical Windows validation has been performed. |

## Implementation notes for Windows work

- Win32/WinRT calls belong only in `crates/sershi-platform/src/windows/`
  ([ADR 0006](adr/0006-windows-platform-boundary.md)); window behaviour belongs in
  `apps/desktop/src-tauri/src/surfaces.rs`.
- Win32 FFI is the only place `unsafe` is allowed (`#![allow(unsafe_code)]` in
  `windows/mod.rs`; the workspace denies it elsewhere). COM is initialized per
  call on a worker thread (`ComApartment`), and every native operation runs
  under a timeout (`util::with_timeout`).
- Applications are launched as described in [APPLICATIONS.md](APPLICATIONS.md):
  `CreateProcessW` with the discovered executable, `IApplicationActivationManager`
  for packaged apps, `ShellExecuteExW` only for fixed `ms-settings:` and
  installer-managed shortcuts — never `cmd /c`, PowerShell or a string from a
  model.
- Cloud cross-check of Windows code (no Windows needed):
  `rustup target add x86_64-pc-windows-msvc` then
  `cargo clippy -p sershi-platform --target x86_64-pc-windows-msvc -- -D warnings`
  (type-checks the `cfg(windows)` modules; linking needs Windows).
- Credential storage: Windows Credential Manager (`CredWriteW`/`CredReadW`) behind
  a `CredentialStore` port.
- Global shortcut: `Ctrl+Alt+Space`. Not `Alt+Space` (Windows window menu,
  PowerToys Run) and not `Ctrl+Shift+Space` (VS Code parameter hints, Excel
  select-all; a global registration would silently break them). Becomes
  configurable with the v0.1 settings store.
- Planned: click-through companion mode (`set_ignore_cursor_events`), start
  with Windows (opt-in).
