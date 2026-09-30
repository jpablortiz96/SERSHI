import { ASSISTANT_STATES, type CommandOutcome } from "@sershi/contracts";
import completed from "@sershi/contracts/fixtures/command-outcome-completed.json";
import answer from "@sershi/contracts/fixtures/command-outcome-answer.json";
import unavailable from "@sershi/contracts/fixtures/command-outcome-unavailable.json";
import { beforeEach, describe, expect, it } from "vitest";

import { MESSAGES, SUPPORTED_LOCALES, useLocaleStore } from "../src/i18n";
import { detectSystemLocale, matchLocale, resolveLocale } from "../src/i18n/detect";
import { composeReply, describeActivity, stateLabel, stateLine } from "../src/i18n/domain";
import { createFormatters } from "../src/i18n/format";
import { DEFAULT_PREFERENCES, parsePreferences, PREFERENCES_KEY } from "../src/i18n/preferences";
import { createTranslator, flattenMessages, interpolate } from "../src/i18n/translate";

const canonical = flattenMessages(MESSAGES["en-US"]);
const placeholders = (s: string) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();

describe("locale resources", () => {
  it.each(SUPPORTED_LOCALES)("%s has exactly the canonical keys", (locale) => {
    const table = flattenMessages(MESSAGES[locale]);
    expect([...table.keys()].sort()).toEqual([...canonical.keys()].sort());
  });

  it.each(SUPPORTED_LOCALES)("%s has no empty messages and matching placeholders", (locale) => {
    const table = flattenMessages(MESSAGES[locale]);
    for (const [key, english] of canonical) {
      const message = table.get(key) ?? "";
      expect(message.trim(), key).not.toBe("");
      expect(placeholders(message), key).toEqual(placeholders(english));
    }
  });

  it.each(SUPPORTED_LOCALES)("%s labels every assistant state", (locale) => {
    const t = createTranslator(locale);
    for (const state of ASSISTANT_STATES) {
      expect(stateLabel(t, state)).not.toMatch(/^state\./);
      expect(stateLine(t, state)).not.toMatch(/^state\./);
    }
  });

  it("translations are not accidental copies of English", () => {
    for (const locale of ["es-419", "pt-BR"] as const) {
      const table = flattenMessages(MESSAGES[locale]);
      const identical = [...canonical].filter(([k, v]) => table.get(k) === v).map(([k]) => k);
      // Only brand names, key caps, units and punctuation-only patterns may match.
      expect(identical.sort()).toEqual(
        [
          "activity.withSubject",
          "settings.sources.appPaths",
          "settings.sources.builtIn",
          "settings.sources.packagedApp",
          "command.keyEnter",
          "command.keyEscape",
          "core.label",
          "platforms.linux",
          "platforms.macos",
          "platforms.windows",
          // Product names (like "SERSHI"): themes and the Orbital companion.
          "settings.appearance.themes.dark",
          "settings.appearance.themes.light",
          "settings.appearance.companions.orbital",
          "system.memoryTotal",
          // Speech model names are product names.
          "voice.models.whisper-large-v3-turbo-q8",
          "voice.models.whisper-small-q8",
          // Technical terms shown in Developer Mode.
          "settings.developer.voice.backend",
          // A punctuation-only pattern (Gate 3C).
          "reply.clarify.option",
          // A number-and-percent pattern; pt-BR writes it like English.
          ...(locale === "pt-BR" ? ["system.threads", "settings.appearance.volumeValue"] : []),
          // "No" is the same word in Spanish.
          ...(locale === "es-419" ? ["settings.sections.general", "reply.clarify.no"] : []),
        ].sort(),
      );
    }
  });
});

describe("locale detection", () => {
  it.each([
    ["es-MX", "es-419"],
    ["es-ES", "es-419"],
    ["es", "es-419"],
    ["pt-BR", "pt-BR"],
    ["pt-PT", "pt-BR"],
    ["en-GB", "en-US"],
    ["EN-us", "en-US"],
    ["es_CO", "es-419"],
  ] as const)("maps %s to %s", (tag, expected) => {
    expect(matchLocale(tag)).toBe(expected);
  });

  it("falls back to English for unsupported or missing languages", () => {
    expect(detectSystemLocale(["fr-FR", "de-DE"])).toBe("en-US");
    expect(detectSystemLocale([])).toBe("en-US");
    expect(matchLocale("")).toBeNull();
    expect(matchLocale(undefined)).toBeNull();
    expect(matchLocale("zz-invalid-🙂")).toBeNull();
  });

  it("uses the first supported language in the system's preference order", () => {
    expect(detectSystemLocale(["fr-FR", "pt-BR", "es-MX"])).toBe("pt-BR");
  });

  it("a manual choice overrides the system language", () => {
    expect(resolveLocale("es-419", "pt-BR")).toBe("es-419");
    expect(resolveLocale("auto", "pt-BR")).toBe("pt-BR");
  });
});

describe("preferences", () => {
  beforeEach(() => {
    localStorage.clear();
    useLocaleStore.setState({ preference: "auto", systemLocale: "pt-BR", locale: "pt-BR" });
  });

  it("ignores corrupt or unsupported stored values", () => {
    for (const raw of [null, "", "{", "[]", '{"uiLocale":"fr-FR"}', '{"uiLocale":42}']) {
      expect(parsePreferences(raw)).toEqual(DEFAULT_PREFERENCES);
    }
    expect(parsePreferences('{"uiLocale":"es-419"}').uiLocale).toBe("es-419");
  });

  it("persists a manual override and returns to the system language on Automatic", () => {
    const { setPreference } = useLocaleStore.getState();
    setPreference("es-419");
    expect(useLocaleStore.getState().locale).toBe("es-419");
    expect(parsePreferences(localStorage.getItem(PREFERENCES_KEY)).uiLocale).toBe("es-419");

    setPreference("auto");
    expect(useLocaleStore.getState().locale).toBe("pt-BR");
    expect(parsePreferences(localStorage.getItem(PREFERENCES_KEY)).uiLocale).toBe("auto");
  });

  it("keeps conversation language independent of the interface language", () => {
    useLocaleStore.getState().setPreference("pt-BR");
    expect(parsePreferences(localStorage.getItem(PREFERENCES_KEY)).conversationLanguage).toBe(
      "automatic",
    );
  });
});

describe("translation", () => {
  it("interpolates placeholders and leaves unknown ones visible", () => {
    expect(interpolate("{a} and {b}", { a: 1 })).toBe("1 and {b}");
  });

  it("never crashes on an unsupported locale at runtime", () => {
    // Simulates corrupted state reaching the translator.
    const t = createTranslator("xx-XX" as never);
    expect(t("nav.home")).toBe("Home");
    expect(createFormatters("xx-XX" as never).percent(50)).toBe("50%");
  });
});

describe("formatting", () => {
  it("formats numbers with each locale's conventions", () => {
    expect(createFormatters("en-US").gigabytes(1.5 * 1024 ** 3)).toBe("1.5");
    expect(createFormatters("pt-BR").gigabytes(1.5 * 1024 ** 3)).toBe("1,5");
    expect(createFormatters("en-US").percent(67.4)).toBe("67%");
    expect(createFormatters("pt-BR").percentParts(67.4).number).toBe("67");
  });

  it("formats times per locale", () => {
    const at = Date.UTC(2026, 0, 1, 15, 59, 12);
    const tz = new Date(at).getHours(); // local-time sanity for the 24h check
    expect(createFormatters("pt-BR").time(at)).toMatch(
      new RegExp(`^${String(tz).padStart(2, "0")}:59:12$`),
    );
  });
});

describe("core outcomes are phrased in the interface language", () => {
  it.each(SUPPORTED_LOCALES)("%s", (locale) => {
    const t = createTranslator(locale);
    const f = createFormatters(locale);
    for (const outcome of [completed, answer, unavailable] as CommandOutcome[]) {
      const text = composeReply(t, f, outcome);
      expect(text.length).toBeGreaterThan(0);
      if (locale !== "en-US") expect(text).not.toBe(outcome.reply);
    }
    expect(composeReply(t, f, completed as CommandOutcome)).toContain(f.gigabytes(32 * 1024 ** 3));
  });

  it("falls back to the core's English reply for tools without translations", () => {
    const t = createTranslator("es-419");
    const outcome = { ...(completed as CommandOutcome), toolId: "spotify.play" };
    expect(composeReply(t, createFormatters("es-419"), outcome)).toBe(outcome.reply);
  });

  it("activity entries are described from their kind, not the English summary", () => {
    const t = createTranslator("es-419");
    const text = describeActivity(t, {
      id: 1,
      atMs: 0,
      kind: "toolCompleted",
      toolId: "system.get_memory",
      subject: null,
      summary: "Memory usage completed",
      durationMs: 0,
    });
    expect(text).toBe("Completado: Uso de memoria");
  });
});
