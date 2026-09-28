/**
 * Maps core identifiers (tool ids, capability ids, states, activity kinds,
 * command outcomes) to localized copy. Identifiers themselves are never
 * translated; unknown ones fall back to the text the core supplied.
 */
import type {
  ActivityEntry,
  ApplicationResult,
  ApplicationSummary,
  AssistantState,
  CapabilityStatus,
  CommandOutcome,
  Platform,
  PlatformCapability,
  RiskLevel,
} from "@sershi/contracts";

import type { Formatters } from "./format";
import type { PlainKey, Translate } from "./types";

const TOOL_KEYS: Partial<Record<string, PlainKey>> = {
  "system.get_info": "tools.systemInfo",
  "system.get_memory": "tools.memory",
  "system.get_cpu": "tools.cpu",
  "system.open_application": "tools.openApplication",
  "system.close_application": "tools.closeApplication",
};

/** Windows components have stable ids and localized names. */
const BUILTIN_APP_KEYS: Partial<Record<string, PlainKey>> = {
  "windows.calculator": "apps.calculator",
  "windows.notepad": "apps.notepad",
  "windows.explorer": "apps.explorer",
  "windows.settings": "apps.settings",
};

/** An application's name: localized for Windows components, otherwise as Windows shows it. */
export function applicationName(t: Translate, app: Pick<ApplicationSummary, "id" | "displayName">) {
  const key = BUILTIN_APP_KEYS[app.id];
  return key ? t(key) : app.displayName;
}

const CAPABILITY_KEYS: Partial<Record<string, PlainKey>> = {
  "system.telemetry": "capabilities.telemetry",
  "companion.overlay": "capabilities.companionOverlay",
  "apps.launch": "capabilities.appsLaunch",
  "system.battery": "capabilities.battery",
  "shell.shortcut": "capabilities.shortcut",
  "ai.provider": "capabilities.aiProvider",
  "context.files": "capabilities.contextFiles",
  voice: "capabilities.voice",
  "connected.mail_calendar": "capabilities.mailCalendar",
};

export function toolName(t: Translate, toolId: string | null, fallback?: string): string {
  const key = toolId ? TOOL_KEYS[toolId] : undefined;
  return key ? t(key) : (fallback ?? toolId ?? "");
}

export function capabilityLabel(t: Translate, id: string, fallback?: string): string {
  const key = CAPABILITY_KEYS[id];
  return key ? t(key) : (fallback ?? id);
}

export function capabilityLabelFor(t: Translate, capability: PlatformCapability): string {
  return capabilityLabel(t, capability.id, capability.label);
}

export function stateLabel(t: Translate, state: AssistantState): string {
  return t(`state.${state}.label`);
}

export function stateLine(t: Translate, state: AssistantState): string {
  return t(`state.${state}.line`);
}

export function capabilityStatus(t: Translate, status: CapabilityStatus): string {
  return t(`settings.capabilityStatus.${status}`);
}

export function riskLabel(t: Translate, risk: RiskLevel): string {
  return t(`settings.risk.${risk}`);
}

export function platformName(t: Translate, platform: Platform): string {
  return t(`platforms.${platform}`);
}

export function describeActivity(t: Translate, entry: ActivityEntry): string {
  const event = describeEvent(t, entry);
  // `subject` is a trusted, resolved name (never raw input).
  const subject = entry.subject;
  if (!subject) return event;
  if (entry.kind === "toolCompleted" && entry.toolId === "system.open_application") {
    return t("activity.events.appOpened", { app: subject });
  }
  if (entry.kind === "toolCompleted" && entry.toolId === "system.close_application") {
    return t("activity.events.appCloseRequested", { app: subject });
  }
  return t("activity.withSubject", { event, subject });
}

function describeEvent(t: Translate, entry: ActivityEntry): string {
  const tool = toolName(t, entry.toolId);
  switch (entry.kind) {
    case "systemReady":
      return t("activity.events.systemReady");
    case "commandReceived":
      return t("activity.events.commandReceived");
    case "toolRequested":
      return t("activity.events.toolRequested", { tool });
    case "toolCompleted":
      return t("activity.events.toolCompleted", { tool });
    case "toolFailed":
      return t("activity.events.toolFailed", { tool });
    case "toolDenied":
      return t("activity.events.toolDenied", { tool });
    case "confirmationRequired":
      return t("activity.events.confirmationRequired", { tool });
    case "capabilityUnavailable":
      return t("activity.events.capabilityUnavailable");
    case "toolDeclined":
      return t("activity.events.toolDeclined", { tool });
    case "confirmationApproved":
      return t("activity.events.confirmationApproved", { tool });
    case "confirmationCancelled":
      return t("activity.events.confirmationCancelled", { tool });
    case "confirmationExpired":
      return t("activity.events.confirmationExpired", { tool });
  }
}

type Json = Record<string, unknown>;
const isObj = (v: unknown): v is Json => typeof v === "object" && v !== null;
const num = (o: Json, k: string) => (typeof o[k] === "number" ? o[k] : undefined);
const str = (o: Json, k: string) => (typeof o[k] === "string" ? o[k] : undefined);

/** Phrases a tool result from its structured data, or returns null if the shape is unknown. */
function completedReply(t: Translate, f: Formatters, outcome: CommandOutcome): string | null {
  const data = outcome.data;
  if (!isObj(data)) return null;
  switch (outcome.toolId) {
    case "system.get_memory": {
      const total = num(data, "totalBytes");
      const used = num(data, "usedBytes");
      if (total === undefined || used === undefined || total <= 0) return null;
      return t("reply.memory", {
        used: f.gigabytes(used),
        total: f.gigabytes(total),
        percent: f.percent((used / total) * 100),
      });
    }
    case "system.get_cpu": {
      const cores = num(data, "logicalCores");
      if (cores === undefined) return null;
      const usage = num(data, "usagePercent");
      return usage === undefined
        ? t("reply.cpuWarmingUp", { cores: f.integer(cores) })
        : t("reply.cpu", { percent: f.percent(usage), cores: f.integer(cores) });
    }
    case "system.get_info": {
      const os = isObj(data.os) ? data.os : undefined;
      const cpu = isObj(data.cpu) ? data.cpu : undefined;
      const memory = isObj(data.memory) ? data.memory : undefined;
      const name = os && str(os, "name");
      const arch = os && str(os, "arch");
      const brand = cpu && str(cpu, "brand");
      const cores = cpu && num(cpu, "logicalCores");
      const total = memory && num(memory, "totalBytes");
      if (!name || !arch || !brand || cores === undefined || total === undefined) return null;
      return t("reply.systemInfo", {
        os: name,
        arch,
        cpu: brand,
        cores: f.integer(cores),
        memory: f.gigabytes(total),
      });
    }
    default:
      return null;
  }
}

const APPLICATION_RESULT_KINDS = [
  "opened",
  "notFound",
  "ambiguous",
  "launchFailed",
  "closeRequested",
  "notRunning",
  "closeUnsupported",
  "catalogUnavailable",
] as const satisfies readonly ApplicationResult["kind"][];

/** Narrows tool output data to an application result. */
export function asApplicationResult(data: unknown): ApplicationResult | null {
  if (!isObj(data) || typeof data.kind !== "string") return null;
  return (APPLICATION_RESULT_KINDS as readonly string[]).includes(data.kind)
    ? (data as ApplicationResult)
    : null;
}

function applicationReply(t: Translate, result: ApplicationResult): string {
  switch (result.kind) {
    case "opened":
      return t("reply.apps.opened", { app: applicationName(t, result.application) });
    case "notFound":
      return t("reply.apps.notFound", { app: result.query });
    case "ambiguous":
      return t("reply.apps.ambiguous", { query: result.query });
    case "launchFailed":
      return t(`reply.apps.launchFailed.${result.reason}`, {
        app: applicationName(t, result.application),
      });
    case "closeRequested":
      return t("reply.apps.closeRequested", { app: applicationName(t, result.application) });
    case "notRunning":
      return t("reply.apps.notRunning", { app: applicationName(t, result.application) });
    case "closeUnsupported":
      return t("reply.apps.closeUnsupported", { app: applicationName(t, result.application) });
    case "catalogUnavailable":
      return t("reply.apps.catalogUnavailable");
  }
}

/**
 * Renders a command outcome in the interface language. Falls back to the
 * core's canonical English `reply` for anything it cannot phrase (e.g. a tool
 * added without translations).
 */
export function composeReply(t: Translate, f: Formatters, outcome: CommandOutcome): string {
  const tool = toolName(t, outcome.toolId);
  const detail = outcome.detail;
  const app = asApplicationResult(outcome.data);
  switch (outcome.status) {
    case "completed":
      if (app) return applicationReply(t, app);
      return completedReply(t, f, outcome) ?? outcome.reply;
    case "unresolved":
      return app ? applicationReply(t, app) : outcome.reply;
    case "cancelled":
      return t("reply.cancelled");
    case "expired":
      return t("reply.expired");
    case "answered":
      return detail?.kind === "answer" ? t(`reply.answer.${detail.topic}`) : outcome.reply;
    case "needsConfirmation":
      return t("reply.needsConfirmation", { tool });
    case "denied":
      return detail?.kind === "denied"
        ? t(`reply.denied.${detail.reason.kind}`, { tool })
        : outcome.reply;
    case "unavailable":
      return detail?.kind === "unavailable"
        ? t("reply.unavailable", {
            capability: capabilityLabel(t, detail.capability),
            milestone: detail.milestone,
          })
        : outcome.reply;
    case "notUnderstood":
      return t("reply.notUnderstood");
    case "failed":
      if (app) return applicationReply(t, app);
      return t("reply.failed", { tool });
    case "rejected":
      if (detail?.kind !== "rejected") return outcome.reply;
      return detail.reason === "tooLong"
        ? t("reply.rejected.tooLong", { max: f.integer(detail.maxChars) })
        : t(`reply.rejected.${detail.reason}`);
  }
}
