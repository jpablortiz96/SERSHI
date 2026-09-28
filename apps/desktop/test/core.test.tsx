import { ASSISTANT_STATES } from "@sershi/contracts";
import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";

import { Core } from "../src/components/core/Core";
import { SUPPORTED_LOCALES, useLocaleStore } from "../src/i18n";
import { stateLabel } from "../src/i18n/domain";
import { createTranslator } from "../src/i18n/translate";

describe("Core", () => {
  beforeEach(() => {
    useLocaleStore.setState({ preference: "auto", systemLocale: "en-US", locale: "en-US" });
  });

  it.each(ASSISTANT_STATES)("renders %s with an accessible description", (state) => {
    render(<Core state={state} size={120} />);
    const core = screen.getByRole("img");
    expect(core.dataset.state).toBe(state);
    expect(core.getAttribute("aria-label")).toContain(stateLabel(createTranslator("en-US"), state));
  });

  it.each(SUPPORTED_LOCALES)(
    "every state has distinct words in %s, so nothing depends on seeing motion",
    (locale) => {
      const t = createTranslator(locale);
      const labels = ASSISTANT_STATES.map((s) => stateLabel(t, s));
      expect(new Set(labels).size).toBe(labels.length);
      expect(labels.every((l) => l.trim().length > 0)).toBe(true);
    },
  );

  it("describes itself in the selected language", () => {
    useLocaleStore.setState({ locale: "pt-BR" });
    render(<Core state="thinking" size={120} />);
    expect(screen.getByRole("img").getAttribute("aria-label")).toBe("SERSHI — Pensando");
  });
});
