/**
 * Runtime guards for payloads crossing the IPC boundary. TypeScript types
 * vanish at runtime; these make a malformed payload fail loudly at the edge
 * instead of rendering nonsense deep inside the UI.
 */
import type { ActivityEntry } from "./generated/ActivityEntry";
import type { AssistantSnapshot } from "./generated/AssistantSnapshot";
import type { AssistantState } from "./generated/AssistantState";
import type { CommandOutcome } from "./generated/CommandOutcome";
import type { SystemSnapshot } from "./generated/SystemSnapshot";

/** Mirrors `AssistantState::ALL` in Rust. */
export const ASSISTANT_STATES = [
  "sleeping",
  "idle",
  "awake",
  "listening",
  "thinking",
  "planning",
  "executing",
  "speaking",
  "success",
  "warning",
  "error",
] as const satisfies readonly AssistantState[];

type Obj = Record<string, unknown>;

const isObj = (v: unknown): v is Obj => typeof v === "object" && v !== null && !Array.isArray(v);
const isNum = (v: unknown): v is number => typeof v === "number" && Number.isFinite(v);
const isStr = (v: unknown): v is string => typeof v === "string";
const isNullable = <T>(v: unknown, guard: (x: unknown) => x is T): v is T | null =>
  v === null || guard(v);

export function isAssistantState(v: unknown): v is AssistantState {
  return isStr(v) && (ASSISTANT_STATES as readonly string[]).includes(v);
}

export function isAssistantSnapshot(v: unknown): v is AssistantSnapshot {
  return (
    isObj(v) &&
    isAssistantState(v.state) &&
    isNullable(v.previewState, isAssistantState) &&
    isNum(v.revision)
  );
}

export function isActivityEntry(v: unknown): v is ActivityEntry {
  return (
    isObj(v) &&
    isNum(v.id) &&
    isNum(v.atMs) &&
    isStr(v.kind) &&
    isStr(v.summary) &&
    isNullable(v.toolId, isStr) &&
    isNullable(v.durationMs, isNum)
  );
}

export function isSystemSnapshot(v: unknown): v is SystemSnapshot {
  return (
    isObj(v) &&
    isObj(v.os) &&
    isStr(v.os.name) &&
    isStr(v.os.arch) &&
    isObj(v.cpu) &&
    isNum(v.cpu.logicalCores) &&
    isNullable(v.cpu.usagePercent, isNum) &&
    isObj(v.memory) &&
    isNum(v.memory.totalBytes) &&
    isNum(v.memory.usedBytes) &&
    isNum(v.uptimeSecs)
  );
}

export function isCommandOutcome(v: unknown): v is CommandOutcome {
  return isObj(v) && isStr(v.status) && isStr(v.reply) && isNullable(v.toolId, isStr);
}
