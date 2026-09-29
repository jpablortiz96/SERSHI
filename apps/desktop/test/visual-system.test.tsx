/**
 * Visual Experience 2.0: the state → visual mapping, the Core's variants,
 * reduced-motion coverage and contextual presence. Tests behaviour, not
 * pixels (screenshots cover the look; see docs/VISUAL_EXPERIENCE.md).
 */
import { ASSISTANT_STATES, type AssistantState } from "@sershi/contracts";
import { render, screen } from "@testing-library/react";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative, resolve } from "node:path";
import { beforeEach, describe, expect, it } from "vitest";

import { Core } from "../src/components/core/Core";
import { StateGlyph } from "../src/components/core/StateGlyph";
import { useLocaleStore } from "../src/i18n";
import { companionIntensity, STATE_VISUALS, stateVisual } from "../src/visual/stateVisuals";

describe("state visual configuration", () => {
  it("covers every assistant state", () => {
    expect(Object.keys(STATE_VISUALS).sort()).toEqual([...ASSISTANT_STATES].sort());
  });

  it("gives thinking, planning and executing distinct motion and shapes", () => {
    const busy: AssistantState[] = ["thinking", "planning", "executing"];
    expect(new Set(busy.map((s) => stateVisual(s).pattern)).size).toBe(3);
    expect(new Set(busy.map((s) => stateVisual(s).glyph)).size).toBe(3);
  });

  it("never asks for attention while idle; always for outcomes and approvals", () => {
    for (const quiet of ["sleeping", "idle", "awake"] as const) {
      expect(stateVisual(quiet).attention, quiet).toBe(false);
    }
    for (const loud of ["success", "warning", "awaitingConfirmation", "error"] as const) {
      expect(stateVisual(loud).attention, loud).toBe(true);
    }
  });

  it("keeps energy within 0–1 and idle quieter than executing", () => {
    for (const s of ASSISTANT_STATES) {
      expect(stateVisual(s).energy).toBeGreaterThan(0);
      expect(stateVisual(s).energy).toBeLessThanOrEqual(1);
    }
    expect(stateVisual("idle").energy).toBeLessThan(stateVisual("executing").energy);
  });

  it("gives outcome states shapes that do not rely on colour", () => {
    const shapes = (["success", "warning", "error", "awaitingConfirmation"] as const).map(
      (s) => stateVisual(s).glyph,
    );
    expect(new Set(shapes).size).toBe(shapes.length);
  });
});

describe("contextual presence", () => {
  it("is calmer while the Command Center is on screen, and present when alone", () => {
    expect(companionIntensity("idle", true)).toBe("calm");
    expect(companionIntensity("idle", false)).toBe("normal");
  });

  it("is always fully present while busy or when the user is needed", () => {
    for (const s of ["thinking", "executing", "awaitingConfirmation", "error"] as const) {
      expect(companionIntensity(s, true), s).toBe("present");
      expect(companionIntensity(s, false), s).toBe("present");
    }
  });
});

describe("Core renderer variants", () => {
  beforeEach(() => {
    useLocaleStore.setState({ preference: "auto", systemLocale: "en-US", locale: "en-US" });
  });

  it.each(ASSISTANT_STATES)("renders the configured pattern for %s", (state) => {
    render(<Core state={state} />);
    const core = screen.getByRole("img");
    expect(core.dataset.pattern).toBe(stateVisual(state).pattern);
    expect(core.dataset.variant).toBe("hero");
  });

  it("uses the same renderer for every variant, with less detail when small", () => {
    const { container } = render(
      <>
        <Core state="thinking" variant="companion" decorative />
        <Core state="thinking" variant="compact" decorative />
      </>,
    );
    const [companion, compact] = [...container.querySelectorAll<HTMLElement>("[data-variant]")];
    expect(companion?.childElementCount).toBeGreaterThan(compact?.childElementCount ?? 0);
    expect(compact?.dataset.pattern).toBe("compute");
  });

  it("decorative cores are hidden from assistive technology", () => {
    const { container } = render(<Core state="idle" variant="compact" decorative />);
    expect(screen.queryByRole("img")).toBeNull();
    expect(container.firstElementChild?.getAttribute("aria-hidden")).toBe("true");
  });

  it("state glyphs are decorative shapes next to a text label", () => {
    const { container } = render(<StateGlyph state="error" />);
    const svg = container.querySelector("svg");
    expect(svg?.getAttribute("aria-hidden")).toBe("true");
    expect(svg?.dataset.glyph).toBe("cross");
  });
});

describe("reduced motion", () => {
  const src = resolve(import.meta.dirname, "../src");
  const cssFiles = (dir: string): string[] =>
    readdirSync(dir).flatMap((name) => {
      const path = join(dir, name);
      return statSync(path).isDirectory() ? cssFiles(path) : path.endsWith(".css") ? [path] : [];
    });
  const sheets = cssFiles(src).map((f) => [relative(src, f), readFileSync(f, "utf8")] as const);

  it("scans every stylesheet", () => {
    expect(sheets.length).toBeGreaterThan(10);
  });

  it.each(sheets.filter(([, css]) => /\binfinite\b/.test(css)))(
    "%s neutralises its looping animations under reduced motion",
    (_name, css) => {
      expect(css).toContain('html[data-motion="reduced"]');
    },
  );

  it("uses one switch: modules key on html[data-motion], not their own media queries", () => {
    for (const [name, css] of sheets) {
      if (name === "styles/global.css") continue;
      expect(css, name).not.toContain("prefers-reduced-motion");
    }
  });

  it("the global switch stops every animation and transition", () => {
    const global = sheets.find(([name]) => name === "styles/global.css")?.[1] ?? "";
    expect(global).toMatch(/html\[data-motion="reduced"\] \*[\s\S]*animation-duration/);
    expect(global).toMatch(/html\[data-motion="reduced"\] \*[\s\S]*transition-duration/);
  });
});
