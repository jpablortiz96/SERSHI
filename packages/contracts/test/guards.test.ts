/**
 * Contract tests: fixtures are serialized by Rust (see
 * crates/sershi-core/src/contract_fixtures.rs); the guards and types here must
 * accept exactly that shape.
 */
import { describe, expect, expectTypeOf, it } from "vitest";

import activity from "../fixtures/activity.json";
import ambiguous from "../fixtures/command-outcome-ambiguous.json";
import confirmation from "../fixtures/command-outcome-confirmation.json";
import notFound from "../fixtures/command-outcome-not-found.json";
import opened from "../fixtures/command-outcome-opened.json";
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
  isConfirmationRequest,
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

  it("every tool declares the expected risk and permission, camelCased", () => {
    const expected: Record<string, [string, string]> = {
      "system.get_info": ["safe", "system.info.read"],
      "system.get_memory": ["safe", "system.info.read"],
      "system.get_cpu": ["safe", "system.info.read"],
      "system.open_application": ["safe", "system.apps.launch"],
      "system.close_application": ["sensitive", "system.apps.close"],
    };
    expect(toolDefinitions.map((t) => t.id).sort()).toEqual(Object.keys(expected).sort());
    for (const tool of toolDefinitions) {
      expect([tool.risk, tool.permissions.join()], tool.id).toEqual(expected[tool.id]);
      expect(Object.keys(tool)).toContain("inputSchema");
    }
  });

  it("application tools accept a name only — never a path, command or arguments", () => {
    for (const id of ["system.open_application", "system.close_application"]) {
      const tool = toolDefinitions.find((t) => t.id === id);
      const schema = tool?.inputSchema as {
        properties: Record<string, unknown>;
        additionalProperties: boolean;
      };
      expect(Object.keys(schema.properties)).toEqual(["application"]);
      expect(schema.additionalProperties).toBe(false);
      expect(tool?.platforms).toEqual(["windows"]);
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

  it("confirmation requests carry only structured, trusted fields", () => {
    const request = confirmation.confirmation;
    expect(isConfirmationRequest(request)).toBe(true);
    expect(Object.keys(request).sort()).toEqual(
      ["action", "canRemember", "expiresAtMs", "id", "reason", "risk", "subject", "toolId"].sort(),
    );
    expect(request.canRemember).toBe(false);
    expect(request.subject.application.displayName).toBe("Spotify");
    const serialized = JSON.stringify(request);
    expect(serialized).not.toMatch(/[A-Za-z]:\\|\.exe|AppData/i);
  });

  it("application outcomes never expose paths or launch targets", () => {
    for (const outcome of [opened, notFound, ambiguous, confirmation]) {
      expect(isCommandOutcome(outcome)).toBe(true);
      expect(JSON.stringify(outcome)).not.toMatch(/\\\\|\.exe|aumid|SpotifyAB/i);
    }
  });

  it("the activity log never carries user text fields", () => {
    for (const entry of activity) {
      expect(Object.keys(entry).sort()).toEqual(
        ["atMs", "durationMs", "id", "kind", "subject", "summary", "toolId"].sort(),
      );
    }
  });
});
