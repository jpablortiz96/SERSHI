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
| Trusted confirmation window (created by Rust, 2-command capability, approve / cancel / × / OS close / expiry / hide / quit) | `src-tauri/src/confirmation.rs`, `surfaces/confirmation/` | ✅ exercised under Xvfb with a temporary, uncommitted fake application adapter: window opened centered and localized, context loaded through its capability, approve executed once and closed it, ×, WM close request, hiding the Command Center, expiry and quit all cancelled with nothing executed | ✅ compiled | REQUIRES_WINDOWS_VALIDATION (focus, always-on-top, Alt+F4, DPI) |
| Visual Experience 2.0: Core renderer 2.0, companion hover / press / drag / presence, window enter & exit transitions, Appearance settings | `apps/desktop/src/visual`, `components/core`, `surfaces/*` | ✅ Chromium preview and WebKitGTK under Xvfb: all 12 states, three languages, 960×640 to 1600×1000, reduced motion; idle CPU unchanged vs Gate 1A (software rendering) | ✅ frontend build + tests | REQUIRES_WINDOWS_VALIDATION (GPU compositing, transparency, DPI, transitions, idle CPU) |
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
git checkout claude/prompt-2-visual-experience  # until merged into main

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

A12. With Spotify open: `Close Spotify` → the separate confirmation window
     "Close Spotify?" appears with a countdown; Cancel has focus; the core is
     amber ("Waiting for you"). Nothing closes while it is shown. (Full
     confirmation checklist: C1–C15 below.)
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
A19. Ask to close, wait 60 s without answering → the confirmation window
     closes, reply "This request expired…", Activity "Approval expired".

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

**Trusted confirmation window (Gate 1A)**

C1. `Close Spotify` (Spotify running).
C2. A **separate** SERSHI window appears, centered and above other windows,
    showing "Confirmation required · Close Spotify?". It has keyboard focus
    (or, if Windows refuses focus, its taskbar button flashes). Record which.
C3. The Command Center shows only "…is waiting for your approval in the
    confirmation window" — **no approve button** anywhere in it.
C4. Press **Cancel** → the window disappears; Spotify stays open; the Command
    Center shows "Cancelled. Nothing was changed.".
C5. Repeat `Close Spotify`.
C6. Press **Close Spotify** → Spotify closes gracefully (may ask to save) **or**
    SERSHI answers "can't close Spotify safely yet". Record which.
C7. The confirmation window disappears after the decision.
C8. Replay: there is no way to approve again — the window is gone; asking
    again creates a new confirmation. (Automated: `approved_close_runs_once_and_cannot_be_replayed`.)
C9. `Close Spotify`, wait more than 60 s → the window closes by itself, the
    reply says the request expired, Spotify stays open.
C10. `Close Spotify`, close the confirmation window with its × **and** (a
     second time) with **Alt+F4** → both cancel; Spotify stays open.
C11. Settings → Language → **Español**, `Cierra Spotify` → the window reads
     "Se requiere confirmación · ¿Cerrar Spotify?", buttons "Cancelar" /
     "Cerrar Spotify".
C12. **Português**, `Feche o Spotify` → "Confirmação necessária · Fechar
     Spotify?", "Cancelar" / "Fechar Spotify"; no clipped text.
C13. Click the companion while the confirmation window is open → the Command
     Center comes forward; nothing is approved; the confirmation window stays
     (or comes back to front).
C14. Press **Ctrl+Alt+Space** while the confirmation window is open → SERSHI
     comes forward with the confirmation window focused; nothing is approved.
C15. With the confirmation window open, quit SERSHI from the tray → SERSHI
     exits; Spotify stays open; relaunch SERSHI → no confirmation reappears.
C16. Accidental input: type `Close Spotify` and keep **Enter** held down as the
     window appears → the request is cancelled or nothing happens; it is
     never approved. Double-click quickly where the Approve button will appear
     → nothing is approved.
C17. At 150% display scaling the window is fully visible and nothing clips.

**Production build**

19. `pnpm tauri build --bundles nsis`, install, launch from the Start menu: no
    console window; UI identical to dev (confirms the production CSP loads fonts and
    styles).
20. Uninstall removes the app cleanly.

## Gate 2A — Windows Integrated Validation

One combined physical pass covering **Prompt 1** (application operation),
**Gate 1A** (trusted confirmations) and **Prompt 2** (visual experience).
Nothing here has been run on Windows yet. Record results in the table at the
end (PASS / FAIL / NOT_TESTED per item, with notes).

**Setup.** Build from `claude/prompt-2-visual-experience` (see "Clone, check
and run"), install or run `pnpm dev`, and have Spotify (or another
third-party app — note which), Notepad and Calculator available.

### G1 · Functional (real launches)

1. `Open Spotify` → Spotify opens; core Thinking → Planning → Working → Done;
   reply "Opened Spotify."; Activity row with ✓.
2. Español: `Abre Spotify` → "Abrí Spotify.".
3. Português: `Abra o Spotify` → "Abri Spotify.".
4. `Open Calculator` → Calculator opens.
5. `Open Notepad` → Notepad opens.
6. Unknown app (`Open Zzyzx`) → honest "couldn't find" reply; nothing launches.
7. Ambiguous (`Open Visual Studio` with Code and VS installed, or `Open
   Google`) → candidate buttons; nothing launches until one is chosen.

### G2 · Trusted confirmation (nothing may bypass explicit approval)

1. `Close Spotify` → a **separate** confirmation window appears (approval
   mark, amber frame); the Command Center shows only "waiting…"; no approve
   button in it; the companion turns amber (Waiting for you).
2. **Cancel** → Spotify stays open; window disappears.
3. Repeat, **Close Spotify** (approve) → graceful close or honest
   "can't close safely yet".
4. Repeat, **Escape** → cancelled.
5. Repeat, the window's **×** → cancelled.
6. Repeat, **Alt+F4** on the confirmation window → cancelled.
7. Repeat, wait **60 s** → window closes by itself; "request expired".
8. Hold **Enter** while submitting `Close Spotify` so it is still held when
   the window appears → never approved.
9. **Double-click** where the approve button will appear → never approved.
10. With the window open, press **Ctrl+Alt+Space** → SERSHI comes forward,
    the confirmation stays pending and focused; nothing approved.
11. With the window open, **click the companion** → Command Center comes
    forward; nothing approved.
12. With the window open, **quit from the tray** → nothing executes; Spotify
    stays open.
13. **Restart** SERSHI → no confirmation reappears.

### G3 · Windows integration

1. Tray: Open / Hide / Quit work; labels follow the language.
2. **Ctrl+Alt+Space** from another app → SERSHI appears, command bar focused
   (or taskbar flash if Windows refuses focus — record which), core Attending.
3. Command Center **×** → short retreat, window hides, SERSHI stays in tray
   and companion.
4. Second launch → no second tray icon; existing windows come forward.
5. Companion **click** → Command Center restores and the command bar has focus.
6. Companion **drag** → moves smoothly, shrinks slightly while dragging,
   settles on release; never opens the Command Center.
7. Minimized Command Center restores on summon.

### G4 · Visual states (Settings → Developer · State preview)

Preview each: Ready, Attending, Thinking, Planning, Working, Done, Couldn't
complete, Waiting for you (plus Listening, Speaking, Needs attention,
Sleeping). For each, in the Command Center **and** the companion:

1. States are clearly distinct (motion, shape, colour, glyph + label).
2. Motion is smooth, calm, no stutter; Ready is almost still.
3. Text stays readable; hierarchy clear.
4. Settings → Appearance → Motion → **Reduced**: loops stop; every state is
   still recognisable by shape, colour and label. Back to **System**.
5. Companion size Small / Medium / Large → glow never clips into a square.

### G5 · Languages

English, Español, Português — check the Command Center, companion
screen-reader label, confirmation window, Settings (incl. Appearance),
Activity (status words), tray menu and application replies. No clipped or
overflowing text (navigation, confirmation, Appearance choices).

### G6 · Display

1. Record: Windows version, GPU, resolution, display scale, monitor count.
2. At 100 %, 125 % and 150 % (where practical): companion fully visible above
   the taskbar, confirmation window fully visible, Command Center at its
   minimum size (960 × 640) and maximized.
3. Only one monitor available → record `MULTI_MONITOR_NOT_TESTED`.

### G7 · Performance (obvious regressions only)

Task Manager → Details (`sershi-desktop.exe` + `msedgewebview2.exe`):

1. Idle, Command Center hidden, 30 s: companion CPU (target < 1 %).
2. Idle, Command Center visible: CPU.
3. Thinking preview running: CPU.
4. Memory (working set) after 5 minutes.
5. Startup: time from launch to an interactive Command Center (rough).

### G8 · Visual defects

Look specifically for: white flashes (startup, Command Center open,
confirmation open), black rectangles behind the transparent companion,
clipped glow, jagged orbit rendering, text overflow, blurry transforms during
window transitions, scrollbar flashes, focus-ring glitches, window flicker.

## Windows validation record

Copy this block for each validation session. Do not mark an item PASS without
running it on Windows hardware.

```text
Windows version:      (e.g. Windows 11 Pro 24H2, build 26100.xxxx)
GPU:                  (e.g. Intel Iris Xe)
Resolution:           (e.g. 2560×1440)
Display scale:        (e.g. 150%)
Monitor setup:        (count — if single: MULTI_MONITOR_NOT_TESTED)
Validation date:      (YYYY-MM-DD)
Commit:               (git rev-parse HEAD)
Catalog scan time:    (ms, first scan / refresh)
Results:              Gate 2A G1–G8 (supersedes 1–20, A1–A29, C1–C17): PASS / FAIL / NOT_TESTED each
Idle CPU:             (companion only / Command Center visible / Thinking)
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
