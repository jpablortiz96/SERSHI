import type { CommandOutcome } from "@sershi/contracts";
import ambiguous from "@sershi/contracts/fixtures/command-outcome-ambiguous.json";
import notFound from "@sershi/contracts/fixtures/command-outcome-not-found.json";
import opened from "@sershi/contracts/fixtures/command-outcome-opened.json";
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";

import { Transcript } from "../src/components/conversation/Transcript";
import { useLocaleStore, type Locale } from "../src/i18n";
import { composeReply, describeActivity } from "../src/i18n/domain";
import { createFormatters } from "../src/i18n/format";
import { createTranslator } from "../src/i18n/translate";
import { useConversation, type Message } from "../src/state/conversation";

const reply = (locale: Locale, outcome: unknown) =>
  composeReply(createTranslator(locale), createFormatters(locale), outcome as CommandOutcome);

describe("application replies", () => {
  it("say what happened, in every language", () => {
    expect(reply("en-US", opened)).toBe("Opened Spotify.");
    expect(reply("es-419", opened)).toBe("Abrí Spotify.");
    expect(reply("pt-BR", opened)).toBe("Abri Spotify.");
    expect(reply("en-US", notFound)).toBe("I couldn't find Photoshop on this computer.");
    expect(reply("es-419", notFound)).toBe("No encontré Photoshop en esta computadora.");
    expect(reply("pt-BR", notFound)).toBe("Não encontrei Photoshop neste computador.");
    expect(reply("es-419", ambiguous)).toContain("«visual studio»");
  });

  it("localize Windows components by id", () => {
    const calculator = {
      ...(opened as CommandOutcome),
      data: {
        kind: "opened",
        matched: "alias",
        application: { id: "windows.calculator", displayName: "Calculator", source: "builtIn" },
      },
    };
    expect(reply("es-419", calculator)).toBe("Abrí Calculadora.");
    expect(reply("pt-BR", calculator)).toBe("Abri Calculadora.");
  });

  it("activity names the trusted application, never the request text", () => {
    const t = createTranslator("es-419");
    expect(
      describeActivity(t, {
        id: 1,
        atMs: 0,
        kind: "toolCompleted",
        toolId: "system.open_application",
        subject: "Spotify",
        summary: "Open application completed",
        durationMs: 3,
      }),
    ).toBe("Se abrió Spotify");
  });
});

describe("ambiguous requests", () => {
  beforeEach(() => {
    useLocaleStore.setState({ preference: "auto", systemLocale: "en-US", locale: "pt-BR" });
    useConversation.setState({ messages: [], pending: false });
  });

  it("offer each candidate and re-ask with its exact name in the user's language", async () => {
    const messages: Message[] = [
      { id: 1, role: "sershi", reply: { kind: "outcome", outcome: ambiguous as CommandOutcome } },
    ];
    render(<Transcript messages={messages} />);
    const choice = screen.getByRole("button", { name: "Visual Studio Code" });
    expect(screen.getByRole("button", { name: "Visual Studio 2022" })).toBeTruthy();
    fireEvent.click(choice);
    // Browser preview: the request is recorded, answered honestly as offline.
    await screen.findByText("Visual Studio Code");
    const [sent] = useConversation.getState().messages;
    expect(sent?.role === "user" && sent.text).toBe("Abra Visual Studio Code");
  });
});
