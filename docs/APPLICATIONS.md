# Applications

How SERSHI discovers, opens and closes installed applications on Windows, and
why a request can never become an arbitrary command.

```text
"Abre Spotify"
   │  KeywordIntentResolver (EN/ES/PT imperative verbs)
   ▼
ToolCall system.open_application { "application": "Spotify" }     ← a name, never a path
   │  ToolRegistry → PolicyEngine (system.apps.launch, risk: safe)
   ▼
prepare: ApplicationManager.resolve("Spotify")                   ← cached catalog
   │  Found(ApplicationDescriptor { target: PackagedApp { aumid } })
   ▼
execute: WindowsApplications.launch(target)                      ← IApplicationActivationManager
   ▼
ApplicationResult::Opened { application }  →  "Abrí Spotify."  +  Activity "Se abrió Spotify"
```

## Model

| Type                    | Where                           | Notes                                                                   |
| ----------------------- | ------------------------------- | ----------------------------------------------------------------------- |
| `ApplicationDescriptor` | `sershi-core::apps::model`      | id, display name, aliases, source, launch target, close support         |
| `LaunchTarget`          | core (internal)                 | `Executable` · `PackagedApp { aumid }` · `ShellUri` (fixed) · `Shortcut` |
| `CloseSupport`          | core (internal)                 | `ExecutablePath` · `PackagedApp` · `Any` · `Unsupported`               |
| `ApplicationSummary`    | core → UI                       | id, display name, source — **never paths**                              |
| `ApplicationCatalog`    | `apps::catalog`                 | de-duplicated list + deterministic resolution                           |
| `ApplicationManager`    | `apps::manager`                 | cached catalog, lazy scan, manual refresh, one stale re-scan            |
| `ApplicationPlatform`   | `ports` (trait)                 | `discover`, `launch`, `running_state`, `close`                          |
| `WindowsApplications`   | `sershi-platform/src/windows`   | the only implementation with native code                                |
| `ApplicationResult`     | tool output `data`              | `opened` · `notFound` · `ambiguous` · `launchFailed` · `closeRequested` · `notRunning` · `closeUnsupported` · `catalogUnavailable` |

Launch targets and paths never leave the core: the UI, the activity log and
the contracts only see `ApplicationSummary`.

## Discovery sources (Windows)

| Priority | Source         | How                                                                                                  | Launch strategy                     |
| -------- | -------------- | ---------------------------------------------------------------------------------------------------- | ----------------------------------- |
| 1        | Built-in       | Fixed list: Settings (`ms-settings:`), Notepad, Calculator, File Explorer, with names in EN/ES/PT     | `CreateProcessW` / `ShellExecuteExW` for the fixed URI |
| 2        | Packaged apps  | `shell:AppsFolder` via `IShellItem` enumeration; items whose parsing name is an AUMID (`…!App`)       | `IApplicationActivationManager::ActivateApplication` |
| 3        | Start Menu     | `.lnk` files under `FOLDERID_Programs` and `FOLDERID_CommonPrograms` (depth ≤ 4), read with `IShellLinkW` (no `Resolve`) | `CreateProcessW` on the shortcut's `.exe` target with the shortcut's own arguments; installer-managed ("advertised") shortcuts via `ShellExecuteExW` |
| 4        | App Paths      | `HKCU` + `HKLM` (64- and 32-bit views) `…\CurrentVersion\App Paths\*.exe`                            | `CreateProcessW`                    |

Not used: crawling disks, `PATH`, `where.exe`, PowerShell, `cmd`.

Filtered: shortcuts whose names indicate uninstallers, readmes, help,
documentation, licences or websites; shortcuts to documents or URLs; targets
that don't exist.

**De-duplication.** Entries with the same normalized name or the same launch
target (case-insensitive) keep the higher-priority source; a discovered entry
whose name is an alias of a higher-priority application (e.g. a Start Menu
"Calculadora") is dropped.

## Catalog lifecycle and performance

- **Not scanned at start-up.** A background warm-up scans once, 5 s after
  launch, off the critical path; the first command scans lazily if the
  warm-up hasn't run.
- **No polling.** The catalog is cached in memory. Settings → Windows
  integration → Refresh rescans on demand; a "not found" against a catalog
  older than 60 s rescans once, so newly installed apps are found.
- **Bounded.** Discovery has a 20 s timeout, launch 10 s, close 5 s; hung
  native calls return an error instead of blocking SERSHI. Scan duration is
  shown in Settings for measurement.
- Persistence of the catalog is not needed; SQLite arrives with v0.1 settings.

REQUIRES_WINDOWS_VALIDATION: scan duration on real machines (target: under
1 s for a typical Start Menu).

## Resolution

Queries and names are normalized conservatively (case, common accents,
punctuation, whitespace, a trailing `.exe`). Tiers, first match wins:

1. exact name or id — `spotify`, `windows.calculator`
2. alias — built-in names in every language (`calc`, `calculadora`, `bloc de notas`) and curated aliases (`chrome`, `vscode`, `vs code`, `code`, `edge`, `word`, …)
3. word-boundary prefix — `google` → Google Chrome (query ≥ 3 characters)
4. every query word is a whole word of the name — `studio code` → Visual Studio Code

One match → act. Several → **ambiguous**: nothing runs; the UI lists the
candidates as buttons that re-ask with the exact name. No edit-distance
matching: `spot` does not open Spotify.

## Intent parsing

Only imperative commands act: the verb must be the first word (after an
optional courtesy prefix such as "please", "por favor", "¿puedes…"). Articles
("el", "la", "o", "a", "the") and trailing filler ("app", "por favor") are
dropped.

| Language | Open                                        | Close                                  |
| -------- | ------------------------------------------- | -------------------------------------- |
| English  | open, launch, start, run                    | close, quit, exit                      |
| Spanish  | abre, abrir, abra, inicia, iniciar, ejecuta, ejecutar | cierra, cerrar, cierre       |
| Portuguese | abra, abrir, inicie, iniciar, execute, executar, rode | feche, fechar, encerre, encerrar |

"I like Spotify", "Spotify is open", "Me gusta abrir Spotify" never act
(tested).

## Opening

- `system.open_application` — risk **safe**, permission `system.apps.launch`
  (granted by default), Windows only.
- Input `{ "application": string }`, 1–80 characters, `deny_unknown_fields`.
  Anything containing `\ / : < > | " * ?` or control characters is rejected as
  input rather than "cleaned".
- Executables start with `CreateProcessW` — no shell, no command
  interpreter. Windows answers `ERROR_ELEVATION_REQUIRED` instead of prompting,
  so **SERSHI never triggers UAC** by itself; the user is told the app needs
  administrator rights. For installer-managed shortcuts started through the
  shell, Windows may show its own UAC prompt; SERSHI never bypasses it.
- Arguments exist only when the installer's shortcut defines them. Callers
  cannot supply arguments, URLs or paths (future typed tools would).
- SERSHI does not claim an app "is already open": Windows' own launch
  semantics apply (most apps focus their existing window).

## Closing

- `system.close_application` — risk **sensitive**, permission
  `system.apps.close` (ask by default) ⇒ **always confirmed** today.
- `prepare` resolves the application and checks it is running with a window
  SERSHI can close; otherwise it answers "isn't running" / "can't be closed
  safely yet" **without** asking for confirmation.
- Close = `WM_CLOSE` posted to visible, unowned, titled top-level windows of
  processes whose full image path (or packaged AUMID, via
  `GetApplicationUserModelId`) matches the resolved application. Equivalent to
  clicking ×; the app can ask to save. **Processes are never terminated**
  (`TerminateProcess` / `taskkill /F` are not used).
- Never closable: File Explorer (it hosts the taskbar and desktop), Settings,
  installer-managed shortcuts, SERSHI itself.
- UWP apps hosted by `ApplicationFrameHost` (e.g. Calculator) usually report
  "can't be closed safely yet" — honest rather than guessing a window.

## Security considerations

- The only thing a caller controls is a name, resolved against discovered
  applications. There is no path from text to `CreateProcessW` arguments.
- Launching a shell application the user explicitly asks for by name (e.g.
  "open PowerShell") opens its window like the Start Menu would; SERSHI cannot
  type into it. When model-driven requests arrive, launching shells, credential
  managers or security tools should require confirmation (policy per app
  category, v0.1+). No deny-list is maintained without evidence.
- Confirmations name the **resolved** application (trusted discovery data),
  never the text of the request.
- Activity records the resolved display name and tool id — never the command
  text, paths or command lines.

## Limitations

- REQUIRES_WINDOWS_VALIDATION for everything native: discovery coverage, every
  launch strategy, close matching, timings.
- Applications installed per-user outside the Start Menu and not registered in
  App Paths or as packages are not found.
- No arguments ("open Chrome with…"), no window focusing of an already-running
  instance beyond what Windows does, no "already open" detection.
- Close is best-effort by design; apps minimized to the tray without a window
  report "can't be closed safely yet".
