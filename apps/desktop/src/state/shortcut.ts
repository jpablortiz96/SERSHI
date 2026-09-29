/**
 * The global summon shortcut, as the Command Center sees it.
 *
 * Rust owns registration (`set_global_shortcut`) and validates every
 * request; this module records a key combination, mirrors the validation
 * for instant feedback, persists the user's choice once Windows accepted it,
 * and re-applies it at start-up. Invocation only — a shortcut can bring
 * SERSHI forward, it never grants a capability or approves anything.
 */
import type { ShortcutChange, ShortcutProblem, ShortcutStatus } from "@sershi/contracts";
import { create } from "zustand";

import { loadPreferences, savePreferences } from "../i18n/preferences";
import { desktopRuntime, sershi } from "../ipc";

/** Mirrors `DEFAULT_SHORTCUT` in sershi-core. */
export const DEFAULT_SHORTCUT = "Ctrl+Alt+Space";

type Parsed = { accelerator: string } | { problem: ShortcutProblem };

/** Mirrors `Accelerator::parse` in sershi-core (Rust stays authoritative). */
export function validateAccelerator(text: string): Parsed {
  if (!text || text.length > 40) return { problem: "malformed" };
  let ctrl = false;
  let alt = false;
  let shift = false;
  let key: string | null = null;
  for (const raw of text.split("+")) {
    const token = raw.trim().toUpperCase();
    if (["WIN", "SUPER", "META", "CMD", "COMMAND"].includes(token)) return { problem: "reserved" };
    if (token === "CTRL" || token === "CONTROL") {
      if (ctrl) return { problem: "malformed" };
      ctrl = true;
    } else if (token === "ALT" || token === "OPTION") {
      if (alt) return { problem: "malformed" };
      alt = true;
    } else if (token === "SHIFT") {
      if (shift) return { problem: "malformed" };
      shift = true;
    } else {
      if (key !== null) return { problem: "malformed" };
      if (/^[A-Z0-9]$/.test(token)) key = token;
      else if (token === "SPACE") key = "Space";
      else if (/^F([1-9]|1[0-2])$/.test(token)) key = token;
      else return { problem: "unsupportedKey" };
    }
  }
  if (key === null) return { problem: "malformed" };
  const modifiers = [ctrl, alt, shift].filter(Boolean).length;
  if (modifiers < 2 || !(ctrl || alt)) return { problem: "needsModifiers" };
  const parts = [ctrl && "Ctrl", alt && "Alt", shift && "Shift", key].filter(Boolean);
  return { accelerator: parts.join("+") };
}

/** Result of one key press while recording. */
export type Recording =
  | { kind: "modifiers"; held: string[] }
  | { kind: "done"; accelerator: string }
  | { kind: "invalid"; problem: ShortcutProblem };

const MODIFIER_KEYS = new Set(["Control", "Alt", "Shift", "Meta", "AltGraph", "OS"]);

/** Turns a keydown into a combination, using physical keys (`code`). */
export function recordKey(
  e: Pick<KeyboardEvent, "key" | "code" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey">,
): Recording {
  if (e.metaKey || e.key === "Meta" || e.key === "OS")
    return { kind: "invalid", problem: "reserved" };
  const held = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift"].filter(
    (m): m is string => Boolean(m),
  );
  if (MODIFIER_KEYS.has(e.key)) return { kind: "modifiers", held };
  const key = /^Key[A-Z]$/.test(e.code)
    ? e.code.slice(3)
    : /^Digit[0-9]$/.test(e.code)
      ? e.code.slice(5)
      : e.code === "Space"
        ? "Space"
        : /^F([1-9]|1[0-2])$/.test(e.code)
          ? e.code
          : null;
  if (key === null) return { kind: "invalid", problem: "unsupportedKey" };
  const parsed = validateAccelerator([...held, key].join("+"));
  return "accelerator" in parsed
    ? { kind: "done", accelerator: parsed.accelerator }
    : { kind: "invalid", problem: parsed.problem };
}

interface ShortcutStore {
  status: ShortcutStatus | null;
  lastChange: ShortcutChange | null;
  applying: boolean;
  setStatus: (status: ShortcutStatus) => void;
  /** Asks Rust to register `accelerator`; persists it only if Windows accepted. */
  change: (accelerator: string) => Promise<ShortcutChange | null>;
  clearFeedback: () => void;
}

export const useShortcut = create<ShortcutStore>((set) => ({
  status: null,
  lastChange: null,
  applying: false,
  setStatus: (status) => {
    set({ status });
  },
  change: async (accelerator) => {
    if (!desktopRuntime) return null;
    set({ applying: true });
    try {
      const change = await sershi.setGlobalShortcut(accelerator);
      if ((change.result === "registered" || change.result === "unchanged") && change.requested) {
        savePreferences({ ...loadPreferences(), shortcut: change.requested });
      }
      set({ lastChange: change, status: change.shortcut });
      return change;
    } catch {
      return null;
    } finally {
      set({ applying: false });
    }
  },
  clearFeedback: () => {
    set({ lastChange: null });
  },
}));

/**
 * Start-up (Command Center only): register the user's stored shortcut, or
 * SERSHI's default. If it is unavailable, it is reported as such — SERSHI
 * does not pick a different combination on its own.
 */
export function applyStoredShortcut(): void {
  if (!desktopRuntime) return;
  const stored = loadPreferences().shortcut ?? DEFAULT_SHORTCUT;
  void sershi.setGlobalShortcut(stored).then(
    (change) => {
      useShortcut.getState().setStatus(change.shortcut);
    },
    () => undefined,
  );
}
