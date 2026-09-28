import { ASSISTANT_STATES } from "@sershi/contracts";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { Core } from "../src/components/core/Core";
import { STATE_COPY } from "../src/components/core/stateCopy";

describe("Core", () => {
  it.each(ASSISTANT_STATES)("renders %s with an accessible description", (state) => {
    render(<Core state={state} size={120} />);
    const core = screen.getByRole("img");
    expect(core.dataset.state).toBe(state);
    expect(core.getAttribute("aria-label")).toContain(STATE_COPY[state].label);
  });

  it("every state has distinct words, so nothing depends on seeing motion", () => {
    const labels = ASSISTANT_STATES.map((s) => STATE_COPY[s].label);
    expect(new Set(labels).size).toBe(labels.length);
  });
});
