/**
 * Natural command understanding in the Command Center (Gate 3C):
 * clarifications, "SERSHI understood", the waiting state, the semantic
 * model's settings — and that none of it can approve or run anything.
 */
import type { CommandOutcome, SemanticSettings, SemanticStatus } from "@sershi/contracts";
import { isCommandOutcome, isSemanticStatus } from "@sershi/contracts";
import clarification from "@sershi/contracts/fixtures/command-outcome-clarification.json";
import understoodFixture from "@sershi/contracts/fixtures/command-outcome-understood.json";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const submitCommand = vi.fn<(text: string) => Promise<CommandOutcome>>(() =>
  Promise.resolve(understoodFixture as CommandOutcome),
);
const configureSemantic = vi.fn<(settings: SemanticSettings) => Promise<SemanticStatus>>(
  (settings) => Promise.resolve(semanticStatus({ enabled: settings.enabled })),
);
const downloadSemanticModel = vi.fn(() => Promise.resolve(null));

vi.mock("../src/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../src/ipc")>();
  return {
    ...actual,
    desktopRuntime: true,
    sershi: {
      ...actual.sershi,
      submitCommand,
      configureSemantic,
      downloadSemanticModel,
      getSemanticStatus: () => Promise.resolve(semanticStatus()),
      stopSpeaking: () => Promise.resolve(null),
    },
  };
});

const { Transcript } = await import("../src/components/conversation/Transcript");
const { useConversation } = await import("../src/state/conversation");
const { useUnderstanding } = await import("../src/state/understanding");
const { UnderstandingSettings, UnderstandingDiagnostics } =
  await import("../src/surfaces/command-center/UnderstandingSettings");
const { composeReply, clarificationReply } = await import("../src/i18n/domain");
const { createTranslator } = await import("../src/i18n/translate");
const { createFormatters } = await import("../src/i18n/format");
const { useLocaleStore } = await import("../src/i18n");
const { STATE_VISUALS } = await import("../src/visual/stateVisuals");
const { parsePreferences } = await import("../src/i18n/preferences");

function semanticStatus(overrides: Partial<SemanticStatus> = {}): SemanticStatus {
  return {
    model: {
      id: "qwen3-1.7b-q4km",
      name: "Qwen3 1.7B",
      quantization: "Q4_K_M",
      sizeBytes: 1_107_409_472,
      sha256: "b139949c5bd74937ad8ed8c8cf3d9ffb1e99c866c823204dc42c0d91fa181897",
      license: "Apache-2.0",
      ramMb: 1195,
      vramMb: 1580,
      state: { kind: "notInstalled" },
    },
    enabled: true,
    available: true,
    running: false,
    backend: null,
    device: null,
    loadMs: null,
    lastMs: null,
    ...overrides,
  };
}

const outcome = (value: unknown) => value as CommandOutcome;

beforeEach(() => {
  useLocaleStore.setState({ preference: "auto", systemLocale: "en-US", locale: "en-US" });
  useConversation.setState({ messages: [], pending: false });
  useUnderstanding.setState({ status: null, enabled: true, error: null, last: null });
  submitCommand.mockClear();
  configureSemantic.mockClear();
  downloadSemanticModel.mockClear();
});

describe("contracts", () => {
  it("accept Rust's clarification and understood outcomes", () => {
    expect(isCommandOutcome(clarification)).toBe(true);
    expect(isCommandOutcome(understoodFixture)).toBe(true);
    expect(isSemanticStatus(semanticStatus())).toBe(true);
  });

  it("reject a clarification carrying anything but trusted summaries", () => {
    const withPath = structuredClone(clarification) as Record<string, unknown>;
    (withPath.detail as { clarification: { candidates: unknown[] } }).clarification.candidates = [
      "C:\\Windows\\System32\\cmd.exe",
    ];
    expect(isCommandOutcome(withPath)).toBe(false);
    expect(isCommandOutcome({ ...clarification, confirmationId: "0".repeat(32) })).toBe(false);
  });
});

describe("questions and repairs, in every language", () => {
  it("asks which application with numbered options", () => {
    const t = (l: "en-US" | "es-419" | "pt-BR") =>
      composeReply(createTranslator(l), createFormatters(l), outcome(clarification));
    expect(t("en-US")).toBe(
      "I found more than one option: 1. Visual Studio 2022; 2. Visual Studio Code. Which one do you want?",
    );
    expect(t("es-419")).toContain("¿Cuál quieres?");
    expect(t("pt-BR")).toContain("Qual você quer?");
  });

  it("phrases did-you-mean for opening and closing differently", () => {
    const word = { id: "word", displayName: "Word", source: "startMenu" as const };
    const es = createTranslator("es-419");
    expect(clarificationReply(es, { kind: "didYouMean", action: "open", candidates: [word] })).toBe(
      "¿Quisiste decir Word?",
    );
    expect(
      clarificationReply(es, { kind: "didYouMean", action: "close", candidates: [word] }),
    ).toBe("¿Quieres que cierre Word?");
    expect(
      clarificationReply(createTranslator("pt-BR"), {
        kind: "whichApplication",
        action: "open",
        candidates: [],
      }),
    ).toBe("Qual aplicativo você quer abrir?");
  });

  it("acknowledges a negated request without acting", () => {
    const noAction = outcome({
      ...understoodFixture,
      status: "answered",
      toolId: null,
      data: null,
      understood: null,
      detail: { kind: "answer", topic: "noAction" },
    });
    const es = createTranslator("es-419");
    expect(composeReply(es, createFormatters("es-419"), noAction)).toBe(
      "De acuerdo, no haré nada.",
    );
  });
});

describe("the transcript", () => {
  it("shows what SERSHI understood only for repaired requests", () => {
    render(
      <Transcript
        messages={[
          { id: 1, role: "user", text: "Apreer Spotify", via: "voice" },
          {
            id: 2,
            role: "sershi",
            reply: { kind: "outcome", outcome: outcome(understoodFixture) },
          },
        ]}
      />,
    );
    expect(screen.getByText("SERSHI understood:")).toBeTruthy();
    expect(screen.getByText("Open Spotify")).toBeTruthy();
  });

  it("offers the candidates as answers that name only an offered application", async () => {
    useConversation.setState({
      messages: [
        { id: 1, role: "sershi", reply: { kind: "outcome", outcome: outcome(clarification) } },
      ],
    });
    render(<Transcript messages={useConversation.getState().messages} />);
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Visual Studio Code" }));
      await Promise.resolve();
    });
    expect(submitCommand).toHaveBeenCalledWith("Visual Studio Code");
  });

  it("answers did-you-mean with yes or no, and never on an old question", async () => {
    const didYouMean = outcome({
      ...clarification,
      detail: {
        kind: "clarification",
        clarification: {
          kind: "didYouMean",
          action: "open",
          candidates: [{ id: "word", displayName: "Word", source: "startMenu" }],
        },
      },
    });
    const messages = [
      { id: 1, role: "sershi" as const, reply: { kind: "outcome" as const, outcome: didYouMean } },
      { id: 2, role: "user" as const, text: "hola", via: "typed" as const },
    ];
    const { rerender } = render(<Transcript messages={messages} />);
    expect(screen.queryByRole("button", { name: "Yes" })).toBeNull();
    rerender(<Transcript messages={messages.slice(0, 1)} />);
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Yes" }));
      await Promise.resolve();
    });
    expect(submitCommand).toHaveBeenCalledWith("Yes");
  });
});

describe("the waiting state", () => {
  it("attends without looking busy", () => {
    const visual = STATE_VISUALS.waitingForClarification;
    expect(visual.busy).toBe(false);
    expect(visual.attention).toBe(true);
  });
});

describe("settings", () => {
  it("offer the download with its size, and never download silently", async () => {
    render(<UnderstandingSettings />);
    await act(async () => {
      await useUnderstanding.getState().refresh();
    });
    const button = await screen.findByRole("button", { name: /Download \(1,107/ });
    expect(downloadSemanticModel).not.toHaveBeenCalled();
    await act(async () => {
      fireEvent.click(button);
      await Promise.resolve();
    });
    expect(downloadSemanticModel).toHaveBeenCalledTimes(1);
    expect(screen.getByText(/The model only interprets what you say/)).toBeTruthy();
  });

  it("let the user turn the installed model off, remembered locally", async () => {
    useUnderstanding.setState({
      status: semanticStatus({
        model: { ...semanticStatus().model, state: { kind: "installed" } },
      }),
    });
    render(<UnderstandingSettings />);
    await act(async () => {
      fireEvent.click(screen.getByRole("radio", { name: "Off" }));
      await Promise.resolve();
    });
    expect(configureSemantic).toHaveBeenCalledWith({ enabled: false });
    expect(
      parsePreferences(localStorage.getItem("sershi.preferences.v1")).naturalUnderstanding,
    ).toBe(false);
  });

  it("developer diagnostics show the resolution, never a prompt", () => {
    useUnderstanding.getState().observe(outcome(understoodFixture), "Apreer Spotify");
    render(<UnderstandingDiagnostics />);
    expect(screen.getByText("Misheard command word")).toBeTruthy();
    expect(screen.getByText("apreer spotify")).toBeTruthy();
    expect(document.body.textContent).not.toMatch(/im_start|<request>/);
  });
});
