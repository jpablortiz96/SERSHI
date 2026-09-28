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

function grantedCommands(capability: string): string[] {
  const json = JSON.parse(
    readFileSync(resolve(tauriDir, "capabilities", `${capability}.json`), "utf8"),
  ) as { permissions: string[] };
  return json.permissions
    .filter((p) => p.startsWith("allow-"))
    .map((p) => p.slice("allow-".length).replaceAll("-", "_"));
}

describe("IPC command surface", () => {
  it("TypeScript and Rust declare the same commands", () => {
    expect([...COMMAND_NAMES].sort()).toEqual(rustCommands().sort());
  });

  it("capabilities only grant declared commands", () => {
    for (const cap of ["command-center", "companion"]) {
      for (const command of grantedCommands(cap)) {
        expect(COMMAND_NAMES).toContain(command);
      }
    }
  });

  it("the companion window can only read state and summon the Command Center", () => {
    expect(grantedCommands("companion").sort()).toEqual(
      ["get_assistant_snapshot", "summon_command_center"].sort(),
    );
  });

  it("confirmation decisions are main-window only; the companion cannot approve anything", () => {
    expect(grantedCommands("command-center")).toContain("decide_confirmation");
    expect(grantedCommands("companion")).not.toContain("decide_confirmation");
  });

  it("no command resembles generic execution", () => {
    for (const name of COMMAND_NAMES) {
      expect(name).not.toMatch(/exec|shell|run_command|eval|spawn/);
    }
  });
});
