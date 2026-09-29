/**
 * Companion 2.0, command acceptance and window transitions — behaviour only.
 * The IPC module is replaced by a recorder.
 */
import type { PresenceUpdate } from "@sershi/contracts";
import { act, fireEvent, render, renderHook, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const handlers: { presence?: (u: PresenceUpdate) => void; focus?: () => void } = {};
const summonCommandCenter = vi.fn(() => Promise.resolve(null));
const hideCommandCenter = vi.fn(() => Promise.resolve(null));
const startDragging = vi.fn();

vi.mock("../src/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../src/ipc")>();
  return {
    ...actual,
    currentWindow: { ...actual.currentWindow, startDragging },
    sershi: {
      ...actual.sershi,
      summonCommandCenter,
      hideCommandCenter,
      onPresence: (h: (u: PresenceUpdate) => void) => {
        handlers.presence = h;
        return () => undefined;
      },
      onFocusCommand: (h: () => void) => {
        handlers.focus = h;
        return () => undefined;
      },
    },
  };
});

const { Companion, DRAG_THRESHOLD } = await import("../src/surfaces/companion/Companion");
const { CommandBar, ACCEPTED_MS } = await import("../src/components/command/CommandBar");
const { useWindowPresence, WINDOW_EXIT_MS } = await import("../src/visual/windowPresence");
const { useAppearance } = await import("../src/visual/appearance");
const { useAssistantStore } = await import("../src/state/assistant");
const { useConversation } = await import("../src/state/conversation");
const { useLocaleStore } = await import("../src/i18n");

const core = () => document.querySelector<HTMLElement>("[data-variant='companion']");

beforeEach(() => {
  summonCommandCenter.mockClear();
  hideCommandCenter.mockClear();
  startDragging.mockClear();
  useLocaleStore.setState({ preference: "auto", systemLocale: "en-US", locale: "en-US" });
  useAppearance.setState({ motion: "system", companionSize: "medium", systemReduced: false });
  useAssistantStore.setState({
    snapshot: { state: "idle", previewState: null, revision: 1 },
    localPreview: null,
  });
});

describe("companion 2.0", () => {
  it("summons on click and is never an approval control", () => {
    render(<Companion />);
    const button = screen.getByRole("button", { name: /open sershi command center/i });
    fireEvent.pointerDown(button, { button: 0, clientX: 10, clientY: 10 });
    fireEvent.pointerUp(button);
    expect(summonCommandCenter).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("button", { name: /approve|allow|close spotify/i })).toBeNull();
  });

  it("a drag moves the window and never summons", () => {
    vi.useFakeTimers();
    try {
      render(<Companion />);
      const button = screen.getByRole("button", { name: /open sershi command center/i });
      fireEvent.pointerDown(button, { button: 0, clientX: 10, clientY: 10 });
      fireEvent.pointerMove(button, { clientX: 10 + DRAG_THRESHOLD + 2, clientY: 10 });
      fireEvent.pointerUp(button);
      expect(startDragging).toHaveBeenCalledTimes(1);
      expect(summonCommandCenter).not.toHaveBeenCalled();
      expect(document.querySelector("main")?.dataset.dragging).toBe("true");
      act(() => {
        vi.advanceTimersByTime(1000);
      });
      expect(document.querySelector("main")?.dataset.dragging).toBeUndefined();
    } finally {
      vi.useRealTimers();
    }
  });

  it("is calmer while the Command Center is visible, more present when it is gone", () => {
    render(<Companion />);
    expect(core()?.dataset.intensity).toBe("calm");
    act(() => {
      handlers.presence?.({ commandCenterVisible: false });
    });
    expect(core()?.dataset.intensity).toBe("normal");
    act(() => {
      useAssistantStore.setState({
        snapshot: { state: "awaitingConfirmation", previewState: null, revision: 2 },
      });
    });
    expect(core()?.dataset.intensity).toBe("present");
    expect(core()?.dataset.pattern).toBe("await");
  });

  it("follows the companion size setting", () => {
    render(<Companion />);
    expect(core()?.style.getPropertyValue("--core-size")).toBe("104px");
    act(() => {
      useAppearance.setState({ companionSize: "large" });
    });
    expect(core()?.style.getPropertyValue("--core-size")).toBe("120px");
  });
});

describe("command acceptance", () => {
  beforeEach(() => {
    useConversation.setState({ messages: [], pending: false });
  });

  it("clears the input, shows the accepted command lifting away, and sends once", () => {
    vi.useFakeTimers();
    try {
      render(<CommandBar />);
      const input = screen.getByRole<HTMLInputElement>("textbox", { name: "Command" });
      fireEvent.change(input, { target: { value: "Open Notepad" } });
      fireEvent.submit(input);
      expect(input.value).toBe("");
      expect(screen.getByText("Open Notepad", { selector: "span" })).toBeTruthy();
      expect(useConversation.getState().messages.filter((m) => m.role === "user")).toHaveLength(1);
      act(() => {
        vi.advanceTimersByTime(ACCEPTED_MS + 10);
      });
      expect(screen.queryByText("Open Notepad", { selector: "span" })).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });
});

describe("window transitions", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("plays a short exit, then asks Rust to hide the window", () => {
    vi.useFakeTimers();
    const { result } = renderHook(() => useWindowPresence());
    act(() => {
      result.current.hide();
    });
    expect(result.current.presence).toBe("leaving");
    expect(hideCommandCenter).not.toHaveBeenCalled();
    act(() => {
      vi.advanceTimersByTime(WINDOW_EXIT_MS + 5);
    });
    expect(hideCommandCenter).toHaveBeenCalledTimes(1);
  });

  it("hides immediately with reduced motion", () => {
    useAppearance.setState({ motion: "reduced" });
    const { result } = renderHook(() => useWindowPresence());
    act(() => {
      result.current.hide();
    });
    expect(hideCommandCenter).toHaveBeenCalledTimes(1);
    expect(result.current.presence).toBe("present");
  });

  it("uses one entrance for every summon origin, without replaying on rapid summons", () => {
    vi.useFakeTimers();
    const { result } = renderHook(() => useWindowPresence());
    expect(result.current.presence).toBe("entering");
    act(() => {
      vi.advanceTimersByTime(400);
    });
    expect(result.current.presence).toBe("present");
    act(() => {
      handlers.focus?.();
    });
    // Within the replay guard: no second entrance.
    expect(result.current.presence).toBe("present");
    act(() => {
      vi.advanceTimersByTime(2000);
      handlers.focus?.();
    });
    expect(result.current.presence).toBe("entering");
  });
});
