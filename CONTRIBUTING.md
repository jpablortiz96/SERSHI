# Contributing to SERSHI

Thanks for your interest. SERSHI is pre-alpha; the architecture is stabilising, so
please open an issue to discuss anything larger than a bug fix before starting.

## Setup

See the README for requirements. Then:

```bash
pnpm install
pnpm dev
```

Windows contributors: [docs/WINDOWS_PLATFORM.md](docs/WINDOWS_PLATFORM.md).
Building the voice adapters on Windows also needs CMake, Ninja, LLVM
(libclang) and the Vulkan SDK; see [docs/VOICE.md](docs/VOICE.md#building).
The same prerequisites build `sershi-semantic` (llama.cpp, the optional local
semantic model's runtime, [docs/SEMANTIC.md](docs/SEMANTIC.md)); `pnpm dev` and
`pnpm tauri build` build it next to SERSHI automatically. Its first build takes
about ten minutes.

## Before opening a pull request

```bash
pnpm check        # format, lint, typecheck, TS tests, build
pnpm check:rust   # rustfmt, clippy (-D warnings), cargo test
```

If you changed Rust types that cross IPC, run `pnpm contracts:generate` and commit
the generated files.

## Ground rules

- **Security boundaries are not negotiable.** No generic execution commands, no
  shell tools, no direct Tauri imports outside `apps/desktop/src/ipc`, no Win32
  outside `crates/sershi-platform/src/windows`. New tools need policy tests. Read
  [docs/SECURITY.md](docs/SECURITY.md).
- **Honesty in the UI.** Never show a feature as working when it isn't. Windows
  behaviour stays `REQUIRES_WINDOWS_VALIDATION` until checked on hardware.
- **Design tokens only.** No hard-coded colors, sizes or durations in components.
- **Rust:** typed errors, no `unwrap`/`expect`/`panic` in production code, small
  cohesive modules.
- **TypeScript:** strict; no `any`, no `@ts-ignore`; justify any cast in a comment.
- **Dependencies:** every new dependency needs a reason in the PR description.
- **Secrets:** never commit keys or tokens; `.env` files are ignored for a reason.
- Consequential decisions get an ADR in `docs/adr/`.

## Commits

Conventional-style prefixes (`feat:`, `fix:`, `docs:`, `chore:`, `refactor:`,
`test:`), one logical change per commit.

## License

By contributing you agree that your contributions are licensed under the
Apache License 2.0, the same license as the project.
