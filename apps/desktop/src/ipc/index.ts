import {
  EVENTS,
  isActivityEntry,
  isAssistantSnapshot,
  isCommandOutcome,
  isSystemSnapshot,
  type ActivityEntry,
  type AssistantSnapshot,
  type AssistantState,
} from "@sershi/contracts";

import { call, subscribe } from "./client";

export { desktopRuntime, IpcFailure } from "./client";
export { currentWindow } from "./window";

const isActivityList = (v: unknown): v is ActivityEntry[] =>
  Array.isArray(v) && v.every(isActivityEntry);

/** Typed facade over every SERSHI IPC command and event. */
export const sershi = {
  getAssistantSnapshot: () => call("get_assistant_snapshot", {}, isAssistantSnapshot),
  getSystemSnapshot: () => call("get_system_snapshot", {}, isSystemSnapshot),
  getRuntimeInfo: () => call("get_runtime_info", {}),
  listActivity: (limit: number) => call("list_activity", { limit }, isActivityList),
  submitCommand: (text: string) => call("submit_command", { request: { text } }, isCommandOutcome),
  summonCommandCenter: () => call("summon_command_center", {}),
  dismissAssistant: () => call("dismiss_assistant", {}),
  previewState: (state: AssistantState | null) =>
    call("preview_assistant_state", { state }, isAssistantSnapshot),
  quit: () => call("quit_app", {}),

  onAssistantState: (handler: (snapshot: AssistantSnapshot) => void) =>
    subscribe(EVENTS.assistantState, isAssistantSnapshot, handler),
  onActivity: (handler: (entry: ActivityEntry) => void) =>
    subscribe(EVENTS.activity, isActivityEntry, handler),
};
