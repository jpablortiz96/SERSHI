import {
  EVENTS,
  isActivityEntry,
  isAssistantSnapshot,
  isCaptureStart,
  isCommandOutcome,
  isModelProgress,
  isNullPayload,
  isAppPermissionList,
  isPermissionChange,
  isPermissionList,
  isPlanReport,
  isPresenceUpdate,
  isSemanticStatus,
  isShortcutChange,
  isSystemSnapshot,
  isVoiceLevel,
  isVoiceStatus,
  isVoiceSessionStatus,
  isVoiceUpdate,
  type ActivityEntry,
  type AppPermissionStatus,
  type ConfigurablePermission,
  type PermissionSetting,
  type PermissionStatus,
  type VoiceSessionStatus,
  type AssistantSnapshot,
  type AssistantState,
  type CommandOutcome,
  type ModelProgress,
  type PlanReport,
  type PresenceUpdate,
  type SemanticSettings,
  type TrayLabels,
  type VoiceLevel,
  type VoiceSettings,
  type VoiceUpdate,
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
  /** Invocation only: registers the summon shortcut, never grants authority. */
  setGlobalShortcut: (accelerator: string) =>
    call("set_global_shortcut", { accelerator }, isShortcutChange),
  summonCommandCenter: () => call("summon_command_center", {}),
  hideCommandCenter: () => call("hide_command_center", {}),
  dismissAssistant: () => call("dismiss_assistant", {}),
  previewState: (state: AssistantState | null) =>
    call("preview_assistant_state", { state }, isAssistantSnapshot),
  quit: () => call("quit_app", {}),

  // Voice: input and output only. Nothing here can approve or grant.
  getVoiceStatus: () => call("get_voice_status", {}, isVoiceStatus),
  configureVoice: (settings: VoiceSettings) => call("configure_voice", { settings }, isVoiceStatus),
  startVoiceCapture: () => call("start_voice_capture", {}, isCaptureStart),
  stopVoiceCapture: () => call("stop_voice_capture", {}),
  cancelVoiceCapture: () => call("cancel_voice_capture", {}),
  speakReply: (text: string, language: string | null) => call("speak_reply", { text, language }),
  stopSpeaking: () => call("stop_speaking", {}),
  downloadVoiceModel: (model: string) => call("download_voice_model", { model }),
  cancelVoiceModelDownload: () => call("cancel_voice_model_download", {}),

  // Natural understanding: the local model interprets, it never acts.
  getSemanticStatus: () => call("get_semantic_status", {}, isSemanticStatus),
  configureSemantic: (settings: SemanticSettings) =>
    call("configure_semantic", { settings }, isSemanticStatus),
  downloadSemanticModel: () => call("download_semantic_model", {}),
  cancelSemanticModelDownload: () => call("cancel_semantic_model_download", {}),

  // The local Agent Brain: it decides and proposes, it never acts.
  getBrainStatus: () => call("get_brain_status", {}, isSemanticStatus),
  configureBrain: (settings: SemanticSettings) =>
    call("configure_brain", { settings }, isSemanticStatus),
  downloadBrainModel: () => call("download_brain_model", {}),
  cancelBrainModelDownload: () => call("cancel_brain_model_download", {}),
  /** Forgets the session context (not preferences). */
  resetConversation: () => call("reset_conversation", {}),

  // Hands-free voice session (Gate 4.1): input only, never authorization.
  startVoiceSession: () => call("start_voice_session", {}, isCaptureStart),
  stopVoiceSession: () => call("stop_voice_session", {}),
  getVoiceSession: () =>
    call(
      "get_voice_session",
      {},
      (v: unknown): v is VoiceSessionStatus | null => v === null || isVoiceSessionStatus(v),
    ),

  // Settings > Security: a less restrictive setting waits for the trusted
  // confirmation window; this call can never apply it by itself.
  getPermissionSettings: () => call("get_permission_settings", {}, isPermissionList),
  requestPermissionChange: (permission: ConfigurablePermission, setting: PermissionSetting) =>
    call("request_permission_change", { permission, setting }, isPermissionChange),
  getApplicationPermissions: () => call("get_application_permissions", {}, isAppPermissionList),
  requestAppPermissionChange: (appId: string, setting: PermissionSetting) =>
    call("request_app_permission_change", { appId, setting }, isPermissionChange),

  onAssistantState: (handler: (snapshot: AssistantSnapshot) => void) =>
    subscribe(EVENTS.assistantState, isAssistantSnapshot, handler),
  onActivity: (handler: (entry: ActivityEntry) => void) =>
    subscribe(EVENTS.activity, isActivityEntry, handler),
  onFocusCommand: (handler: () => void) => subscribe(EVENTS.focusCommand, isNullPayload, handler),
  /** Companion: whether the Command Center is on screen (presentation only). */
  onPresence: (handler: (update: PresenceUpdate) => void) =>
    subscribe(EVENTS.presence, isPresenceUpdate, handler),
  /** Outcomes of confirmations decided on the trusted surface (or expired). */
  onCommandOutcome: (handler: (outcome: CommandOutcome) => void) =>
    subscribe(EVENTS.commandOutcome, isCommandOutcome, handler),
  onVoice: (handler: (update: VoiceUpdate) => void) =>
    subscribe(EVENTS.voice, isVoiceUpdate, handler),
  /** A bounded 0–1 level for visuals; raw audio never reaches the UI. */
  onVoiceLevel: (handler: (level: VoiceLevel) => void) =>
    subscribe(EVENTS.voiceLevel, isVoiceLevel, handler),
  onVoiceModel: (handler: (progress: ModelProgress) => void) =>
    subscribe(EVENTS.voiceModel, isModelProgress, handler),
  onSemanticModel: (handler: (progress: ModelProgress) => void) =>
    subscribe(EVENTS.semanticModel, isModelProgress, handler),
  onBrainModel: (handler: (progress: ModelProgress) => void) =>
    subscribe(EVENTS.brainModel, isModelProgress, handler),
  /** A plan's progress after each step (display only). */
  onPlan: (handler: (report: PlanReport) => void) => subscribe(EVENTS.plan, isPlanReport, handler),
  onVoiceSession: (handler: (status: VoiceSessionStatus) => void) =>
    subscribe(EVENTS.voiceSession, isVoiceSessionStatus, handler),
  onPermissions: (handler: (settings: PermissionStatus[]) => void) =>
    subscribe(EVENTS.permissions, isPermissionList, handler),
  onAppPermissions: (handler: (apps: AppPermissionStatus[]) => void) =>
    subscribe(EVENTS.appPermissions, isAppPermissionList, handler),
};
