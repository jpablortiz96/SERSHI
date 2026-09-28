import type { CommandOutcome, ConfirmationRequest } from "@sershi/contracts";
import confirmationFixture from "@sershi/contracts/fixtures/command-outcome-confirmation.json";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

// Pretend to run inside the desktop shell, with a recording IPC.
const decideConfirmation =
  vi.fn<(r: ConfirmationRequest, approved: boolean) => Promise<CommandOutcome>>();
vi.mock("../src/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../src/ipc")>();
  return {
    ...actual,
    desktopRuntime: true,
    sershi: { ...actual.sershi, decideConfirmation },
  };
});

const { ConfirmationDialog } = await import("../src/components/confirmation/ConfirmationDialog");
const { useConfirmation } = await import("../src/state/confirmation");
const { useConversation } = await import("../src/state/conversation");
const { useLocaleStore } = await import("../src/i18n");

const request = (confirmationFixture as { confirmation: ConfirmationRequest }).confirmation;
const fresh = (): ConfirmationRequest => ({ ...request, expiresAtMs: Date.now() + 90_000 });

const cancelled: CommandOutcome = {
  status: "cancelled",
  reply: "Cancelled. Nothing was changed.",
  detail: null,
  toolId: request.toolId,
  data: null,
  confirmation: null,
  durationMs: null,
};

describe("trusted confirmation dialog", () => {
  beforeEach(() => {
    decideConfirmation.mockReset();
    decideConfirmation.mockResolvedValue(cancelled);
    useConfirmation.getState().clear();
    useConversation.setState({ messages: [], pending: false });
    useLocaleStore.setState({ preference: "auto", systemLocale: "en-US", locale: "en-US" });
  });

  it("renders the trusted, resolved subject with dialog semantics", () => {
    act(() => {
      useConfirmation.getState().show(fresh());
    });
    render(<ConfirmationDialog />);
    const dialog = screen.getByRole("dialog", { name: "Close Spotify?" });
    expect(dialog.getAttribute("aria-modal")).toBe("true");
    expect(dialog.textContent).toContain("Unsaved work may be lost");
    expect(screen.getByRole("button", { name: "Close Spotify" })).toBeTruthy();
  });

  it("focuses Cancel first, so a stray Enter can never approve", () => {
    act(() => {
      useConfirmation.getState().show(fresh());
    });
    render(<ConfirmationDialog />);
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Cancel" }));
  });

  it("Escape cancels and sends only the decision for this id", async () => {
    act(() => {
      useConfirmation.getState().show(fresh());
    });
    render(<ConfirmationDialog />);
    await act(async () => {
      fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
      await Promise.resolve();
    });
    expect(decideConfirmation).toHaveBeenCalledTimes(1);
    const [sent, approved] = decideConfirmation.mock.calls[0] ?? [];
    expect(approved).toBe(false);
    expect(sent?.id).toBe(request.id);
    expect(sent?.toolId).toBe("system.close_application");
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("approving sends one decision even if clicked repeatedly", async () => {
    let finish: (o: CommandOutcome) => void = () => undefined;
    decideConfirmation.mockReturnValue(
      new Promise((resolve) => {
        finish = resolve;
      }),
    );
    act(() => {
      useConfirmation.getState().show(fresh());
    });
    render(<ConfirmationDialog />);
    const approve = screen.getByRole("button", { name: "Close Spotify" });
    fireEvent.click(approve);
    fireEvent.click(approve);
    expect(decideConfirmation).toHaveBeenCalledTimes(1);
    expect(decideConfirmation.mock.calls[0]?.[1]).toBe(true);
    await act(async () => {
      finish({ ...cancelled, status: "completed" });
      await Promise.resolve();
    });
    expect(useConversation.getState().messages).toHaveLength(1);
  });

  it("traps focus inside the dialog", () => {
    act(() => {
      useConfirmation.getState().show(fresh());
    });
    render(<ConfirmationDialog />);
    const cancel = screen.getByRole("button", { name: "Cancel" });
    const approve = screen.getByRole("button", { name: "Close Spotify" });
    approve.focus();
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Tab" });
    expect(document.activeElement).toBe(cancel);
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Tab", shiftKey: true });
    expect(document.activeElement).toBe(approve);
  });

  it("returns focus to where it was when the dialog closes", () => {
    const opener = document.createElement("button");
    document.body.appendChild(opener);
    opener.focus();
    act(() => {
      useConfirmation.getState().show(fresh());
    });
    render(<ConfirmationDialog />);
    act(() => {
      useConfirmation.getState().clear();
    });
    expect(document.activeElement).toBe(opener);
    opener.remove();
  });

  it("is localized, using SERSHI's own copy plus the trusted name", () => {
    useLocaleStore.setState({ locale: "es-419" });
    act(() => {
      useConfirmation.getState().show(fresh());
    });
    render(<ConfirmationDialog />);
    expect(screen.getByRole("dialog", { name: "¿Cerrar Spotify?" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Cancelar" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Cerrar Spotify" })).toBeTruthy();
  });

  it("localizes built-in application names by id", () => {
    useLocaleStore.setState({ locale: "pt-BR" });
    act(() => {
      useConfirmation.getState().show({
        ...fresh(),
        subject: {
          kind: "application",
          application: { id: "windows.notepad", displayName: "Notepad", source: "builtIn" },
        },
      });
    });
    render(<ConfirmationDialog />);
    expect(screen.getByRole("dialog", { name: "Fechar Bloco de notas?" })).toBeTruthy();
  });

  it("an expired request closes and is reported, without deciding", () => {
    vi.useFakeTimers();
    try {
      act(() => {
        useConfirmation.getState().show({ ...request, expiresAtMs: Date.now() + 1_500 });
      });
      render(<ConfirmationDialog />);
      act(() => {
        vi.advanceTimersByTime(3_000);
      });
      expect(screen.queryByRole("dialog")).toBeNull();
      expect(decideConfirmation).not.toHaveBeenCalled();
      const [message] = useConversation.getState().messages;
      expect(message?.role === "sershi" && message.reply.kind).toBe("expired");
    } finally {
      vi.useRealTimers();
    }
  });

  it("the developer preview can never reach the core", () => {
    act(() => {
      useConfirmation.getState().showPreview(fresh());
    });
    render(<ConfirmationDialog />);
    fireEvent.click(screen.getByRole("button", { name: "Close Spotify" }));
    expect(decideConfirmation).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});
