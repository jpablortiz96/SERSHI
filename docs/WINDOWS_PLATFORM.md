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
| Close Command Center hides (app keeps running)     | `surfaces.rs::on_window_event`               | ⚠️ not exercised                     | —                     | REQUIRES_WINDOWS_VALIDATION      |
| Strict CSP with production build                   | `tauri.conf.json`                            | ✅ release binary ran under Xvfb (WebKitGTK) | —                     | REQUIRES_WINDOWS_VALIDATION      |
| NSIS installer                                     | `bundle.targets`                             | —                                   | —                     | REQUIRES_WINDOWS_VALIDATION      |
| Open/close applications, battery, global shortcut, tray, notifications, credentials, microphone, screen capture, startup | Not implemented | — | — | Planned (see ROADMAP) |

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
git checkout claude/beautiful-goodall-5zbk18   # until merged into main

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
11. The × button hides the window; the companion remains; clicking the companion
    shows it again. "Quit SERSHI" in Settings exits the app.
12. Telemetry shows the correct Windows edition (e.g. "Windows 11 Pro"), CPU model,
    thread count, memory total and a plausible CPU % (compare with Task Manager).
13. Type `How much RAM am I using?` → the core goes violet → blue → mint, a reply
    with `system.get_memory` appears, Activity shows three entries, and after ~2.4 s
    both windows return to Ready. The companion changes colour in sync.
14. Type `Open Spotify` → an amber "not available yet — planned for v0.1" reply.
15. Keyboard: `/` focuses input, `↑` recalls, `Esc` clears, `Ctrl+2` opens Activity.
16. Settings → Developer · State preview: each state previews in both windows;
    "Live" returns to the real state.
17. Windows "Show animations" off (Settings → Accessibility → Visual effects): loops
    stop; states remain distinguishable by colour and label.
18. Narrator reads the state label and SERSHI's replies.

**Production build**

19. `pnpm tauri build --bundles nsis`, install, launch from the Start menu: no
    console window; UI identical to dev (confirms the production CSP loads fonts and
    styles).
20. Uninstall removes the app cleanly.

## Implementation notes for Windows work

- Win32/WinRT calls belong only in `crates/sershi-platform/src/windows/`
  ([ADR 0006](adr/0006-windows-platform-boundary.md)); window behaviour belongs in
  `apps/desktop/src-tauri/src/surfaces.rs`.
- Launching applications must resolve a known executable (App Paths registry,
  Start-menu shortcuts, `shell:AppsFolder`) and use `ShellExecuteW`/`CreateProcessW`
  with explicit arguments — never `cmd /c` or a string from a model.
- Credential storage: Windows Credential Manager (`CredWriteW`/`CredReadW`) behind
  a `CredentialStore` port.
- Planned: tray icon and global shortcut (Tauri `tray-icon` feature and
  `global-shortcut` plugin), click-through companion mode
  (`set_ignore_cursor_events`), start with Windows (opt-in).
