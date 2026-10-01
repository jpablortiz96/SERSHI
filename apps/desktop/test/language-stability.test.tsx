/**
 * Gate 3C.1: a short speech-recognition language mistake must not change
 * SERSHI's language, and a language retry stays transparent.
 */
import type { CommandOutcome, VoiceSettings, VoiceStatus } from "@sershi/contracts";
import { isVoiceUpdate } from "@sershi/contracts";
import opened from "@sershi/contracts/fixtures/command-outcome-opened.json";
import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const speakReply = vi.fn<(text: string, language: string | null) => Promise<null>>(() =>
  Promise.resolve(null),
);
const configureVoice = vi.fn<(settings: VoiceSettings) => Promise<VoiceStatus>>(() =>
  Promise.reject(new Error("not needed")),
);

vi.mock("../src/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../src/ipc")>();
  return {
    ...actual,
    desktopRuntime: true,
    sershi: {
      ...actual.sershi,
      speakReply,
      configureVoice,
      stopSpeaking: () => Promise.resolve(null),
      onVoice: () => () => undefined,
      onVoiceModel: () => () => undefined,
    },
  };
});

const { Transcript } = await import("../src/components/conversation/Transcript");
const { useConversation } = await import("../src/state/conversation");
const { useVoice, handleVoiceUpdate, voiceSettings, connectVoice } =
  await import("../src/state/voice");
const { useLocaleStore } = await import("../src/i18n");
const { DEFAULT_PREFERENCES } = await import("../src/i18n/preferences");

const outcome = opened as CommandOutcome;
const notUnderstood: CommandOutcome = {
  status: "notUnderstood",
  reply: "I didn't understand that.",
  detail: null,
  toolId: null,
  data: null,
  durationMs: null,
  understood: null,
  understanding: null,
};

beforeEach(() => {
  useConversation.setState({ messages: [], pending: false });
  useVoice.setState({ prefs: { ...DEFAULT_PREFERENCES, voiceResponses: true } });
  useLocaleStore.setState({ preference: "auto", systemLocale: "es-419", locale: "es-419" });
  speakReply.mockClear();
  configureVoice.mockClear();
});

describe("response language", () => {
  it.each([
    ["es-419", "ru", "Abrí Spotify."],
    ["es-419", "is", "Abrí Spotify."],
    ["en-US", "is", "Opened Spotify."],
    ["pt-BR", "ru", "Abri Spotify."],
  ])("stays %s when recognition detected %s", (locale, detected, reply) => {
    useLocaleStore.setState({ locale: locale as "es-419" });
    handleVoiceUpdate({ kind: "heard", text: "Пон Мекром", language: detected, firstHeard: null });
    handleVoiceUpdate({ kind: "answered", outcome });
    expect(speakReply).toHaveBeenCalledWith(reply, locale);
  });

  it("a misunderstood utterance is answered in the interface language too", () => {
    handleVoiceUpdate({ kind: "heard", text: "Ári Óðluk", language: "is", firstHeard: null });
    handleVoiceUpdate({ kind: "answered", outcome: notUnderstood });
    const [text, language] = speakReply.mock.calls[0] ?? [];
    expect(text).toBe(
      "No entendí eso. Prueba, por ejemplo, «Abre Chrome» o «¿Cuánta memoria estoy usando?».",
    );
    expect(language).toBe("es-419");
    expect(text).not.toMatch(/[Ѐ-ӿ]|ð/);
  });
});

describe("a language retry is transparent", () => {
  it("shows the accepted text, labelled, with what was first heard", () => {
    handleVoiceUpdate({
      kind: "heard",
      text: "Ponme Chrome",
      language: "es",
      firstHeard: "Пон Мекром",
    });
    render(<Transcript messages={useConversation.getState().messages} />);
    expect(screen.getByText("Reconocido de nuevo")).toBeTruthy();
    expect(screen.getByText("Ponme Chrome")).toBeTruthy();
    expect(screen.getByText(/Primero se oyó “Пон Мекром”/)).toBeTruthy();
    expect(screen.queryByText("Dijiste")).toBeNull();
  });

  it("an ordinary transcript is just “You said”", () => {
    handleVoiceUpdate({ kind: "heard", text: "Abre Outlook", language: "es", firstHeard: null });
    render(<Transcript messages={useConversation.getState().messages} />);
    expect(screen.getByText("Dijiste")).toBeTruthy();
    expect(screen.queryByText(/Primero se oyó/)).toBeNull();
  });

  it("the contract accepts the first hearing only as text", () => {
    expect(isVoiceUpdate({ kind: "heard", text: "a", language: "es", firstHeard: "b" })).toBe(true);
    expect(isVoiceUpdate({ kind: "heard", text: "a", language: "es", firstHeard: null })).toBe(
      true,
    );
    expect(isVoiceUpdate({ kind: "heard", text: "a", language: "es", firstHeard: 3 })).toBe(false);
  });
});

describe("the interface language is a recognition hint, never a lock", () => {
  it("is sent with the voice settings; the conversation language stays separate", () => {
    const settings = voiceSettings({ ...DEFAULT_PREFERENCES }, "pt-BR");
    expect(settings.interfaceLanguage).toBe("pt-BR");
    expect(settings.language).toBeNull();
    const fixed = voiceSettings({ ...DEFAULT_PREFERENCES, conversationLanguage: "en" }, "es-419");
    expect(fixed.language).toBe("en");
    expect(fixed.interfaceLanguage).toBe("es-419");
  });

  it("follows interface language changes", () => {
    const disconnect = connectVoice();
    expect(configureVoice).toHaveBeenLastCalledWith(
      expect.objectContaining({ interfaceLanguage: "es-419" }),
    );
    useLocaleStore.setState({ locale: "en-US" });
    expect(configureVoice).toHaveBeenLastCalledWith(
      expect.objectContaining({ interfaceLanguage: "en-US", language: null }),
    );
    disconnect();
  });
});
