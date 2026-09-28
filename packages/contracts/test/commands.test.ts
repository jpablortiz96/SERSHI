/**
 * Contract + security tests for the IPC surface: TypeScript command names
 * must match the Rust shell, and each window's capability file must grant
 * only commands that exist — the companion only a minimal set.
 */
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { describe, expect, it } from "vitest";

import { COMMAND_NAMES } from "../src/commands";

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
