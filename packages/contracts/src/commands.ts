import type { ActivityEntry } from "./generated/ActivityEntry";
import type { AssistantSnapshot } from "./generated/AssistantSnapshot";
import type { AssistantState } from "./generated/AssistantState";
import type { CommandOutcome } from "./generated/CommandOutcome";
import type { CommandRequest } from "./generated/CommandRequest";
import type { RuntimeInfo } from "./generated/RuntimeInfo";
import type { SystemSnapshot } from "./generated/SystemSnapshot";

type NoArgs = Record<string, never>;

/**
 * Every IPC command the core exposes, with its arguments and result.
 * Names must match `COMMANDS` in `apps/desktop/src-tauri/build.rs`
 * (enforced by `test/commands.test.ts`).
 */
export interface CommandMap {
  get_assistant_snapshot: { args: NoArgs; result: AssistantSnapshot };
  get_system_snapshot: { args: NoArgs; result: SystemSnapshot };
  get_runtime_info: { args: NoArgs; result: RuntimeInfo };
  list_activity: { args: { limit: number }; result: ActivityEntry[] };
  submit_command: { args: { request: CommandRequest }; result: CommandOutcome };
  summon_command_center: { args: NoArgs; result: null };
  dismiss_assistant: { args: NoArgs; result: null };
  preview_assistant_state: { args: { state: AssistantState | null }; result: AssistantSnapshot };
  quit_app: { args: NoArgs; result: null };
}

export type CommandName = keyof CommandMap;

export const COMMAND_NAMES = [
  "get_assistant_snapshot",
  "get_system_snapshot",
  "get_runtime_info",
  "list_activity",
  "submit_command",
  "summon_command_center",
  "dismiss_assistant",
  "preview_assistant_state",
  "quit_app",
] as const satisfies readonly CommandName[];

/** Events broadcast by the core to every window. */
export const EVENTS = {
  assistantState: "sershi://assistant-state",
  activity: "sershi://activity",
} as const;

export interface EventMap {
  [EVENTS.assistantState]: AssistantSnapshot;
  [EVENTS.activity]: ActivityEntry;
}

export type EventName = keyof EventMap;
