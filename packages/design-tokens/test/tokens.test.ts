import { describe, expect, it } from "vitest";

import { applyTheme, createTheme, toCssText, toCssVariables, tokens } from "../src";

describe("design tokens", () => {
  const vars = toCssVariables(tokens);

  it("produce valid, predictable CSS variable names", () => {
    for (const name of vars.keys()) {
      expect(name).toMatch(/^--[a-z0-9]+(-[a-z0-9]+)*$/);
    }
    expect(vars.get("--color-signal-deep")).toBe(tokens.color.signalDeep);
    expect(vars.get("--type-hero-size")).toBe(tokens.type.hero.size);
    expect(vars.get("--duration-fast")).toBe(tokens.motion.duration.fast);
    expect(vars.get("--ease-enter")).toBe(tokens.motion.ease.enter);
    expect(vars.get("--space-0-5")).toBe("2px");
  });

  it("define every semantic state the UI can show", () => {
    for (const state of [
      "sleeping",
      "idle",
      "awake",
      "listening",
      "thinking",
      "planning",
      "executing",
      "speaking",
      "success",
      "warning",
      "awaiting-confirmation",
      "error",
      "offline",
      "private",
      "disabled",
    ]) {
      expect(vars.get(`--state-${state}`), state).toMatch(/^#[0-9a-f]{6}$/);
    }
  });

  it("keep motion durations within the documented budget", () => {
    const ms = (v: string) => Number(v.replace("ms", ""));
    const { breath, drift, ...interactive } = tokens.motion.duration;
    for (const [name, value] of Object.entries(interactive)) {
      expect(ms(value), name).toBeLessThanOrEqual(720);
    }
    expect(ms(breath)).toBeGreaterThan(ms(tokens.motion.duration.cinematic));
    expect(ms(drift)).toBeGreaterThan(ms(breath));
  });

  it("render as CSS text", () => {
    expect(toCssText(tokens)).toContain("--surface-base: #07080b;");
  });
});

describe("themes", () => {
  it("override only known tokens", () => {
    const theme = createTheme({
      state: { listening: "#00ff88" },
      // @ts-expect-error — themes cannot invent tokens
      state2: { nope: "x" },
    });
    expect(theme.state.listening).toBe("#00ff88");
    expect(theme.state.thinking).toBe(tokens.state.thinking);
    expect("state2" in theme).toBe(false);
  });

  it("ignore overrides that would change the token tree's shape", () => {
    const malformed = JSON.parse('{"type": "12px", "state": {"idle": {"x": 1}}}') as object;
    const theme = createTheme(malformed);
    expect(theme.type.hero.size).toBe(tokens.type.hero.size);
    expect(theme.state.idle).toBe(tokens.state.idle);
  });

  it("apply through the CSSOM", () => {
    const root = document.createElement("div");
    applyTheme(root, createTheme({ surface: { base: "#000000" } }));
    expect(root.style.getPropertyValue("--surface-base")).toBe("#000000");
    expect(root.style.getPropertyValue("--state-idle")).toBe(tokens.state.idle);
  });
});

describe("accessibility", () => {
  const luminance = (hex: string) => {
    const channels = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255);
    const [r = 0, g = 0, b = 0] = channels.map((c) =>
      c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4,
    );
    return 0.2126 * r + 0.7152 * g + 0.0722 * b;
  };
  const contrast = (a: string, b: string) => {
    const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x) as [number, number];
    return (hi + 0.05) / (lo + 0.05);
  };

  it.each(["primary", "secondary", "tertiary"] as const)(
    "text.%s meets WCAG AA (4.5:1) on every opaque surface",
    (role) => {
      for (const surface of [tokens.surface.base, tokens.surface.raised, tokens.surface.overlay]) {
        expect(contrast(tokens.text[role], surface)).toBeGreaterThanOrEqual(4.5);
      }
    },
  );
});
