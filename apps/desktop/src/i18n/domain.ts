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
  Clarification,
  CommandOutcome,
  PlanReport,
  PlanStepReport,
  StepAction,
  UnderstoodAs,
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
  "voice.wake_word": "capabilities.wakeWord",
  "connected.mail_calendar": "capabilities.mailCalendar",
};

const MODEL_KEYS: Partial<Record<string, PlainKey>> = {
  "whisper-small-q8": "voice.models.whisper-small-q8",
  "whisper-large-v3-turbo-q8": "voice.models.whisper-large-v3-turbo-q8",
};

/** A speech model's display name. */
export function modelName(t: Translate, id: string): string {
  const key = MODEL_KEYS[id];
  return key ? t(key) : id;
}

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
    case "microphoneOn":
      return t("activity.events.microphoneOn");
    case "microphoneOff":
      return t("activity.events.microphoneOff");
    case "clarificationRequested":
      return t("activity.events.clarificationRequested");
    case "clarificationCancelled":
      return t("activity.events.clarificationCancelled");
    case "clarificationExpired":
      return t("activity.events.clarificationExpired");
    case "commandInterpreted":
      return t("activity.events.commandInterpreted");
    case "planStarted":
      return t("activity.events.planStarted");
    case "planFinished":
      return t("activity.events.planFinished");
    case "planCancelled":
      return t("activity.events.planCancelled");
  }
}

type Json = Record<string, unknown>;
const isObj = (v: unknown): v is Json => typeof v === "object" && v !== null;
const num = (o: Json, k: string) => (typeof o[k] === "number" ? o[k] : undefined);
const str = (o: Json, k: string) => (typeof o[k] === "string" ? o[k] : undefined);

/** Phrases a tool result from its structured data, or returns null if the shape is unknown. */
function completedReply(t: Translate, f: Formatters, outcome: CommandOutcome): string | null {
  return systemReply(t, f, outcome.toolId, outcome.data);
}

/** Phrases a system tool's structured result (memory, CPU, system). */
function systemReply(
  t: Translate,
  f: Formatters,
  toolId: string | null,
  data: unknown,
): string | null {
  if (!isObj(data)) return null;
  switch (toolId) {
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

/** Numbered options ("1. Windows PowerShell; 2. Windows PowerShell ISE"). */
function optionList(t: Translate, apps: readonly ApplicationSummary[]): string {
  return apps
    .map((app, i) => t("reply.clarify.option", { n: String(i + 1), app: applicationName(t, app) }))
    .join("; ");
}

/** A question SERSHI asks instead of guessing. */
export function clarificationReply(t: Translate, c: Clarification): string {
  const first = c.candidates[0];
  const open = c.action === "open";
  switch (c.kind) {
    case "chooseApplication":
      return t("reply.clarify.choose", { options: optionList(t, c.candidates) });
    case "didYouMean":
      if (!first) return t(open ? "reply.clarify.whichOpen" : "reply.clarify.whichClose");
      return t(open ? "reply.clarify.didYouMeanOpen" : "reply.clarify.didYouMeanClose", {
        app: applicationName(t, first),
      });
    case "whichApplication":
      return t(open ? "reply.clarify.whichOpen" : "reply.clarify.whichClose");
    case "multipleTargets":
      return t(open ? "reply.clarify.multipleOpen" : "reply.clarify.multipleClose", {
        options: optionList(t, c.candidates),
      });
  }
}

/** "Open Microsoft Word": what SERSHI understood a repaired request as. */
export function understoodAction(t: Translate, understood: UnderstoodAs): string {
  const app = applicationName(t, understood.application);
  return understood.action === "open"
    ? t("reply.understood.open", { app })
    : t("reply.understood.close", { app });
}

/** What one plan step does, as a short label ("Abrir Google Chrome"). */
export function stepLabel(
  t: Translate,
  step: Pick<PlanStepReport, "action" | "application">,
): string {
  const app = step.application ? applicationName(t, step.application) : "";
  const key: Record<StepAction, string> = {
    open: "plan.step.open",
    close: "plan.step.close",
    memory: "plan.step.memory",
    cpu: "plan.step.cpu",
    systemInfo: "plan.step.systemInfo",
  };
  return t(key[step.action] as "plan.step.open", { app });
}

/**
 * A plan's result, phrased from each step's trusted data: what happened,
 * never what a model said would happen ("Abrí Chrome, pero no pude abrir
 * Outlook. Estás usando 19,8 GB de 31,7 GB.").
 */
export function planReply(t: Translate, f: Formatters, plan: PlanReport): string {
  const sentences: string[] = [];
  for (const step of plan.steps) {
    const app = step.application ? applicationName(t, step.application) : "";
    switch (step.status) {
      case "completed": {
        if (step.action === "open") sentences.push(t("plan.done.open", { app }));
        else if (step.action === "close") sentences.push(t("plan.done.close", { app }));
        else {
          const tool = {
            memory: "system.get_memory",
            cpu: "system.get_cpu",
            systemInfo: "system.get_info",
          }[step.action];
          const phrased = systemReply(t, f, tool, step.data);
          if (phrased) sentences.push(phrased);
        }
        break;
      }
      case "needsConfirmation":
        sentences.push(t("plan.waiting", { step: stepLabel(t, step) }));
        break;
      case "failed":
      case "unresolved":
        sentences.push(t("plan.failed", { step: stepLabel(t, step) }));
        break;
      case "skipped":
        sentences.push(t("plan.skipped", { step: stepLabel(t, step) }));
        break;
      case "cancelled":
        if (!sentences.includes(t("plan.cancelled"))) sentences.push(t("plan.cancelled"));
        break;
      case "pending":
      case "running":
        break;
    }
  }
  return sentences.join(" ");
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
  // A plan (finished, waiting for a step's approval, or cancelled) is
  // phrased from its steps.
  if (outcome.plan) return planReply(t, f, outcome.plan) || outcome.reply;
  // The Agent Brain's own words, in the interface language.
  if (detail?.kind === "brainAnswer" || detail?.kind === "brainQuestion") return detail.message;
  if (detail?.kind === "planTooLong") return t("plan.tooLong", { max: f.integer(detail.maxSteps) });
  switch (outcome.status) {
    case "partial":
      return outcome.reply;
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
    case "needsClarification":
      return detail?.kind === "clarification"
        ? clarificationReply(t, detail.clarification)
        : outcome.reply;
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
