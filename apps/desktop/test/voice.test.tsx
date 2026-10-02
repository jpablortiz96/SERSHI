/**
 * Voice in the Command Center (Prompt 3): push-to-talk, what SERSHI heard,
 * notices and the model prompt — and the security invariant that voice is
 * input and output only.
 */
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import type { CaptureStart, CommandOutcome, VoiceSettings, VoiceStatus } from "@sershi/contracts";
import { isVoiceLevel, isVoiceUpdate } from "@sershi/contracts";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const startVoiceCapture = vi.fn<() => Promise<CaptureStart>>(() =>
  Promise.resolve({ kind: "started" }),
);
const stopVoiceCapture = vi.fn(() => Promise.resolve(null));
const cancelVoiceCapture = vi.fn(() => Promise.resolve(null));
const stopSpeaking = vi.fn(() => Promise.resolve(null));
const speakReply = vi.fn<(text: string, language: string | null) => Promise<null>>(() =>
  Promise.resolve(null),
);
const downloadVoiceModel = vi.fn<(model: string) => Promise<null>>(() => Promise.resolve(null));
const configureVoice = vi.fn<(settings: VoiceSettings) => Promise<VoiceStatus>>(() =>
  Promise.resolve(status()),
);

vi.mock("../src/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../src/ipc")>();
  return {
    ...actual,
    desktopRuntime: true,
    sershi: {
      ...actual.sershi,
      startVoiceCapture,
      stopVoiceCapture,
      cancelVoiceCapture,
      stopSpeaking,
      speakReply,
      downloadVoiceModel,
      configureVoice,
      getVoiceStatus: () => Promise.resolve(status()),
      dismissAssistant: () => Promise.resolve(null),
      onFocusCommand: () => () => undefined,
    },
  };
});

const { CommandBar } = await import("../src/components/command/CommandBar");
const { Transcript } = await import("../src/components/conversation/Transcript");
const { useAssistantStore } = await import("../src/state/assistant");
const { useConversation } = await import("../src/state/conversation");
const { useVoice, handleVoiceUpdate, voiceSettings } = await import("../src/state/voice");
const { parsePreferences, DEFAULT_PREFERENCES } = await import("../src/i18n/preferences");
const { cueForTransition } = await import("../src/audio/connect");
const { useLocaleStore } = await import("../src/i18n");

function status(overrides: Partial<VoiceStatus> = {}): VoiceStatus {
  return {
    supported: true,
    microphone: { access: "allowed", devices: [], fallback: false },
    models: [
      {
        id: "whisper-small-q8",
        profile: "fast",
        fileName: "ggml-small-q8_0.bin",
        quantization: "q8_0",
        sizeBytes: 264_464_607,
        sha256: "49c8fb02b65e6049d5fa6c04f81f53b867b5ec9540406812c643f177317f779f",
        memoryMb: 300,
        state: { kind: "notInstalled" },
      },
      {
        id: "whisper-large-v3-turbo-q8",
        profile: "accurate",
        fileName: "ggml-large-v3-turbo-q8_0.bin",
        quantization: "q8_0",
        sizeBytes: 874_188_075,
        sha256: "317eb69c11673c9de1e1f0d459b253999804ec71ac4c23c17ecf5fbe24e259a1",
        memoryMb: 950,
        state: { kind: "notInstalled" },
      },
    ],
    profile: "fast",
    model: "whisper-small-q8",
    acceleration: "vulkan",
    accelerator: "NVIDIA GeForce RTX 3050 6GB Laptop GPU",
    voices: [],
    capturing: false,
    speaking: false,
    ...overrides,
  };
}

function setState(state: "idle" | "listening" | "transcribing" | "speaking") {
  useAssistantStore.setState({
    snapshot: { state, previewState: null, revision: Date.now() },
  });
}

const outcome: CommandOutcome = {
  status: "completed",
  reply: "Opened Spotify.",
  detail: null,
  toolId: "system.open_application",
  data: {
    kind: "opened",
    application: { id: "spotify", displayName: "Spotify", source: "startMenu" },
  },
  durationMs: 120,
  understood: null,
  understanding: null,
  plan: null,
  brain: null,
  session: null,
};

beforeEach(() => {
  vi.clearAllMocks();
  useLocaleStore.setState({ locale: "en-US" });
  useConversation.setState({ messages: [], pending: false });
  useVoice.setState({
    status: status(),
    notice: null,
    prefs: { ...useVoice.getState().prefs, voiceResponses: true },
  });
  setState("idle");
});

describe("push-to-talk", () => {
  it("starts listening only when the microphone button is pressed", async () => {
    render(<CommandBar />);
    expect(startVoiceCapture).not.toHaveBeenCalled();
    const mic = screen.getByRole("button", { name: "Talk to SERSHI" });
    expect(mic.getAttribute("aria-pressed")).toBe("false");
    await act(async () => {
      await Promise.resolve();
      fireEvent.click(mic);
    });
    expect(startVoiceCapture).toHaveBeenCalledTimes(1);
  });

  it("shows listening unmistakably and stops-and-sends on a second press", () => {
    setState("listening");
    render(<CommandBar />);
    const mic = screen.getByRole("button", { name: "Stop listening and send" });
    expect(mic.getAttribute("aria-pressed")).toBe("true");
    expect(mic.dataset.mic).toBe("listening");
    expect(screen.getByRole("textbox").getAttribute("placeholder")).toMatch(/^Listening…/);
    fireEvent.click(mic);
    expect(stopVoiceCapture).toHaveBeenCalledTimes(1);
    expect(startVoiceCapture).not.toHaveBeenCalled();
  });

  it("Escape cancels listening, from the input or anywhere", () => {
    setState("listening");
    render(<CommandBar />);
    fireEvent.keyDown(screen.getByRole("textbox"), { key: "Escape" });
    expect(cancelVoiceCapture).toHaveBeenCalledTimes(1);
    fireEvent.keyDown(screen.getByRole("button", { name: "Stop listening and send" }), {
      key: "Escape",
    });
    expect(cancelVoiceCapture).toHaveBeenCalledTimes(2);
  });

  it("cannot be pressed while transcribing", () => {
    setState("transcribing");
    render(<CommandBar />);
    const mic = screen.getByRole("button", { name: "Understanding what you said" });
    expect(mic).toHaveProperty("disabled", true);
  });

  it("offers Stop speaking only while SERSHI speaks", () => {
    const first = render(<CommandBar />);
    expect(screen.queryByRole("button", { name: "Stop speaking" })).toBeNull();
    first.unmount();
    setState("speaking");
    render(<CommandBar />);
    fireEvent.click(screen.getByRole("button", { name: "Stop speaking" }));
    expect(stopSpeaking).toHaveBeenCalledTimes(1);
  });

  it("asks before downloading a missing model, with size and storage", async () => {
    startVoiceCapture.mockResolvedValueOnce({ kind: "refused", reason: "modelMissing" });
    render(<CommandBar />);
    await act(async () => {
      await Promise.resolve();
      fireEvent.click(screen.getByRole("button", { name: "Talk to SERSHI" }));
    });
    expect(screen.getByText("Local speech model required")).toBeTruthy();
    expect(screen.getByText("Whisper Small")).toBeTruthy();
    expect(screen.getAllByText("264 MB")).toHaveLength(2);
    expect(downloadVoiceModel).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Download" }));
    expect(downloadVoiceModel).toHaveBeenCalledWith("whisper-small-q8");
  });

  it("explains a denied microphone with actionable guidance", async () => {
    startVoiceCapture.mockResolvedValueOnce({ kind: "refused", reason: "permissionDenied" });
    render(<CommandBar />);
    await act(async () => {
      await Promise.resolve();
      fireEvent.click(screen.getByRole("button", { name: "Talk to SERSHI" }));
    });
    const alert = screen.getByRole("alert");
    expect(alert.textContent).toMatch(/Microphone unavailable/);
    expect(alert.textContent).toMatch(/Privacy & security › Microphone/);
  });
});

describe("what SERSHI heard", () => {
  it("shows the transcript as “You said” before the result, then speaks the reply", () => {
    handleVoiceUpdate({ kind: "heard", text: "Abre Spotify", language: "es", firstHeard: null });
    handleVoiceUpdate({ kind: "answered", outcome, speak: true, anythingElse: false });
    const { messages } = useConversation.getState();
    expect(messages.map((m) => m.role)).toEqual(["user", "sershi"]);
    render(<Transcript messages={messages} />);
    expect(screen.getByText("You said")).toBeTruthy();
    expect(screen.getByText("Abre Spotify")).toBeTruthy();
    expect(speakReply).toHaveBeenCalledWith("Opened Spotify.", "en-US");
  });

  it("does not speak when voice responses are off", () => {
    useVoice.setState({ prefs: { ...useVoice.getState().prefs, voiceResponses: false } });
    handleVoiceUpdate({ kind: "answered", outcome, speak: true, anythingElse: false });
    expect(speakReply).not.toHaveBeenCalled();
  });

  it("silence and unclear speech become notices, never commands", () => {
    handleVoiceUpdate({ kind: "noSpeech" });
    handleVoiceUpdate({ kind: "unclear" });
    const { messages } = useConversation.getState();
    expect(messages.every((m) => m.role === "sershi")).toBe(true);
    render(<Transcript messages={messages} />);
    expect(screen.getByText("No speech detected.")).toBeTruthy();
    expect(screen.getByText(/couldn't understand that clearly/)).toBeTruthy();
  });

  it("phrases the reply in the interface language, whatever language was spoken", () => {
    useLocaleStore.setState({ locale: "es-419" });
    handleVoiceUpdate({ kind: "heard", text: "Open Spotify", language: "en", firstHeard: null });
    handleVoiceUpdate({ kind: "answered", outcome, speak: true, anythingElse: false });
    expect(speakReply).toHaveBeenCalledWith("Abrí Spotify.", "es-419");
  });
});

describe("voice is input and output, never authorization", () => {
  const src = resolve(import.meta.dirname, "../src");
  const read = (path: string) => readFileSync(resolve(src, path), "utf8");

  it("the voice modules have no path to confirmation decisions or ids", () => {
    for (const file of [
      "state/voice.ts",
      "state/speech.ts",
      "components/command/VoicePanel.tsx",
      "surfaces/command-center/VoiceSettings.tsx",
      "visual/voiceLevel.ts",
    ]) {
      const code = read(file);
      expect(code, file).not.toMatch(/decide|decideConfirmation|confirmationId|ipc\/confirmation/);
    }
  });

  it("the trusted confirmation surface imports no voice code", () => {
    for (const file of [
      "surfaces/confirmation/ConfirmationSurface.tsx",
      "surfaces/confirmation/main.tsx",
      "ipc/confirmation.ts",
    ]) {
      expect(read(file), file).not.toMatch(/voice|speech|Voice|Speech|microphone/);
    }
    // The shared bootstrap must not pull the IPC facade (and with it the
    // voice commands) into the confirmation window's bundle.
    expect(read("surfaces/bootstrap.tsx")).not.toMatch(/from "\.\.\/ipc"/);
  });

  it("the companion only receives a level, never controls voice", () => {
    const code = read("surfaces/companion/Companion.tsx");
    expect(code).not.toMatch(/startVoiceCapture|speakReply|configureVoice|download/);
  });

  it("voice events carry text and levels only, validated at the edge", () => {
    expect(isVoiceUpdate({ kind: "approve" })).toBe(false);
    expect(isVoiceUpdate({ kind: "heard", text: 1 })).toBe(false);
    expect(isVoiceLevel({ level: 2, source: "input" })).toBe(false);
    expect(isVoiceLevel({ level: 0.4, source: "input", samples: [1] })).toBe(true);
  });
});

describe("voice preferences", () => {
  it("default to microphone off-until-pressed, spoken voice replies, silent typing", () => {
    expect(DEFAULT_PREFERENCES.voiceResponses).toBe(true);
    expect(DEFAULT_PREFERENCES.speakTypedResponses).toBe(false);
    expect(DEFAULT_PREFERENCES.conversationLanguage).toBe("automatic");
    expect(DEFAULT_PREFERENCES.microphone).toBeNull();
  });

  it("reject unknown languages and control characters", () => {
    const parsed = parsePreferences(
      JSON.stringify({
        conversationLanguage: "klingon",
        microphone: "{0.0.1}\u0000evil",
        speechVoice: "x".repeat(600),
        voiceResponses: false,
      }),
    );
    expect(parsed.conversationLanguage).toBe("automatic");
    expect(parsed.microphone).toBeNull();
    expect(parsed.speechVoice).toBeNull();
    expect(parsed.voiceResponses).toBe(false);
  });

  it("map to the core's settings (automatic = no language hint)", () => {
    expect(voiceSettings({ ...DEFAULT_PREFERENCES }).language).toBeNull();
    expect(voiceSettings({ ...DEFAULT_PREFERENCES, conversationLanguage: "pt" }).language).toBe(
      "pt",
    );
  });
});

describe("interface sounds and speech", () => {
  it("never cue over the microphone or a spoken reply", () => {
    expect(cueForTransition("idle", "listening")).toBeNull();
    expect(cueForTransition("success", "speaking")).toBeNull();
    expect(cueForTransition("executing", "success", true)).toBeNull();
    expect(cueForTransition("executing", "success", false)).toBe("success");
    // Approval requests always ask, spoken or not.
    expect(cueForTransition("planning", "awaitingConfirmation", true)).toBe("confirmation");
  });
});

describe("Gate 3B: fast, accurate and measured", () => {
  it("offers Fast and Accurate with exact model details and the active backend", async () => {
    const { VoiceSettings } = await import("../src/surfaces/command-center/VoiceSettings");
    render(<VoiceSettings />);
    expect(screen.getByText("Fast")).toBeTruthy();
    expect(screen.getByText("Accurate")).toBeTruthy();
    expect(screen.getByText(/Whisper Small · q8_0 · 264 MB/)).toBeTruthy();
    expect(screen.getByText(/Whisper Large v3 Turbo · q8_0 · 874 MB/)).toBeTruthy();
    expect(screen.getByText(/Graphics card · NVIDIA GeForce RTX 3050/)).toBeTruthy();
    expect(screen.queryByText(/Very slow without a graphics card/)).toBeNull();
  });

  it("without a GPU shows the processor and warns that Accurate is slow", async () => {
    useVoice.setState({ status: status({ acceleration: "cpu", accelerator: null }) });
    const { VoiceSettings } = await import("../src/surfaces/command-center/VoiceSettings");
    render(<VoiceSettings />);
    expect(screen.getByText("Processor")).toBeTruthy();
    expect(screen.getByText(/Very slow without a graphics card/)).toBeTruthy();
  });

  it("explains that Automatic language may be slower", async () => {
    const { VoiceSettings } = await import("../src/surfaces/command-center/VoiceSettings");
    render(<VoiceSettings />);
    expect(screen.getByText(/Automatic detects the language every time you speak/)).toBeTruthy();
  });

  it("profiles and developer overrides are validated when stored", () => {
    const parsed = parsePreferences(JSON.stringify({ speechProfile: "turbo", endpointMs: 5 }));
    expect(parsed.speechProfile).toBe("fast");
    expect(parsed.endpointMs).toBeNull();
    const accurate = parsePreferences(
      JSON.stringify({ speechProfile: "accurate", endpointMs: 550 }),
    );
    expect(accurate.speechProfile).toBe("accurate");
    expect(voiceSettings({ ...DEFAULT_PREFERENCES, ...accurate }).profile).toBe("accurate");
    expect(voiceSettings({ ...DEFAULT_PREFERENCES, ...accurate }).endpointMs).toBe(550);
  });

  it("timings are diagnostics only: shown in Developer Mode, never a command", async () => {
    handleVoiceUpdate({
      kind: "timings",
      timings: {
        speechStartMs: 300,
        speechMs: 900,
        endpointMs: 610,
        modelLoadMs: null,
        sttMs: 340,
        postCaptureMs: 360,
        speechEndToTranscriptMs: 970,
        pipelineMs: 45,
        toolMs: 30,
        speculative: true,
        detectedLanguage: false,
        language: "es",
        confidencePct: 91,
        firstLanguage: null,
        retryAccepted: false,
        languageRetryMs: null,
        acceleration: "vulkan",
        model: "whisper-small-q8",
      },
    });
    handleVoiceUpdate({ kind: "speechLatency", ms: 180 });
    expect(useConversation.getState().messages).toHaveLength(0);
    const { VoiceDiagnostics } = await import("../src/surfaces/command-center/VoiceSettings");
    render(<VoiceDiagnostics />);
    expect(screen.getByText("Last word → transcript")).toBeTruthy();
    expect(screen.getByText(/early decode · fixed language/)).toBeTruthy();
  });

  it("rejects malformed timing events at the edge", () => {
    expect(isVoiceUpdate({ kind: "timings", timings: { model: 1 } })).toBe(false);
    expect(isVoiceUpdate({ kind: "speechLatency", ms: "fast" })).toBe(false);
  });
});
