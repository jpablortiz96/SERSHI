import type { ActivityEntry } from "./generated/ActivityEntry";
import type { ApplicationCatalogInfo } from "./generated/ApplicationCatalogInfo";
import type { AssistantSnapshot } from "./generated/AssistantSnapshot";
import type { AssistantState } from "./generated/AssistantState";
import type { CaptureStart } from "./generated/CaptureStart";
import type { CommandOutcome } from "./generated/CommandOutcome";
import type { CommandRequest } from "./generated/CommandRequest";
import type { ConfirmationDecision } from "./generated/ConfirmationDecision";
import type { ConfirmationRequest } from "./generated/ConfirmationRequest";
import type { IntegrationStatus } from "./generated/IntegrationStatus";
import type { ModelProgress } from "./generated/ModelProgress";
import type { ConfigurablePermission } from "./generated/ConfigurablePermission";
import type { PermissionChange } from "./generated/PermissionChange";
import type { PermissionSetting } from "./generated/PermissionSetting";
import type { PermissionStatus } from "./generated/PermissionStatus";
import type { PlanReport } from "./generated/PlanReport";
import type { PresenceUpdate } from "./generated/PresenceUpdate";
import type { RuntimeInfo } from "./generated/RuntimeInfo";
import type { SemanticSettings } from "./generated/SemanticSettings";
import type { SemanticStatus } from "./generated/SemanticStatus";
import type { ShortcutChange } from "./generated/ShortcutChange";
import type { SystemSnapshot } from "./generated/SystemSnapshot";
import type { TrayLabels } from "./generated/TrayLabels";
import type { VoiceLevel } from "./generated/VoiceLevel";
import type { VoiceSettings } from "./generated/VoiceSettings";
import type { VoiceStatus } from "./generated/VoiceStatus";
import type { VoiceSessionStatus } from "./generated/VoiceSessionStatus";
import type { VoiceUpdate } from "./generated/VoiceUpdate";

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
  /** Confirmation window only: its assigned confirmation. */
  get_confirmation_context: { args: NoArgs; result: ConfirmationRequest };
  /** Confirmation window only: `{ confirmationId, decision }`, nothing else. */
  decide_confirmation: { args: { decision: ConfirmationDecision }; result: null };
  get_application_catalog: { args: NoArgs; result: ApplicationCatalogInfo };
  refresh_application_catalog: { args: NoArgs; result: ApplicationCatalogInfo };
  get_integration_status: { args: NoArgs; result: IntegrationStatus };
  set_tray_labels: { args: { labels: TrayLabels }; result: null };
  /** Command Center only. Invocation, never authority. */
  set_global_shortcut: { args: { accelerator: string }; result: ShortcutChange };
  summon_command_center: { args: NoArgs; result: null };
  hide_command_center: { args: NoArgs; result: null };
  dismiss_assistant: { args: NoArgs; result: null };
  preview_assistant_state: { args: { state: AssistantState | null }; result: AssistantSnapshot };
  quit_app: { args: NoArgs; result: null };
  // Voice (Command Center only). Input and output, never authorization.
  get_voice_status: { args: NoArgs; result: VoiceStatus };
  configure_voice: { args: { settings: VoiceSettings }; result: VoiceStatus };
  /** Push-to-talk: opens the microphone. */
  start_voice_capture: { args: NoArgs; result: CaptureStart };
  /** Stop listening and transcribe. */
  stop_voice_capture: { args: NoArgs; result: null };
  /** Stop listening and discard. */
  cancel_voice_capture: { args: NoArgs; result: null };
  /** Speak a reply already phrased in the user's language (output only). */
  speak_reply: { args: { text: string; language: string | null }; result: null };
  stop_speaking: { args: NoArgs; result: null };
  download_voice_model: { args: { model: string }; result: null };
  cancel_voice_model_download: { args: NoArgs; result: null };
  // Natural understanding (Command Center only). The model interprets;
  // none of these can run a tool or approve anything.
  get_semantic_status: { args: NoArgs; result: SemanticStatus };
  configure_semantic: { args: { settings: SemanticSettings }; result: SemanticStatus };
  download_semantic_model: { args: NoArgs; result: null };
  cancel_semantic_model_download: { args: NoArgs; result: null };
  // The local Agent Brain (Command Center only). It decides and proposes;
  // nothing here can run a tool or approve anything.
  get_brain_status: { args: NoArgs; result: SemanticStatus };
  configure_brain: { args: { settings: SemanticSettings }; result: SemanticStatus };
  download_brain_model: { args: NoArgs; result: null };
  cancel_brain_model_download: { args: NoArgs; result: null };
  /** Forgets the session context (not preferences). Grants nothing. */
  reset_conversation: { args: NoArgs; result: null };
  // Hands-free voice session (Gate 4.1). Explicit start; input only.
  start_voice_session: { args: NoArgs; result: CaptureStart };
  stop_voice_session: { args: NoArgs; result: null };
  get_voice_session: { args: NoArgs; result: VoiceSessionStatus | null };
  // Settings > Security (Gate 4.1). A less restrictive setting is applied
  // only after the trusted confirmation window approves it.
  get_permission_settings: { args: NoArgs; result: PermissionStatus[] };
  request_permission_change: {
    args: { permission: ConfigurablePermission; setting: PermissionSetting };
    result: PermissionChange;
  };
}

export type CommandName = keyof CommandMap;

export const COMMAND_NAMES = [
  "get_assistant_snapshot",
  "get_system_snapshot",
  "get_runtime_info",
  "list_activity",
  "submit_command",
  "get_confirmation_context",
  "decide_confirmation",
  "get_application_catalog",
  "refresh_application_catalog",
  "get_integration_status",
  "set_tray_labels",
  "set_global_shortcut",
  "summon_command_center",
  "hide_command_center",
  "dismiss_assistant",
  "preview_assistant_state",
  "quit_app",
  "get_voice_status",
  "configure_voice",
  "start_voice_capture",
  "stop_voice_capture",
  "cancel_voice_capture",
  "speak_reply",
  "stop_speaking",
  "download_voice_model",
  "cancel_voice_model_download",
  "get_semantic_status",
  "configure_semantic",
  "download_semantic_model",
  "cancel_semantic_model_download",
  "get_brain_status",
  "configure_brain",
  "download_brain_model",
  "cancel_brain_model_download",
  "reset_conversation",
  "start_voice_session",
  "stop_voice_session",
  "get_voice_session",
  "get_permission_settings",
  "request_permission_change",
] as const satisfies readonly CommandName[];

/** Events broadcast by the core to every window. */
export const EVENTS = {
  assistantState: "sershi://assistant-state",
  activity: "sershi://activity",
  /** The Command Center should focus its command input (summon/shortcut). */
  focusCommand: "sershi://focus-command",
  /**
   * Main window only: the outcome of a confirmation decided elsewhere
   * (approved, cancelled or expired on the trusted surface).
   */
  commandOutcome: "sershi://command-outcome",
  /** Companion only: whether the Command Center is on screen (presentation). */
  presence: "sershi://presence",
  /** Main window only: transcript to show, outcome of a spoken command, failures. */
  voice: "sershi://voice",
  /** Main window and companion: a bounded 0–1 audio level (never audio). */
  voiceLevel: "sershi://voice-level",
  /** Main window only: speech-model download progress. */
  voiceModel: "sershi://voice-model",
  /** Main window only: semantic-model download progress. */
  semanticModel: "sershi://semantic-model",
  /** Main window only: Agent Brain model download progress. */
  brainModel: "sershi://brain-model",
  /** Main window only: a plan's progress after each step. */
  plan: "sershi://plan",
  /** Main window and companion: the voice session's status (Gate 4.1). */
  voiceSession: "sershi://voice-session",
  /** Main window only: permission settings after a change (Gate 4.1). */
  permissions: "sershi://permissions",
} as const;

export interface EventMap {
  [EVENTS.assistantState]: AssistantSnapshot;
  [EVENTS.activity]: ActivityEntry;
  [EVENTS.focusCommand]: null;
  [EVENTS.commandOutcome]: CommandOutcome;
  [EVENTS.presence]: PresenceUpdate;
  [EVENTS.voice]: VoiceUpdate;
  [EVENTS.voiceLevel]: VoiceLevel;
  [EVENTS.voiceModel]: ModelProgress;
  [EVENTS.semanticModel]: ModelProgress;
  [EVENTS.brainModel]: ModelProgress;
  [EVENTS.plan]: PlanReport;
  [EVENTS.voiceSession]: VoiceSessionStatus;
  [EVENTS.permissions]: PermissionStatus[];
}

export type EventName = keyof EventMap;
