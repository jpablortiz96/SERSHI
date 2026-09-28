# 0001 — Desktop stack: Tauri 2, React, TypeScript, Vite

**Status:** Accepted · 2026-09

## Context

SERSHI runs all day on Windows as a floating, transparent, always-on-top presence
plus a richer Command Center. It needs native OS access with a strong security
boundary, a small idle footprint, and a front-end capable of premium motion design.

## Decision

- **Shell:** Tauri 2 (Rust core, system WebView — WebView2 on Windows).
- **UI:** React 19 + TypeScript (strict) + Vite, CSS Modules, no UI kit.
- **Tooling:** pnpm workspaces, Vitest, ESLint (type-checked), Prettier.

## Alternatives

| Option       | For                                                           | Against                                                                                          |
| ------------ | ------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| Electron     | Mature, one Chromium everywhere, huge ecosystem               | ~150 MB+ install, ~100 MB+ idle RAM per app, Node in the privileged process makes a weaker boundary for an agent that acts on the OS |
| WinUI 3 / .NET | Truly native Windows look and APIs                          | Windows-only forever, weaker web-grade motion tooling, smaller OSS contributor pool             |
| Flutter      | Good custom rendering                                         | Desktop plugins for OS integration are thinner; no Rust core by default                          |
| Pure Rust UI (egui/iced/Slint) | Tiny, fast                                  | Much harder to reach the visual/motion bar; fewer designers can contribute                       |

## Consequences

- Rust owns every privileged operation; the WebView is an unprivileged renderer.
- Tauri's capability system gives per-window, per-command permissions for free.
- WebView2 is evergreen Chromium on Windows; the Linux development WebView is
  WebKitGTK, so CSS must work on both (no Chromium-only features without fallback).
- Windows-specific window behaviour (transparency, always-on-top, taskbar
  exclusion) depends on WebView2/Tauri and is **REQUIRES_WINDOWS_VALIDATION**.
- TypeScript is pinned to 6.0.x because `typescript-eslint` does not yet support
  the TypeScript 7 native compiler.
