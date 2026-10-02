/**
 * Gate 4.1 in the Command Center: the voice session (indicator, turn-taking
 * replies, farewells), answers grounded in the action ledger, conversation
 * scrollback, the compact "New conversation", Settings › Security, and the
 * trusted window's permission-change copy. Nothing here can approve.
 */
import type {
  AppPermissionStatus,
  CommandOutcome,
  ConfirmationChoice,
  ConfirmationRequest,
  PermissionStatus,
  RecallItem,
  VoiceSessionStatus,
} from "@sershi/contracts";
import {
  isCommandOutcome,
  isPermissionList,
  isVoiceSessionStatus,
  isVoiceUpdate,
} from "@sershi/contracts";
import fixture from "@sershi/contracts/fixtures/confirmation-request.json";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const speakReply = vi.fn<(text: string, language: string | null) => Promise<null>>(() =>
  Promise.resolve(null),
);
const startVoiceSession = vi.fn(() => Promise.resolve({ kind: "started" as const }));
const stopVoiceSession = vi.fn(() => Promise.resolve(null));
const resetConversation = vi.fn(() => Promise.resolve(null));
const requestPermissionChange = vi.fn<
  (permission: string, setting: string) => Promise<"needsConfirmation">
>(() => Promise.resolve("needsConfirmation"));
let permissionHandler: ((settings: PermissionStatus[]) => void) | null = null;
let appHandler: ((apps: AppPermissionStatus[]) => void) | null = null;
const requestAppPermissionChange = vi.fn<
  (appId: string, setting: string) => Promise<"needsConfirmation">
>(() => Promise.resolve("needsConfirmation"));
const appDefaults: AppPermissionStatus[] = [
  {
    appId: "windows.calculator",
    displayName: "Calculator",
    setting: null,
    closeRisk: "safeToClose",
  },
  { appId: "outlook", displayName: "Outlook", setting: null, closeRisk: "unknown" },
];

const defaults: PermissionStatus[] = [
  { permission: "openApplications", setting: "alwaysAllow", defaultSetting: "alwaysAllow" },
  { permission: "closeApplications", setting: "askEveryTime", defaultSetting: "askEveryTime" },
  { permission: "systemInformation", setting: "alwaysAllow", defaultSetting: "alwaysAllow" },
];

vi.mock("../src/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../src/ipc")>();
  return {
    ...actual,
    desktopRuntime: true,
    sershi: {
      ...actual.sershi,
      speakReply,
      startVoiceSession,
      stopVoiceSession,
      resetConversation,
      requestPermissionChange,
      stopSpeaking: () => Promise.resolve(null),
      getPermissionSettings: () => Promise.resolve(defaults),
      getApplicationPermissions: () => Promise.resolve(appDefaults),
      requestAppPermissionChange,
      onAppPermissions: (handler: (apps: AppPermissionStatus[]) => void) => {
        appHandler = handler;
        return () => {
          appHandler = null;
        };
      },
      onPermissions: (handler: (settings: PermissionStatus[]) => void) => {
        permissionHandler = handler;
        return () => {
          permissionHandler = null;
        };
      },
    },
  };
});

const getContext = vi.fn<() => Promise<ConfirmationRequest>>();
const decide = vi.fn<(id: string, choice: ConfirmationChoice) => Promise<null>>();
vi.mock("../src/ipc/confirmation", () => ({
  desktopRuntime: true,
  confirmationSurface: { getContext, decide },
}));

const { Transcript } = await import("../src/components/conversation/Transcript");
const { CommandBar } = await import("../src/components/command/CommandBar");
const { useConversation, MAX_MESSAGES } = await import("../src/state/conversation");
const { useVoice, handleVoiceUpdate, handleVoiceSession } = await import("../src/state/voice");
const { usePermissions } = await import("../src/state/permissions");
const { SecuritySettings } = await import("../src/surfaces/command-center/SecuritySettings");
const { HomeView } = await import("../src/surfaces/command-center/HomeView");
const { ConfirmationSurface } = await import("../src/surfaces/confirmation/ConfirmationSurface");
const { recallReply, batchReply, awaitingReply } = await import("../src/i18n/domain");
const { createTranslator } = await import("../src/i18n/translate");
const { useLocaleStore } = await import("../src/i18n");
const { DEFAULT_PREFERENCES } = await import("../src/i18n/preferences");
const { useAssistantStore } = await import("../src/state/assistant");

const excel = { id: "excel", displayName: "Excel", source: "startMenu" as const };
const chrome = { id: "google-chrome", displayName: "Google Chrome", source: "startMenu" as const };

function outcome(overrides: Partial<CommandOutcome> = {}): CommandOutcome {
  return {
    status: "completed",
    reply: "Opened Excel.",
    detail: null,
    toolId: "system.open_application",
    data: { kind: "opened", matched: "exact", application: excel },
    durationMs: 20,
    understood: null,
    understanding: null,
    plan: null,
    brain: null,
    session: null,
    ...overrides,
  };
}

function session(overrides: Partial<VoiceSessionStatus> = {}): VoiceSessionStatus {
  return {
    id: 1,
    phase: "listening",
    turn: 0,
    bargeIns: 0,
    idleTimeoutMs: 25_000,
    ended: null,
    ...overrides,
  };
}

beforeEach(() => {
  useConversation.setState({ messages: [], pending: false, livePlan: null });
  useVoice.setState({
    prefs: { ...DEFAULT_PREFERENCES, voiceResponses: true },
    session: null,
    lastSession: null,
  });
  useLocaleStore.setState({ preference: "auto", systemLocale: "es-419", locale: "es-419" });
  speakReply.mockClear();
  startVoiceSession.mockClear();
  stopVoiceSession.mockClear();
  requestPermissionChange.mockClear();
});

describe("voice session", () => {
  it("is never hidden: the indicator shows the phase and how to finish", () => {
    useVoice.setState({ status: null });
    render(<CommandBar />);
    expect(screen.queryByText("Sesión de voz")).toBeNull();
    act(() => {
      handleVoiceSession(session());
    });
    expect(screen.getByText("Sesión de voz")).toBeTruthy();
    expect(screen.getByText("Escuchando")).toBeTruthy();
    expect(screen.getByText(/Di “No, gracias” cuando termines/)).toBeTruthy();
    act(() => {
      handleVoiceSession(session({ phase: "waitingForConfirmation" }));
    });
    expect(screen.getByText("Aprueba en la ventana de confirmación")).toBeTruthy();
    // Both the End button and the lit wave button end it.
    const enders = screen.getAllByRole("button", { name: "Terminar la sesión de voz" });
    expect(enders).toHaveLength(2);
    fireEvent.click(screen.getByText("Terminar"));
    expect(stopVoiceSession).toHaveBeenCalledTimes(1);
  });

  it("starts only from the explicit button", async () => {
    useVoice.setState({
      status: {
        supported: true,
        microphone: { access: "allowed", devices: [], fallback: false },
        models: [],
        profile: "accurate",
        model: "whisper-large-v3-turbo-q8",
        acceleration: "vulkan",
        accelerator: null,
        voices: [],
        capturing: false,
        speaking: false,
      },
    });
    render(<CommandBar />);
    expect(startVoiceSession).not.toHaveBeenCalled();
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Iniciar una sesión de voz" }));
      await Promise.resolve();
    });
    expect(startVoiceSession).toHaveBeenCalledTimes(1);
  });

  it("says it is listening once, then follows each reply with the next turn", () => {
    handleVoiceSession(session());
    handleVoiceSession(session({ phase: "transcribing" }));
    const notices = useConversation
      .getState()
      .messages.filter((m) => m.role === "sershi" && m.reply.kind === "session");
    expect(notices).toHaveLength(1);
  });

  it("asks “anything else?” only when the core says so", () => {
    handleVoiceUpdate({ kind: "answered", outcome: outcome(), speak: true, anythingElse: true });
    expect(speakReply).toHaveBeenCalledWith("Abrí Excel. ¿Necesitas algo más?", "es-419");
    speakReply.mockClear();
    handleVoiceUpdate({ kind: "answered", outcome: outcome(), speak: true, anythingElse: false });
    expect(speakReply).toHaveBeenCalledWith("Abrí Excel.", "es-419");
  });

  it("a reply the user talked over is shown but never spoken", () => {
    handleVoiceUpdate({ kind: "answered", outcome: outcome(), speak: false, anythingElse: false });
    expect(speakReply).not.toHaveBeenCalled();
    expect(useConversation.getState().messages).toHaveLength(1);
  });

  it("“sí” to “anything else?” only answers that it is listening", () => {
    handleVoiceUpdate({ kind: "prompt" });
    expect(speakReply).toHaveBeenCalledWith("Te escucho.", "es-419");
  });

  it("a farewell ends the session with a goodbye; silence ends it quietly", () => {
    handleVoiceSession(session());
    handleVoiceSession(session({ phase: null, ended: "farewell" }));
    expect(useVoice.getState().session).toBeNull();
    expect(speakReply).toHaveBeenCalledWith("Hasta luego.", "es-419");
    speakReply.mockClear();
    handleVoiceSession(session({ id: 2 }));
    handleVoiceSession(session({ id: 2, phase: null, ended: "timeout" }));
    expect(speakReply).not.toHaveBeenCalled();
    const last = useConversation.getState().messages.at(-1);
    expect(last?.role === "sershi" && last.reply).toEqual({ kind: "session", notice: "timeout" });
  });
});

describe("answers grounded in what SERSHI actually did", () => {
  const item = (app: typeof excel, result: RecallItem["result"]): RecallItem => ({
    action: "open",
    application: app,
    result,
  });

  it("names what was opened, and never a failed open as opened", () => {
    const es = createTranslator("es-419");
    expect(recallReply(es, "opened", [item(excel, "succeeded")])).toBe("Abrí Excel.");
    expect(recallReply(es, "opened", [item(chrome, "succeeded"), item(excel, "failed")])).toBe(
      "Abrí Google Chrome. No pude abrir Excel.",
    );
    expect(recallReply(es, "opened", [])).toBe(
      "No he abierto ninguna aplicación en esta conversación.",
    );
    const en = createTranslator("en-US");
    expect(recallReply(en, "opened", [item(excel, "failed")])).toBe("I couldn't open Excel.");
    const pt = createTranslator("pt-BR");
    expect(recallReply(pt, "did", [item(excel, "succeeded")])).toBe("Agora há pouco: abrir excel.");
  });

  it("guards accept ledger items with trusted applications only", () => {
    const recall = (items: unknown) =>
      outcome({
        status: "answered",
        toolId: null,
        data: null,
        detail: { kind: "recall", recall: "opened", items } as CommandOutcome["detail"],
      });
    expect(isCommandOutcome(recall([item(excel, "succeeded")]))).toBe(true);
    expect(
      isCommandOutcome(
        recall([{ action: "open", application: { path: "C:\\x.exe" }, result: "succeeded" }]),
      ),
    ).toBe(false);
    expect(isCommandOutcome(recall(Array.from({ length: 40 }, () => item(excel, "failed"))))).toBe(
      false,
    );
  });
});

describe("contracts", () => {
  it("voice updates must say whether to speak", () => {
    expect(isVoiceUpdate({ kind: "answered", outcome: outcome() })).toBe(false);
    expect(
      isVoiceUpdate({ kind: "answered", outcome: outcome(), speak: true, anythingElse: false }),
    ).toBe(true);
    expect(isVoiceUpdate({ kind: "prompt" })).toBe(true);
  });

  it("a session status has a known phase", () => {
    expect(isVoiceSessionStatus(session())).toBe(true);
    expect(isVoiceSessionStatus(session({ phase: "recording" as never }))).toBe(false);
  });

  it("only the closed list of permissions and settings exists", () => {
    expect(isPermissionList(defaults)).toBe(true);
    for (const bad of [
      [{ permission: "allowAll", setting: "alwaysAllow", defaultSetting: "askEveryTime" }],
      [{ permission: "files.delete", setting: "alwaysAllow", defaultSetting: "askEveryTime" }],
      [{ permission: "closeApplications", setting: "granted", defaultSetting: "askEveryTime" }],
    ]) {
      expect(isPermissionList(bad)).toBe(false);
    }
  });
});

describe("conversation scrollback", () => {
  const scrollIntoView = vi.fn();
  beforeEach(() => {
    scrollIntoView.mockClear();
    Element.prototype.scrollIntoView = scrollIntoView;
  });
  afterEach(() => {
    // jsdom has no layout; restore a no-op.
    Element.prototype.scrollIntoView = () => undefined;
  });

  const turns = (n: number) =>
    Array.from({ length: n }, (_, i) => ({
      id: i + 1,
      role: "user" as const,
      text: `turn ${i + 1}`,
      via: "typed" as const,
    }));

  /** Pretends the user scrolled `fromEnd` px above the bottom. */
  function scrollTo(region: HTMLElement, fromEnd: number) {
    Object.defineProperty(region, "scrollHeight", { configurable: true, value: 2000 });
    Object.defineProperty(region, "clientHeight", { configurable: true, value: 400 });
    region.scrollTop = 1600 - fromEnd;
    fireEvent.scroll(region);
  }

  it("keeps every turn of the session, scrollable by keyboard", () => {
    const { rerender } = render(<Transcript messages={turns(60)} />);
    const region = screen.getByRole("region", { name: "Conversación" });
    expect(region.tabIndex).toBe(0);
    expect(screen.getByText("turn 1")).toBeTruthy();
    rerender(<Transcript messages={turns(61)} />);
    expect(screen.getByText("turn 1")).toBeTruthy();
    expect(MAX_MESSAGES).toBeGreaterThanOrEqual(100);
  });

  it("never snaps a reader back to the bottom; offers Jump to latest", () => {
    const { rerender } = render(<Transcript messages={turns(10)} />);
    const region = screen.getByRole("region", { name: "Conversación" });
    scrollTo(region, 600);
    scrollIntoView.mockClear();
    rerender(<Transcript messages={turns(11)} />);
    expect(scrollIntoView).not.toHaveBeenCalled();
    const jump = screen.getByRole("button", { name: /Ir a lo último/ });
    fireEvent.click(jump);
    expect(scrollIntoView).toHaveBeenCalled();
    scrollTo(region, 0);
    expect(screen.queryByRole("button", { name: /Ir a lo último/ })).toBeNull();
  });

  it("follows new turns while the user is at the latest", () => {
    const { rerender } = render(<Transcript messages={turns(3)} />);
    scrollIntoView.mockClear();
    rerender(<Transcript messages={turns(4)} />);
    expect(scrollIntoView).toHaveBeenCalled();
  });
});

describe("new conversation", () => {
  it("is a compact control that clears the conversation and the core's session", () => {
    useAssistantStore.setState({ snapshot: { state: "idle", previewState: null, revision: 1 } });
    useConversation.setState({
      messages: [{ id: 1, role: "user", text: "Abre Excel", via: "voice" }],
    });
    render(<HomeView onViewActivity={() => undefined} />);
    const button = screen.getByRole("button", { name: "Nueva conversación" });
    expect(button.querySelector("svg")).toBeTruthy();
    fireEvent.click(button);
    expect(resetConversation).toHaveBeenCalled();
    expect(useConversation.getState().messages).toHaveLength(0);
  });
});

describe("Settings › Security", () => {
  it("shows the Gate 1A defaults and asks the trusted window to loosen one", async () => {
    usePermissions.setState({ settings: null, pending: null, notice: null });
    render(<SecuritySettings />);
    await act(async () => {
      await Promise.resolve();
    });
    const close = screen.getByRole("radiogroup", { name: "Cerrar aplicaciones" });
    const ask = close.querySelector<HTMLInputElement>('input[value="askEveryTime"]');
    const always = close.querySelector<HTMLInputElement>('input[value="alwaysAllow"]');
    expect(ask?.checked).toBe(true);
    expect(close.textContent).toContain("Preguntar cada vez · Recomendado");

    await act(async () => {
      if (always) fireEvent.click(always);
      await Promise.resolve();
    });
    expect(requestPermissionChange).toHaveBeenCalledWith("closeApplications", "alwaysAllow");
    // Not applied here: it waits for the confirmation window.
    expect(screen.getByText("Aprueba este cambio en la ventana de confirmación.")).toBeTruthy();
    expect(close.querySelector<HTMLInputElement>('input[value="askEveryTime"]')?.checked).toBe(
      true,
    );

    // The trusted window approved: the core publishes the new settings.
    act(() => {
      permissionHandler?.(
        defaults.map((d) =>
          d.permission === "closeApplications" ? { ...d, setting: "alwaysAllow" } : d,
        ),
      );
    });
    expect(close.querySelector<HTMLInputElement>('input[value="alwaysAllow"]')?.checked).toBe(true);
  });
});

describe("the trusted window explains a permission change", () => {
  it("names the permission and that Agent Brain proposals still ask", async () => {
    useLocaleStore.setState({ preference: "auto", systemLocale: "en-US", locale: "en-US" });
    getContext.mockResolvedValue({
      ...(fixture as ConfirmationRequest),
      toolId: "settings.permissions",
      action: "changePermission",
      subject: { kind: "permission", permission: "closeApplications", setting: "alwaysAllow" },
      reason: "permissionChange",
      expiresAtMs: Date.now() + 60_000,
    });
    render(<ConfirmationSurface />);
    await act(async () => {
      await Promise.resolve();
    });
    expect(screen.getByText("Allow “Close applications” without asking?")).toBeTruthy();
    expect(screen.getByText("Actions proposed by the Agent Brain will still ask.")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Always allow" })).toBeTruthy();
  });
});

// ── Gate 4.1.1: grouped closes, per-application settings, cold start ──

describe("closing several applications", () => {
  const calc = { id: "windows.calculator", displayName: "Calculator", source: "builtIn" as const };
  const outlook = { id: "outlook", displayName: "Outlook", source: "startMenu" as const };

  it("says what closed, what waits and never claims a close before it happened", () => {
    const es = createTranslator("es-419");
    expect(
      batchReply(es, [
        { application: calc, status: "completed" },
        { application: outlook, status: "needsConfirmation" },
      ]),
    ).toBe(
      "Cerré Calculadora. Outlook está esperando tu aprobación para cerrarse. Necesito tu aprobación en la ventana segura.",
    );
    expect(
      batchReply(es, [
        { application: chrome, status: "cancelled" },
        { application: outlook, status: "cancelled" },
      ]),
    ).toBe("No cerré Google Chrome y Outlook.");
    expect(batchReply(es, [{ application: outlook, status: "failed" }])).toBe(
      "No pude cerrar Outlook.",
    );
    expect(awaitingReply(es, [chrome, outlook])).toBe(
      "Google Chrome y Outlook esperan tu aprobación para cerrarse. Necesito tu aprobación en la ventana segura.",
    );
  });

  it("phrases a single close only after it happened", () => {
    const awaiting = outcome({
      status: "needsConfirmation",
      data: null,
      detail: { kind: "awaitingApproval", applications: [outlook] },
    });
    handleVoiceUpdate({ kind: "answered", outcome: awaiting, speak: true, anythingElse: false });
    expect(speakReply).toHaveBeenCalledWith(
      "Outlook está esperando tu aprobación para cerrarse. Necesito tu aprobación en la ventana segura.",
      "es-419",
    );
    speakReply.mockClear();
    const closed = outcome({
      toolId: "system.close_application",
      data: { kind: "closeRequested", application: outlook, windows: 1 },
    });
    handleVoiceUpdate({ kind: "answered", outcome: closed, speak: true, anythingElse: false });
    expect(speakReply).toHaveBeenCalledWith("Cerré Outlook.", "es-419");
  });

  it("an unshown confirmation is reported, never left waiting", () => {
    const failed = outcome({
      status: "failed",
      toolId: null,
      data: null,
      detail: { kind: "confirmationUnavailable" },
    });
    handleVoiceUpdate({ kind: "answered", outcome: failed, speak: true, anythingElse: false });
    expect(speakReply).toHaveBeenCalledWith(
      "No pude mostrar la ventana de confirmación, así que no hice nada.",
      "es-419",
    );
  });

  it("guards accept grouped closes of trusted applications only, bounded", () => {
    const batch = (steps: unknown) =>
      outcome({
        status: "needsConfirmation",
        data: null,
        detail: { kind: "closeBatch", steps } as CommandOutcome["detail"],
      });
    expect(isCommandOutcome(batch([{ application: outlook, status: "needsConfirmation" }]))).toBe(
      true,
    );
    expect(
      isCommandOutcome(batch([{ application: { path: "C:\\x.exe" }, status: "completed" }])),
    ).toBe(false);
    expect(
      isCommandOutcome(
        batch(Array.from({ length: 6 }, () => ({ application: outlook, status: "completed" }))),
      ),
    ).toBe(false);
  });

  it("the trusted window lists exactly the applications of a group", async () => {
    useLocaleStore.setState({ preference: "auto", systemLocale: "en-US", locale: "en-US" });
    getContext.mockResolvedValue({
      ...(fixture as ConfirmationRequest),
      action: "closeApplications",
      subject: { kind: "applications", applications: [chrome, outlook] },
      expiresAtMs: Date.now() + 60_000,
    });
    render(<ConfirmationSurface />);
    await act(async () => {
      await Promise.resolve();
    });
    expect(screen.getByText("Close 2 applications?")).toBeTruthy();
    const list = screen.getByRole("list", { name: "Close 2 applications?" });
    expect([...list.querySelectorAll("li")].map((li) => li.textContent)).toEqual([
      "Google Chrome",
      "Outlook",
    ]);
    expect(screen.getByRole("button", { name: "Close them" })).toBeTruthy();
  });
});

describe("per-application close settings", () => {
  it("shows the safe Calculator and asks the trusted window for Always allow", async () => {
    usePermissions.setState({ settings: null, apps: [], pending: null, notice: null });
    render(<SecuritySettings />);
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(screen.getByText(/Segura de cerrar/)).toBeTruthy();
    const group = screen.getByRole("radiogroup", { name: "Outlook" });
    const always = group.querySelector<HTMLInputElement>('input[value="alwaysAllow"]');
    await act(async () => {
      if (always) fireEvent.click(always);
      await Promise.resolve();
    });
    expect(requestAppPermissionChange).toHaveBeenCalledWith("outlook", "alwaysAllow");
    expect(screen.getByText("Aprueba este cambio en la ventana de confirmación.")).toBeTruthy();
    act(() => {
      appHandler?.(
        appDefaults.map((a) => (a.appId === "outlook" ? { ...a, setting: "alwaysAllow" } : a)),
      );
    });
    expect(group.querySelector<HTMLInputElement>('input[value="alwaysAllow"]')?.checked).toBe(true);
  });

  it("the window explains one application's setting", async () => {
    useLocaleStore.setState({ preference: "auto", systemLocale: "en-US", locale: "en-US" });
    getContext.mockResolvedValue({
      ...(fixture as ConfirmationRequest),
      toolId: "settings.permissions",
      action: "changePermission",
      subject: {
        kind: "applicationPermission",
        appId: "outlook",
        displayName: "Outlook",
        setting: "alwaysAllow",
      },
      reason: "permissionChange",
      expiresAtMs: Date.now() + 60_000,
    });
    render(<ConfirmationSurface />);
    await act(async () => {
      await Promise.resolve();
    });
    expect(screen.getByText("Close “Outlook” without asking?")).toBeTruthy();
  });
});

describe("voice session cold start", () => {
  it("says it is preparing voice instead of pretending to transcribe", () => {
    useAssistantStore.setState({
      snapshot: { state: "transcribing", previewState: null, revision: 2 },
    });
    render(<CommandBar />);
    act(() => {
      handleVoiceUpdate({ kind: "preparing" });
    });
    expect(screen.getByPlaceholderText("Preparando voz…")).toBeTruthy();
    act(() => {
      handleVoiceSession(session({ phase: "preparing" }));
    });
    expect(screen.getByText("Preparando voz")).toBeTruthy();
    act(() => {
      handleVoiceUpdate({ kind: "heard", text: "Abre Excel", language: "es", firstHeard: null });
    });
    expect(useVoice.getState().preparing).toBe(false);
  });
});
