/**
 * Prompt 4 in the Command Center: plans phrased from trusted data, live
 * progress, the Agent Brain's answers and questions, a new conversation,
 * and the brain's settings — none of which can run or approve anything.
 */
import type { CommandOutcome, PlanReport, PlanStepReport, SemanticStatus } from "@sershi/contracts";
import { isCommandOutcome, isPlanReport } from "@sershi/contracts";
import planFixture from "@sershi/contracts/fixtures/command-outcome-plan.json";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const submitCommand = vi.fn<(text: string) => Promise<CommandOutcome>>(() =>
  Promise.resolve(planFixture as unknown as CommandOutcome),
);
const resetConversation = vi.fn(() => Promise.resolve(null));
const downloadBrainModel = vi.fn(() => Promise.resolve(null));

vi.mock("../src/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../src/ipc")>();
  return {
    ...actual,
    desktopRuntime: true,
    sershi: {
      ...actual.sershi,
      submitCommand,
      resetConversation,
      downloadBrainModel,
      getBrainStatus: () => Promise.resolve(status("brain")),
      getSemanticStatus: () => Promise.resolve(status("semantic", { standby: true })),
      stopSpeaking: () => Promise.resolve(null),
    },
  };
});

const { Transcript } = await import("../src/components/conversation/Transcript");
const { useConversation } = await import("../src/state/conversation");
const { useUnderstanding } = await import("../src/state/understanding");
const { UnderstandingSettings, UnderstandingDiagnostics } =
  await import("../src/surfaces/command-center/UnderstandingSettings");
const { HomeView } = await import("../src/surfaces/command-center/HomeView");
const { composeReply } = await import("../src/i18n/domain");
const { createTranslator } = await import("../src/i18n/translate");
const { createFormatters } = await import("../src/i18n/format");
const { useLocaleStore } = await import("../src/i18n");

const plan = planFixture as unknown as CommandOutcome;
const report = plan.plan as PlanReport;
const [first, second, third] = report.steps as [PlanStepReport, PlanStepReport, PlanStepReport];

function status(
  role: "brain" | "semantic",
  overrides: Partial<SemanticStatus> = {},
): SemanticStatus {
  const brain = role === "brain";
  return {
    model: {
      id: brain ? "qwen3-4b-q4km" : "qwen3-1.7b-q4km",
      name: brain ? "Qwen3 4B" : "Qwen3 1.7B",
      quantization: "Q4_K_M",
      sizeBytes: brain ? 2_497_281_312 : 1_107_409_472,
      sha256: "0".repeat(64),
      license: "Apache-2.0",
      ramMb: brain ? 2525 : 1195,
      vramMb: brain ? 3243 : 1580,
      state: { kind: "notInstalled" },
    },
    enabled: true,
    available: true,
    running: false,
    backend: null,
    device: null,
    loadMs: null,
    lastMs: null,
    freeSpaceOk: true,
    standby: false,
    ...overrides,
  };
}

const reply = (locale: "en-US" | "es-419" | "pt-BR", outcome: CommandOutcome) =>
  composeReply(createTranslator(locale), createFormatters(locale), outcome);

beforeEach(() => {
  useLocaleStore.setState({ preference: "auto", systemLocale: "en-US", locale: "en-US" });
  useConversation.setState({ messages: [], pending: false, livePlan: null });
  useUnderstanding.setState({
    models: {
      brain: { status: null, enabled: true, error: null },
      semantic: { status: null, enabled: true, error: null },
    },
    last: null,
  });
  submitCommand.mockClear();
  resetConversation.mockClear();
  downloadBrainModel.mockClear();
});

describe("contracts", () => {
  it("accept Rust's plan and reject anything unbounded or path-like", () => {
    expect(isCommandOutcome(planFixture)).toBe(true);
    expect(isPlanReport(plan.plan)).toBe(true);
    const step = first;
    const six = { id: 1, done: false, steps: Array.from({ length: 6 }, () => step) };
    expect(isPlanReport(six)).toBe(false);
    const path = {
      ...plan.plan,
      steps: [{ ...step, application: "C:\\Windows\\System32\\cmd.exe" }],
    };
    expect(isPlanReport(path)).toBe(false);
    expect(isCommandOutcome({ ...planFixture, plan: path })).toBe(false);
  });
});

describe("plans are reported from trusted data, in every language", () => {
  it("say what each step did", () => {
    expect(reply("en-US", plan)).toMatch(/^Opened Spotify\. Opened Google Chrome\. You're using/);
    expect(reply("es-419", plan)).toMatch(/^Abrí Spotify\. Abrí Google Chrome\./);
    expect(reply("pt-BR", plan)).toMatch(/^Abri Spotify\. Abri Google Chrome\./);
  });

  it("never pretend a failed or waiting step succeeded", () => {
    const partial: CommandOutcome = {
      ...plan,
      status: "partial",
      plan: {
        ...report,
        steps: [
          { ...first, status: "completed" },
          { ...second, status: "failed" },
          { ...third, action: "close", status: "needsConfirmation" },
        ],
      },
    };
    const es = reply("es-419", partial);
    expect(es).toContain("Abrí Spotify.");
    expect(es).toContain("No pude «Abrir Google Chrome».");
    expect(es).toContain("espera tu aprobación");
  });

  it("refuses commands and over-long plans in the user's language", () => {
    const refusal: CommandOutcome = {
      ...plan,
      status: "answered",
      plan: null,
      detail: { kind: "answer", topic: "noCommands" },
    };
    expect(reply("es-419", refusal)).toMatch(/^No puedo ejecutar comandos/);
    const tooLong: CommandOutcome = {
      ...plan,
      status: "notUnderstood",
      plan: null,
      detail: { kind: "planTooLong", maxSteps: 5 },
    };
    expect(reply("pt-BR", tooLong)).toContain("mais de 5 passos");
  });
});

describe("the conversation", () => {
  it("shows the plan's steps, then the result", () => {
    render(
      <Transcript
        messages={[{ id: 1, role: "sershi", reply: { kind: "outcome", outcome: plan } }]}
      />,
    );
    expect(screen.getByRole("list", { name: "Plan" })).toBeTruthy();
    expect(screen.getByText("Open Spotify")).toBeTruthy();
    expect(screen.getByText("Check memory")).toBeTruthy();
  });

  it("follows a running plan live without adding half-done messages", () => {
    const running: PlanReport = {
      ...report,
      done: false,
      steps: [
        { ...first, status: "completed" },
        { ...second, status: "running" },
        { ...third, status: "pending" },
      ],
    };
    useConversation.getState().followPlan(running);
    useConversation.getState().addReply({ kind: "outcome", outcome: { ...plan, plan: running } });
    expect(useConversation.getState().messages).toHaveLength(0);
    render(<Transcript messages={[]} livePlan={useConversation.getState().livePlan} />);
    expect(screen.getByRole("list", { name: "Plan in progress" })).toBeTruthy();
    useConversation.getState().addReply({ kind: "outcome", outcome: plan });
    expect(useConversation.getState().livePlan).toBeNull();
    expect(useConversation.getState().messages).toHaveLength(1);
  });

  it("shows the brain's answer and offers its options as trusted applications", async () => {
    const question: CommandOutcome = {
      ...plan,
      status: "needsClarification",
      plan: null,
      detail: {
        kind: "brainQuestion",
        message: "Which one do you want to close: Google Chrome or Outlook?",
        options: [
          { id: "google-chrome", displayName: "Google Chrome", source: "startMenu" },
          { id: "outlook", displayName: "Outlook", source: "packagedApp" },
        ],
      },
    };
    useConversation.setState({
      messages: [{ id: 1, role: "sershi", reply: { kind: "outcome", outcome: question } }],
    });
    render(<Transcript messages={useConversation.getState().messages} />);
    expect(screen.getByText(/Which one do you want to close/)).toBeTruthy();
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Outlook" }));
      await Promise.resolve();
    });
    expect(submitCommand).toHaveBeenCalledWith("Outlook");
  });

  it("starts a new conversation in the core and on screen", () => {
    useConversation.setState({
      messages: [{ id: 1, role: "user", text: "Abre Excel", via: "typed" }],
    });
    render(<HomeView onViewActivity={() => undefined} />);
    fireEvent.click(screen.getByRole("button", { name: "New conversation" }));
    expect(resetConversation).toHaveBeenCalledTimes(1);
    expect(useConversation.getState().messages).toHaveLength(0);
  });
});

describe("intelligence settings", () => {
  it("offer the Agent Brain download with its size, never silently", async () => {
    render(<UnderstandingSettings />);
    const button = await screen.findByRole("button", { name: /Download \(2,497/ });
    expect(downloadBrainModel).not.toHaveBeenCalled();
    await act(async () => {
      fireEvent.click(button);
      await Promise.resolve();
    });
    expect(downloadBrainModel).toHaveBeenCalledTimes(1);
    // One model at a time: the router says it is on standby.
    expect(await screen.findByText("Not loaded while the Agent Brain is on.")).toBeTruthy();
  });

  it("refuse a download that the disk cannot hold", () => {
    useUnderstanding.setState({
      models: {
        brain: { status: status("brain", { freeSpaceOk: false }), enabled: true, error: null },
        semantic: { status: null, enabled: true, error: null },
      },
    });
    render(<UnderstandingSettings />);
    expect(screen.getByText("Not enough free disk space for this model.")).toBeTruthy();
    const button = screen.getByRole("button", { name: /Download \(2,497/ });
    expect((button as HTMLButtonElement).disabled).toBe(true);
  });

  it("developer diagnostics show the route, never the prompt", () => {
    useUnderstanding.getState().observe(plan, "Abre Spotify y Google Chrome");
    render(<UnderstandingDiagnostics />);
    // The route ("Agent Brain") and the brain row's label.
    expect(screen.getAllByText("Agent Brain").length).toBeGreaterThanOrEqual(2);
    expect(screen.getByText("Context and plan rules (no model)")).toBeTruthy();
    expect(document.body.textContent).not.toMatch(/im_start|<request>|steps":/);
  });
});
