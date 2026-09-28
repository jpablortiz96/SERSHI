import {
  EVENTS,
  isActivityEntry,
  isAssistantSnapshot,
  isCommandOutcome,
  isNullPayload,
  isSystemSnapshot,
  type ActivityEntry,
  type AssistantSnapshot,
  type AssistantState,
  type CommandOutcome,
  type TrayLabels,
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
  getApplicationCatalog: () => call("get_application_catalog", {}),
  refreshApplicationCatalog: () => call("refresh_application_catalog", {}),
  getIntegrationStatus: () => call("get_integration_status", {}),
  setTrayLabels: (labels: TrayLabels) => call("set_tray_labels", { labels }),
  summonCommandCenter: () => call("summon_command_center", {}),
  hideCommandCenter: () => call("hide_command_center", {}),
  dismissAssistant: () => call("dismiss_assistant", {}),
  previewState: (state: AssistantState | null) =>
    call("preview_assistant_state", { state }, isAssistantSnapshot),
  quit: () => call("quit_app", {}),

  onAssistantState: (handler: (snapshot: AssistantSnapshot) => void) =>
    subscribe(EVENTS.assistantState, isAssistantSnapshot, handler),
  onActivity: (handler: (entry: ActivityEntry) => void) =>
    subscribe(EVENTS.activity, isActivityEntry, handler),
  onFocusCommand: (handler: () => void) => subscribe(EVENTS.focusCommand, isNullPayload, handler),
  /** Outcomes of confirmations decided on the trusted surface (or expired). */
  onCommandOutcome: (handler: (outcome: CommandOutcome) => void) =>
    subscribe(EVENTS.commandOutcome, isCommandOutcome, handler),
};
