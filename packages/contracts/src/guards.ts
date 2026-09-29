/**
 * Runtime guards for payloads crossing the IPC boundary. TypeScript types
 * vanish at runtime; these make a malformed payload fail loudly at the edge
 * instead of rendering nonsense deep inside the UI.
 */
import type { ActivityEntry } from "./generated/ActivityEntry";
import type { AssistantSnapshot } from "./generated/AssistantSnapshot";
import type { AssistantState } from "./generated/AssistantState";
import type { CommandOutcome } from "./generated/CommandOutcome";
import type { PresenceUpdate } from "./generated/PresenceUpdate";
import type { ShortcutChange } from "./generated/ShortcutChange";
import type { ConfirmationRequest } from "./generated/ConfirmationRequest";
import type { SystemSnapshot } from "./generated/SystemSnapshot";
import type { CaptureStart } from "./generated/CaptureStart";
import type { ModelProgress } from "./generated/ModelProgress";
import type { VoiceLevel } from "./generated/VoiceLevel";
import type { VoiceStatus } from "./generated/VoiceStatus";
import type { VoiceUpdate } from "./generated/VoiceUpdate";

/** Mirrors `AssistantState::ALL` in Rust. */
export const ASSISTANT_STATES = [
  "sleeping",
  "idle",
  "awake",
  "listening",
  "transcribing",
  "thinking",
  "planning",
  "executing",
  "speaking",
  "success",
  "warning",
  "awaitingConfirmation",
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
    isNullable(v.subject, isStr) &&
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

export function isConfirmationRequest(v: unknown): v is ConfirmationRequest {
  return (
    isObj(v) &&
    isStr(v.id) &&
    /^[0-9a-f]{32}$/.test(v.id) &&
    isStr(v.toolId) &&
    isStr(v.action) &&
    isStr(v.risk) &&
    isStr(v.level) &&
    isNum(v.expiresAtMs) &&
    typeof v.canRemember === "boolean"
  );
}

export function isCommandOutcome(v: unknown): v is CommandOutcome {
  return (
    isObj(v) &&
    isStr(v.status) &&
    isStr(v.reply) &&
    isNullable(v.toolId, isStr) &&
    // The Command Center never receives confirmation authorization data.
    !("confirmation" in v)
  );
}

export function isShortcutChange(v: unknown): v is ShortcutChange {
  return (
    isObj(v) &&
    isStr(v.result) &&
    typeof v.altgrWarning === "boolean" &&
    isObj(v.shortcut) &&
    isStr(v.shortcut.accelerator) &&
    isStr(v.shortcut.status)
  );
}

export function isPresenceUpdate(v: unknown): v is PresenceUpdate {
  return isObj(v) && typeof v.commandCenterVisible === "boolean";
}

const VOICE_UPDATE_KINDS = [
  "heard",
  "answered",
  "noSpeech",
  "unclear",
  "cancelled",
  "failed",
  "deviceFallback",
] as const satisfies readonly VoiceUpdate["kind"][];

export function isVoiceUpdate(v: unknown): v is VoiceUpdate {
  if (!isObj(v) || !isStr(v.kind)) return false;
  if (!(VOICE_UPDATE_KINDS as readonly string[]).includes(v.kind)) return false;
  switch (v.kind) {
    case "heard":
      return isStr(v.text) && isNullable(v.language, isStr);
    case "answered":
      return isCommandOutcome(v.outcome);
    case "failed":
      return isStr(v.reason);
    default:
      return true;
  }
}

export function isVoiceLevel(v: unknown): v is VoiceLevel {
  return (
    isObj(v) &&
    isNum(v.level) &&
    v.level >= 0 &&
    v.level <= 1 &&
    (v.source === "input" || v.source === "output")
  );
}

function isModelState(v: unknown): boolean {
  return isObj(v) && isStr(v.kind) && (v.kind !== "downloading" || isNum(v.receivedBytes));
}

export function isModelProgress(v: unknown): v is ModelProgress {
  return isObj(v) && isStr(v.model) && isModelState(v.state) && isNullable(v.error, isStr);
}

export function isVoiceStatus(v: unknown): v is VoiceStatus {
  return (
    isObj(v) &&
    typeof v.supported === "boolean" &&
    isObj(v.microphone) &&
    isStr(v.microphone.access) &&
    Array.isArray(v.microphone.devices) &&
    v.microphone.devices.every((d) => isObj(d) && isStr(d.id) && isStr(d.name)) &&
    typeof v.microphone.fallback === "boolean" &&
    Array.isArray(v.models) &&
    v.models.every((m) => isObj(m) && isStr(m.id) && isNum(m.sizeBytes) && isModelState(m.state)) &&
    isStr(v.model) &&
    Array.isArray(v.voices) &&
    v.voices.every((x) => isObj(x) && isStr(x.id) && isStr(x.name) && isStr(x.language)) &&
    typeof v.capturing === "boolean" &&
    typeof v.speaking === "boolean"
  );
}

export function isCaptureStart(v: unknown): v is CaptureStart {
  return isObj(v) && (v.kind === "started" || (v.kind === "refused" && isStr(v.reason)));
}

/** `null` payload (e.g. the focus-command event). */
export function isNullPayload(v: unknown): v is null {
  return v === null;
}
