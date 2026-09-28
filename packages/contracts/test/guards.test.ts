/**
 * Contract tests: fixtures are serialized by Rust (see
 * crates/sershi-core/src/contract_fixtures.rs); the guards and types here must
 * accept exactly that shape.
 */
import { describe, expect, expectTypeOf, it } from "vitest";

import activity from "../fixtures/activity.json";
import assistantSnapshot from "../fixtures/assistant-snapshot.json";
import completed from "../fixtures/command-outcome-completed.json";
import unavailable from "../fixtures/command-outcome-unavailable.json";
import systemSnapshot from "../fixtures/system-snapshot.json";
import toolDefinitions from "../fixtures/tool-definitions.json";
import type { AssistantState } from "../src";
import {
  ASSISTANT_STATES,
  isActivityEntry,
  isAssistantSnapshot,
  isCommandOutcome,
  isSystemSnapshot,
} from "../src/guards";

describe("Rust-serialized fixtures satisfy the TypeScript contracts", () => {
  it("assistant snapshot", () => {
    expect(isAssistantSnapshot(assistantSnapshot)).toBe(true);
  });

  it("system snapshot", () => {
    expect(isSystemSnapshot(systemSnapshot)).toBe(true);
  });

  it("activity entries", () => {
    expect(activity.length).toBeGreaterThan(0);
    expect(activity.every(isActivityEntry)).toBe(true);
  });

  it("command outcomes", () => {
    expect(isCommandOutcome(completed)).toBe(true);
    expect(completed.status).toBe("completed");
    expect(isCommandOutcome(unavailable)).toBe(true);
    expect(unavailable.status).toBe("unavailable");
  });

  it("tool definitions are all safe, permissioned and camelCased", () => {
    for (const tool of toolDefinitions) {
      expect(tool.risk).toBe("safe");
      expect(tool.permissions.length).toBeGreaterThan(0);
      expect(Object.keys(tool)).toContain("inputSchema");
    }
  });
});

describe("state list", () => {
  it("covers every Rust AssistantState (checked at compile time)", () => {
    expectTypeOf<
      Exclude<AssistantState, (typeof ASSISTANT_STATES)[number]>
    >().toEqualTypeOf<never>();
    expect(new Set(ASSISTANT_STATES).size).toBe(ASSISTANT_STATES.length);
  });
});

describe("guards reject malformed payloads", () => {
  it.each([
    null,
    {},
    { state: "dancing", previewState: null, revision: 1 },
    { state: "idle", previewState: null, revision: "1" },
  ])("rejects %j as an assistant snapshot", (value) => {
    expect(isAssistantSnapshot(value)).toBe(false);
  });

  it("the activity log never carries user text fields", () => {
    for (const entry of activity) {
      expect(Object.keys(entry).sort()).toEqual(
        ["atMs", "durationMs", "id", "kind", "summary", "toolId"].sort(),
      );
    }
  });
});
