import { beforeEach, describe, expect, it } from "vitest";

import { selectDisplayState, useAssistantStore } from "../src/state/assistant";

describe("assistant store", () => {
  beforeEach(() => {
    useAssistantStore.setState({
      snapshot: { state: "idle", previewState: null, revision: 0 },
      localPreview: null,
    });
  });

  it("ignores snapshots older than the one it holds", () => {
    const { receive } = useAssistantStore.getState();
    receive({ state: "success", previewState: null, revision: 5 });
    receive({ state: "thinking", previewState: null, revision: 3 });
    expect(useAssistantStore.getState().snapshot.state).toBe("success");
  });

  it("renders previews over the real state without replacing it", () => {
    const { receive, setLocalPreview } = useAssistantStore.getState();
    receive({ state: "idle", previewState: "speaking", revision: 1 });
    expect(selectDisplayState(useAssistantStore.getState())).toBe("speaking");
    setLocalPreview("error");
    expect(selectDisplayState(useAssistantStore.getState())).toBe("error");
    expect(useAssistantStore.getState().snapshot.state).toBe("idle");
  });
});
