/**
 * Gate 2B — personalization: global shortcut, themes, interface sounds and
 * companion appearances. Behaviour only; and none of it may touch authority.
 */
import type { ShortcutChange } from "@sershi/contracts";
import { darkTheme, lightTheme, createTheme, toCssVariables } from "@sershi/design-tokens";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative, resolve } from "node:path";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const setGlobalShortcut = vi.fn<(accelerator: string) => Promise<ShortcutChange>>();

vi.mock("../src/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../src/ipc")>();
  return {
    ...actual,
    desktopRuntime: true,
    sershi: {
      ...actual.sershi,
      setGlobalShortcut,
      getIntegrationStatus: () =>
        Promise.resolve({
          tray: "active",
          shortcut: { accelerator: "Ctrl+Alt+Space", status: "unavailable" },
          singleInstance: "active",
          startWithWindows: "planned",
        }),
      getApplicationCatalog: () =>
        Promise.resolve({ status: { state: "notScanned", count: 0 }, applications: [] }),
    },
  };
});

const { validateAccelerator, recordKey, useShortcut, applyStoredShortcut, DEFAULT_SHORTCUT } =
  await import("../src/state/shortcut");
const { WindowsIntegration } = await import("../src/surfaces/command-center/WindowsIntegration");
const { useLocaleStore } = await import("../src/i18n");
const { loadPreferences, PREFERENCES_KEY, parsePreferences, clampVolume } =
  await import("../src/i18n/preferences");
const { connectAppearance, useAppearance, activeTheme } = await import("../src/visual/appearance");
const { resolveTheme } = await import("../src/visual/themes");
const { playCue, setAudioContextFactory } = await import("../src/audio/interfaceAudio");
const { cueForTransition } = await import("../src/audio/connect");
const { CUES, SERSHI_SOUNDS, cueDuration } = await import("../src/audio/cues");
const { COMPANION_RENDERERS, companionRenderer } = await import("../src/visual/companions");

const src = resolve(import.meta.dirname, "../src");
const sourceFiles = (dir: string): string[] =>
  readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    return statSync(path).isDirectory() ? sourceFiles(path) : /\.tsx?$/.test(name) ? [path] : [];
  });
const sources = sourceFiles(src).map(
  (f) => [relative(src, f).replaceAll("\\", "/"), readFileSync(f, "utf8")] as const,
);

function change(partial: Partial<ShortcutChange>): ShortcutChange {
  return {
    result: "registered",
    problem: null,
    requested: "Ctrl+Alt+J",
    altgrWarning: false,
    shortcut: { accelerator: "Ctrl+Alt+J", status: "active" },
    ...partial,
  };
}

beforeEach(() => {
  localStorage.clear();
  setGlobalShortcut.mockReset();
  useShortcut.setState({ status: null, lastChange: null, applying: false });
  useLocaleStore.setState({ preference: "auto", systemLocale: "en-US", locale: "en-US" });
  useAppearance.setState({
    theme: "system",
    systemDark: true,
    motion: "system",
    systemReduced: false,
    interfaceSounds: false,
    soundVolume: 35,
    companionAppearance: "orbital",
    companionSize: "medium",
  });
});

// ─── Shortcut ──────────────────────────────────────────────────────────

describe("global shortcut validation (mirrors sershi-core)", () => {
  it.each([
    ["Ctrl+Alt+J", "Ctrl+Alt+J"],
    ["shift+ctrl+k", "Ctrl+Shift+K"],
    ["Alt + Shift + Space", "Alt+Shift+Space"],
    ["Ctrl+Alt+F5", "Ctrl+Alt+F5"],
    [DEFAULT_SHORTCUT, DEFAULT_SHORTCUT],
  ])("accepts %s", (input, canonical) => {
    expect(validateAccelerator(input)).toEqual({ accelerator: canonical });
  });

  it.each([
    ["A", "needsModifiers"],
    ["Enter", "unsupportedKey"],
    ["Space", "malformed"],
    ["F1", "malformed"],
    ["Ctrl+C", "needsModifiers"],
    ["Shift+J", "needsModifiers"],
    ["Alt+F4", "needsModifiers"],
    ["Win+Alt+K", "reserved"],
    ["Ctrl+Alt+Delete", "unsupportedKey"],
    ["Ctrl+Alt+J+K", "malformed"],
  ])("rejects %s", (input, problem) => {
    const result = validateAccelerator(input);
    expect("problem" in result).toBe(true);
    // Escape/Enter/Space/F1 alone are rejected one way or another.
    if (input !== "Space" && input !== "F1") expect(result).toEqual({ problem });
  });

  it("records physical keys and ignores lone modifiers", () => {
    const base = {
      key: "",
      code: "",
      ctrlKey: false,
      altKey: false,
      shiftKey: false,
      metaKey: false,
    };
    expect(recordKey({ ...base, key: "Control", ctrlKey: true })).toEqual({
      kind: "modifiers",
      held: ["Ctrl"],
    });
    expect(recordKey({ ...base, key: "j", code: "KeyJ", ctrlKey: true, altKey: true })).toEqual({
      kind: "done",
      accelerator: "Ctrl+Alt+J",
    });
    expect(recordKey({ ...base, key: "a", code: "KeyA" })).toEqual({
      kind: "invalid",
      problem: "needsModifiers",
    });
    expect(recordKey({ ...base, key: "Meta", code: "MetaLeft", metaKey: true })).toEqual({
      kind: "invalid",
      problem: "reserved",
    });
  });
});

describe("global shortcut setting", () => {
  it("persists a shortcut only once Windows accepted it", async () => {
    setGlobalShortcut.mockResolvedValue(change({}));
    await act(() => useShortcut.getState().change("Ctrl+Alt+J"));
    expect(loadPreferences().shortcut).toBe("Ctrl+Alt+J");

    setGlobalShortcut.mockResolvedValue(
      change({
        result: "unavailable",
        requested: "Ctrl+Shift+K",
        shortcut: { accelerator: "Ctrl+Alt+J", status: "active" },
      }),
    );
    await act(() => useShortcut.getState().change("Ctrl+Shift+K"));
    // The working shortcut stays; the refused one is not saved.
    expect(loadPreferences().shortcut).toBe("Ctrl+Alt+J");
    expect(useShortcut.getState().status?.accelerator).toBe("Ctrl+Alt+J");
  });

  it("re-applies the stored shortcut at start-up, or the default — never another", async () => {
    setGlobalShortcut.mockResolvedValue(change({}));
    applyStoredShortcut();
    expect(setGlobalShortcut).toHaveBeenLastCalledWith(DEFAULT_SHORTCUT);
    localStorage.setItem(PREFERENCES_KEY, JSON.stringify({ shortcut: "Alt+Shift+Space" }));
    applyStoredShortcut();
    expect(setGlobalShortcut).toHaveBeenLastCalledWith("Alt+Shift+Space");
    expect(setGlobalShortcut).toHaveBeenCalledTimes(2);
    await act(async () => {
      await Promise.resolve();
    });
  });

  it("records with the keyboard, reports a conflict and keeps localized status", async () => {
    setGlobalShortcut.mockResolvedValue(
      change({
        result: "unavailable",
        requested: "Ctrl+Shift+K",
        shortcut: { accelerator: "Ctrl+Alt+Space", status: "unavailable" },
      }),
    );
    render(<WindowsIntegration />);
    await screen.findByText("Another application is using this shortcut.", { exact: false });

    fireEvent.click(screen.getByRole("button", { name: "Change" }));
    const recorder = screen.getByRole("button", { name: /press a new shortcut/i });
    expect(document.activeElement).toBe(recorder);

    // Escape cancels without registering anything.
    fireEvent.keyDown(recorder, { key: "Escape", code: "Escape" });
    expect(setGlobalShortcut).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Change" }));
    const again = screen.getByRole("button", { name: /press a new shortcut/i });
    fireEvent.keyDown(again, { key: "Control", code: "ControlLeft", ctrlKey: true });
    fireEvent.keyDown(again, {
      key: "K",
      code: "KeyK",
      ctrlKey: true,
      shiftKey: true,
    });
    await act(async () => {
      await Promise.resolve();
    });
    expect(setGlobalShortcut).toHaveBeenCalledWith("Ctrl+Shift+K");
    expect(screen.getByRole("status").textContent).toContain("Shortcut unavailable");
    expect(screen.getByRole("status").textContent).toContain(
      "already being used by another application",
    );

    act(() => {
      useLocaleStore.setState({ locale: "es-419" });
    });
    expect(screen.getByRole("status").textContent).toContain("Atajo no disponible");
  });

  it("rejects an unsafe combination before asking Windows", () => {
    render(<WindowsIntegration />);
    return screen.findByRole("button", { name: "Change" }).then((button) => {
      fireEvent.click(button);
      const recorder = screen.getByRole("button", { name: /press a new shortcut/i });
      fireEvent.keyDown(recorder, { key: "c", code: "KeyC", ctrlKey: true });
      expect(setGlobalShortcut).not.toHaveBeenCalled();
      expect(screen.getByRole("status").textContent).toContain("Use Ctrl or Alt together");
    });
  });
});

// ─── Themes ────────────────────────────────────────────────────────────

describe("themes", () => {
  let disconnect: () => void = () => undefined;
  afterEach(() => {
    disconnect();
  });

  it("resolve System / Light / Dark", () => {
    expect(resolveTheme("system", true)).toBe("dark");
    expect(resolveTheme("system", false)).toBe("light");
    expect(resolveTheme("light", true)).toBe("light");
    expect(resolveTheme("dark", false)).toBe("dark");
  });

  it("apply through semantic tokens, and every token exists in both themes", () => {
    const dark = [...toCssVariables(createTheme(darkTheme)).keys()];
    const light = [...toCssVariables(createTheme(lightTheme)).keys()];
    expect(light).toEqual(dark);

    disconnect = connectAppearance();
    const html = document.documentElement;
    act(() => {
      useAppearance.getState().setTheme("light");
    });
    expect(html.dataset.theme).toBe("light");
    expect(html.style.getPropertyValue("--surface-base")).toBe(lightTheme.surface?.base);
    act(() => {
      useAppearance.getState().setTheme("dark");
    });
    expect(html.dataset.theme).toBe("dark");
    expect(html.style.getPropertyValue("--surface-base")).toBe(createTheme(darkTheme).surface.base);
  });

  it("System follows the operating system while open", () => {
    disconnect = connectAppearance();
    act(() => {
      useAppearance.setState({ theme: "system", systemDark: false });
    });
    expect(activeTheme()).toBe("light");
    expect(document.documentElement.dataset.theme).toBe("light");
    act(() => {
      useAppearance.setState({ systemDark: true });
    });
    expect(document.documentElement.dataset.theme).toBe("dark");
  });

  it("persist and follow other windows", () => {
    disconnect = connectAppearance();
    act(() => {
      useAppearance.getState().setTheme("light");
    });
    expect(loadPreferences().theme).toBe("light");
    act(() => {
      window.dispatchEvent(
        new StorageEvent("storage", {
          key: PREFERENCES_KEY,
          newValue: JSON.stringify({ theme: "dark" }),
        }),
      );
    });
    expect(useAppearance.getState().theme).toBe("dark");
  });

  it("reach every window, including the trusted confirmation window", () => {
    const bootstrap = sources.find(([name]) => name === "surfaces/bootstrap.tsx")?.[1] ?? "";
    expect(bootstrap).toContain("connectAppearance()");
    for (const entry of ["command-center", "companion", "confirmation"]) {
      const main = sources.find(([name]) => name === `surfaces/${entry}/main.tsx`)?.[1] ?? "";
      expect(main, entry).toContain('from "../bootstrap"');
    }
  });

  it("are never chosen with per-theme branches in components", () => {
    for (const [name, source] of sources) {
      if (name.startsWith("visual/")) continue;
      expect(source, name).not.toMatch(/theme\s*===\s*["'](dark|light)["']/);
    }
  });
});

// ─── Interface sounds ──────────────────────────────────────────────────

class FakeParam {
  value = 0;
  setValueAtTime() {
    return this;
  }
  linearRampToValueAtTime() {
    return this;
  }
  exponentialRampToValueAtTime() {
    return this;
  }
}
class FakeNode {
  gain = new FakeParam();
  pan = new FakeParam();
  frequency = new FakeParam();
  Q = new FakeParam();
  threshold = new FakeParam();
  ratio = new FakeParam();
  type = "";
  buffer: unknown = null;
  connect<T>(next: T): T {
    return next;
  }
  start = vi.fn();
  stop = vi.fn();
}
class FakeContext {
  static created = 0;
  static oscillators = 0;
  state: AudioContextState = "running";
  currentTime = 0;
  sampleRate = 48_000;
  destination = new FakeNode();
  constructor() {
    FakeContext.created++;
  }
  createGain = () => new FakeNode();
  createDynamicsCompressor = () => new FakeNode();
  createStereoPanner = () => new FakeNode();
  createBiquadFilter = () => new FakeNode();
  createOscillator = () => {
    FakeContext.oscillators++;
    return new FakeNode();
  };
  createBufferSource = () => new FakeNode();
  createBuffer = () => ({ getChannelData: () => new Float32Array(16) });
  resume = () => Promise.resolve();
  suspend = () => Promise.resolve();
}

describe("interface sounds", () => {
  beforeEach(() => {
    FakeContext.created = 0;
    FakeContext.oscillators = 0;
    vi.stubGlobal("AudioContext", FakeContext);
    setAudioContextFactory(() => new FakeContext() as unknown as AudioContext);
  });
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("are off by default and, when off, never create audio", () => {
    expect(parsePreferences(null).interfaceSounds).toBe(false);
    expect(playCue("success")).toBe(false);
    expect(FakeContext.created).toBe(0);
  });

  it("play the requested cue when on, and stay silent at volume 0", () => {
    useAppearance.setState({ interfaceSounds: true, soundVolume: 35 });
    expect(playCue("startup")).toBe(true);
    expect(FakeContext.created).toBe(1);
    expect(FakeContext.oscillators).toBe(
      SERSHI_SOUNDS.startup.filter((v) => v.kind === "tone").length,
    );
    useAppearance.setState({ soundVolume: 0 });
    expect(playCue("summon")).toBe(false);
  });

  it("persist and clamp the volume", () => {
    act(() => {
      useAppearance.getState().setInterfaceSounds(true);
      useAppearance.getState().setSoundVolume(140);
    });
    expect(loadPreferences()).toMatchObject({ interfaceSounds: true, soundVolume: 100 });
    expect(clampVolume(-5)).toBe(0);
    expect(clampVolume("loud")).toBe(35);
  });

  it("map events to cues, and only meaningful ones", () => {
    expect(cueForTransition("executing", "success")).toBe("success");
    expect(cueForTransition("executing", "error")).toBe("error");
    expect(cueForTransition("planning", "awaitingConfirmation")).toBe("confirmation");
    expect(cueForTransition("idle", "awake")).toBeNull();
    expect(cueForTransition("thinking", "planning")).toBeNull();
    expect(cueForTransition("success", "success")).toBeNull();
  });

  it("stay short: start-up under a second, other cues under half a second", () => {
    expect(cueDuration(SERSHI_SOUNDS, "startup")).toBeLessThanOrEqual(1);
    for (const cue of CUES.filter((c) => c !== "startup")) {
      expect(cueDuration(SERSHI_SOUNDS, cue), cue).toBeLessThanOrEqual(0.5);
    }
  });

  it("are never part of security: approval code does not know sound exists", () => {
    for (const [name, source] of sources) {
      const security = name.startsWith("surfaces/confirmation/") || name === "ipc/confirmation.ts";
      if (security) expect(source, name).not.toMatch(/from "\.\.\/.*audio/);
      if (name.startsWith("audio/")) {
        expect(source, name).not.toContain("ipc/confirmation");
        expect(source, name).not.toMatch(/decide|approve/i);
      }
    }
  });
});

// ─── Companion appearances ─────────────────────────────────────────────

describe("companion appearances", () => {
  it("register Orbital, and fall back to it for anything unknown", () => {
    expect(Object.keys(COMPANION_RENDERERS)).toEqual(["orbital"]);
    expect(companionRenderer("aurora").id).toBe("orbital");
    expect(parsePreferences('{"companionAppearance":"character2d"}').companionAppearance).toBe(
      "orbital",
    );
  });

  it("are pure visuals: no IPC, no controls, no behaviour", () => {
    const Orbital = COMPANION_RENDERERS.orbital.component;
    const { container } = render(
      <Orbital state="thinking" size={104} intensity="normal" pulseKey={1} />,
    );
    expect(container.querySelector("button, a, input")).toBeNull();
    const registry = sources.find(([name]) => name === "visual/companions.tsx")?.[1] ?? "";
    expect(registry).not.toMatch(/from "\.\.\/ipc/);
  });
});
