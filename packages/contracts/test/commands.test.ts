/**
 * Contract + security tests for the IPC surface: TypeScript command names
 * must match the Rust shell, and each window's capability file must grant
 * only commands that exist — the companion only a minimal set.
 */
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { describe, expect, it } from "vitest";

import { COMMAND_NAMES, EVENTS } from "../src/commands";

const tauriDir = resolve(import.meta.dirname, "../../../apps/desktop/src-tauri");

function rustCommands(): string[] {
  const buildRs = readFileSync(resolve(tauriDir, "build.rs"), "utf8");
  const block = /const COMMANDS: &\[&str\] = &\[([\s\S]*?)\];/.exec(buildRs)?.[1] ?? "";
  return [...block.matchAll(/"([a-z_]+)"/g)].map((m) => m[1] ?? "");
}

interface Capability {
  windows: string[];
  permissions: string[];
}

function capability(name: string): Capability {
  return JSON.parse(
    readFileSync(resolve(tauriDir, "capabilities", `${name}.json`), "utf8"),
  ) as Capability;
}

function allPermissions(name: string): string[] {
  return capability(name).permissions;
}

function capabilityWindows(name: string): string[] {
  return capability(name).windows;
}

function grantedCommands(name: string): string[] {
  return capability(name)
    .permissions.filter((p) => p.startsWith("allow-"))
    .map((p) => p.slice("allow-".length).replaceAll("-", "_"));
}

describe("IPC command surface", () => {
  it("TypeScript and Rust declare the same commands", () => {
    expect([...COMMAND_NAMES].sort()).toEqual(rustCommands().sort());
  });

  it("capabilities only grant declared commands", () => {
    for (const cap of ["command-center", "companion", "confirmation"]) {
      for (const command of grantedCommands(cap)) {
        expect(COMMAND_NAMES).toContain(command);
      }
    }
  });

  it("the companion window can only read state and summon the Command Center", () => {
    expect(grantedCommands("companion").sort()).toEqual(
      ["get_assistant_snapshot", "summon_command_center"].sort(),
    );
    expect(allPermissions("companion")).not.toContain("allow-decide-confirmation");
  });

  it("the Command Center cannot decide or read confirmations", () => {
    const granted = grantedCommands("command-center");
    expect(granted).not.toContain("decide_confirmation");
    expect(granted).not.toContain("get_confirmation_context");
  });

  it("the confirmation window has exactly two commands and no other permission", () => {
    expect(allPermissions("confirmation").sort()).toEqual(
      ["allow-decide-confirmation", "allow-get-confirmation-context"].sort(),
    );
    expect(capabilityWindows("confirmation")).toEqual(["confirmation"]);
  });

  it("only the confirmation window can decide confirmations", () => {
    const deciders = ["command-center", "companion", "confirmation"].filter((cap) =>
      grantedCommands(cap).includes("decide_confirmation"),
    );
    expect(deciders).toEqual(["confirmation"]);
  });

  it("every capability names its window explicitly (no wildcards)", () => {
    expect(capabilityWindows("command-center")).toEqual(["main"]);
    expect(capabilityWindows("companion")).toEqual(["companion"]);
    for (const cap of ["command-center", "companion", "confirmation"]) {
      for (const window of capabilityWindows(cap)) {
        expect(window).not.toMatch(/[*?]/);
      }
    }
  });

  it("no command resembles generic execution", () => {
    for (const name of COMMAND_NAMES) {
      expect(name).not.toMatch(/exec|shell|run_command|eval|spawn/);
    }
  });
});

/**
 * Gate 1A invariants. Visual work (Prompt 2 and later) must never change
 * these; if one fails, a change widened an authority boundary.
 */
describe("Gate 1A invariants survive visual work", () => {
  it("the confirmation window still has exactly two commands", () => {
    expect(grantedCommands("confirmation").sort()).toEqual(
      ["decide_confirmation", "get_confirmation_context"].sort(),
    );
    expect(allPermissions("confirmation")).toHaveLength(2);
  });

  it("the Command Center still cannot approve or read confirmations", () => {
    expect(grantedCommands("command-center")).not.toContain("decide_confirmation");
    expect(grantedCommands("command-center")).not.toContain("get_confirmation_context");
  });

  it("the companion still has only state, summon and event listening", () => {
    expect(allPermissions("companion").sort()).toEqual(
      [
        "allow-get-assistant-snapshot",
        "allow-summon-command-center",
        "core:event:allow-listen",
        "core:event:allow-unlisten",
        "core:window:allow-start-dragging",
      ].sort(),
    );
  });

  it("contextual presence is an event from Rust, not a command anyone can call", () => {
    expect(EVENTS.presence).toBe("sershi://presence");
    expect(COMMAND_NAMES.some((c) => c.includes("presence"))).toBe(false);
  });

  it("no command creates windows or executes anything generic", () => {
    for (const name of COMMAND_NAMES) {
      expect(name).not.toMatch(/exec|shell|spawn|eval|create_window|open_window|webview/);
    }
    for (const cap of ["command-center", "companion", "confirmation"]) {
      for (const permission of allPermissions(cap)) {
        expect(permission).not.toMatch(/webview|create|core:window:default|core:default/);
      }
    }
  });
});

/**
 * Gate 2B: personalization never becomes privilege. Theme, sound, companion
 * appearance and shortcut are presentation or invocation only.
 */
describe("Gate 2B: personalization is not privilege", () => {
  it("only the Command Center can change the global shortcut", () => {
    const owners = ["command-center", "companion", "confirmation"].filter((cap) =>
      grantedCommands(cap).includes("set_global_shortcut"),
    );
    expect(owners).toEqual(["command-center"]);
  });

  it("theme, sound and appearance need no IPC at all", () => {
    for (const name of COMMAND_NAMES) {
      expect(name).not.toMatch(/theme|sound|audio|appearance|companion_pack|skin/);
    }
  });

  it("the approval boundary is unchanged", () => {
    expect(grantedCommands("confirmation").sort()).toEqual(
      ["decide_confirmation", "get_confirmation_context"].sort(),
    );
    for (const cap of ["command-center", "companion"]) {
      expect(grantedCommands(cap)).not.toContain("decide_confirmation");
      expect(grantedCommands(cap)).not.toContain("get_confirmation_context");
    }
  });
});

/**
 * Gate 3A: voice is input and output, never authorization. Voice commands
 * belong to the Command Center alone; the companion and the trusted
 * confirmation surface gain nothing.
 */
describe("Gate 3A: voice does not grant authority", () => {
  const VOICE_COMMANDS = [
    "get_voice_status",
    "configure_voice",
    "start_voice_capture",
    "stop_voice_capture",
    "cancel_voice_capture",
    "speak_reply",
    "stop_speaking",
    "download_voice_model",
    "cancel_voice_model_download",
  ];

  it("voice commands exist and only the Command Center may call them", () => {
    for (const command of VOICE_COMMANDS) {
      expect(COMMAND_NAMES).toContain(command);
      const holders = ["command-center", "companion", "confirmation"].filter((cap) =>
        grantedCommands(cap).includes(command),
      );
      expect(holders, command).toEqual(["command-center"]);
    }
  });

  it("the confirmation surface is unchanged: exactly two commands, no voice", () => {
    expect(allPermissions("confirmation").sort()).toEqual(
      ["allow-decide-confirmation", "allow-get-confirmation-context"].sort(),
    );
  });

  it("the companion is unchanged: no microphone, no speech, no approval", () => {
    const companion = grantedCommands("companion");
    expect(companion.sort()).toEqual(["get_assistant_snapshot", "summon_command_center"].sort());
    for (const command of VOICE_COMMANDS) expect(companion).not.toContain(command);
  });

  it("no voice command can approve, decide or touch permissions", () => {
    for (const command of VOICE_COMMANDS) {
      expect(command).not.toMatch(/approve|decide|confirm|permission|grant|credential|exec|shell/);
    }
    // Still exactly one command that decides, and it is not reachable from voice.
    expect(COMMAND_NAMES.filter((c) => /decide|approve/.test(c))).toEqual(["decide_confirmation"]);
  });

  it("raw audio never crosses IPC: voice events carry text or a level", () => {
    expect(EVENTS.voice).toBe("sershi://voice");
    expect(EVENTS.voiceLevel).toBe("sershi://voice-level");
    for (const name of COMMAND_NAMES) expect(name).not.toMatch(/audio|samples|pcm|recording/);
  });
});
