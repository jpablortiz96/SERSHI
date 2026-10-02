/**
 * The trusted confirmation surface: what it shows, what it sends, and the
 * guards against accidental or inherited input. The IPC module is replaced
 * by a recorder; the real one is exercised by the contract tests.
 */
import type { CommandOutcome, ConfirmationChoice, ConfirmationRequest } from "@sershi/contracts";
import confirmationOutcome from "@sershi/contracts/fixtures/command-outcome-confirmation.json";
import fixture from "@sershi/contracts/fixtures/confirmation-request.json";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative, resolve } from "node:path";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const getContext = vi.fn<() => Promise<ConfirmationRequest>>();
const decide = vi.fn<(id: string, choice: ConfirmationChoice) => Promise<null>>();
vi.mock("../src/ipc/confirmation", () => ({
  desktopRuntime: true,
  confirmationSurface: { getContext, decide },
}));

const { ConfirmationSurface, ARMING_MS } =
  await import("../src/surfaces/confirmation/ConfirmationSurface");
const { Transcript } = await import("../src/components/conversation/Transcript");
const { useLocaleStore } = await import("../src/i18n");
const { sershi } = await import("../src/ipc");

const request = fixture as ConfirmationRequest;
const fresh = (overrides: Partial<ConfirmationRequest> = {}): ConfirmationRequest => ({
  ...request,
  expiresAtMs: Date.now() + 60_000,
  ...overrides,
});

async function show(value: ConfirmationRequest = fresh()) {
  getContext.mockResolvedValue(value);
  render(<ConfirmationSurface />);
  await act(async () => {
    await Promise.resolve();
  });
}

function arm() {
  act(() => {
    window.dispatchEvent(new Event("focus"));
    vi.advanceTimersByTime(ARMING_MS + 10);
  });
}

const approveButton = () => screen.getByRole("button", { name: "Close Spotify" });

describe("trusted confirmation surface", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    getContext.mockReset();
    decide.mockReset();
    decide.mockResolvedValue(null);
    useLocaleStore.setState({ preference: "auto", systemLocale: "en-US", locale: "en-US" });
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("shows its assigned confirmation with SERSHI's own copy and the trusted name", async () => {
    await show();
    expect(getContext).toHaveBeenCalledTimes(1);
    const dialog = screen.getByRole("dialog", { name: "Close Spotify?" });
    expect(dialog.getAttribute("aria-modal")).toBe("true");
    expect(dialog.textContent).toContain("Confirmation required");
    expect(dialog.textContent).toContain("Unsaved work may be lost");
    expect(approveButton()).toBeTruthy();
  });

  it("focuses Cancel first, so Enter cancels rather than approves", async () => {
    await show();
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Cancel" }));
  });

  it("Escape cancels, sending only the id and the choice", async () => {
    await show();
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(decide).toHaveBeenCalledTimes(1);
    expect(decide.mock.calls[0]).toEqual([request.id, "cancel"]);
  });

  it("the × button cancels; it never approves", async () => {
    await show();
    fireEvent.click(screen.getByRole("button", { name: "Cancel and close" }));
    expect(decide.mock.calls).toEqual([[request.id, "cancel"]]);
  });

  it("ignores Approve before the surface is armed", async () => {
    await show();
    fireEvent.pointerDown(approveButton());
    fireEvent.click(approveButton());
    expect(decide).not.toHaveBeenCalled();
    expect(approveButton().getAttribute("aria-disabled")).toBe("true");
  });

  it("a click that began before arming cannot approve", async () => {
    await show();
    fireEvent.pointerDown(approveButton());
    arm();
    fireEvent.click(approveButton());
    expect(decide).not.toHaveBeenCalled();
  });

  it("approves on a deliberate press after arming, exactly once", async () => {
    await show();
    arm();
    expect(approveButton().getAttribute("aria-disabled")).toBe("false");
    fireEvent.pointerDown(approveButton());
    fireEvent.click(approveButton());
    fireEvent.pointerDown(approveButton());
    fireEvent.click(approveButton());
    expect(decide.mock.calls).toEqual([[request.id, "approve"]]);
  });

  it("a held or repeating key cannot approve; a fresh key press can", async () => {
    await show();
    arm();
    const approve = approveButton();
    approve.focus();
    fireEvent.keyDown(approve, { key: "Enter", repeat: true });
    fireEvent.click(approve);
    expect(decide).not.toHaveBeenCalled();
    fireEvent.keyDown(approve, { key: "Enter" });
    fireEvent.click(approve);
    expect(decide.mock.calls).toEqual([[request.id, "approve"]]);
  });

  it("losing focus disarms Approve", async () => {
    await show();
    arm();
    act(() => {
      window.dispatchEvent(new Event("blur"));
    });
    fireEvent.pointerDown(approveButton());
    fireEvent.click(approveButton());
    expect(decide).not.toHaveBeenCalled();
  });

  it("is localized in Spanish and Portuguese", async () => {
    useLocaleStore.setState({ locale: "es-419" });
    await show();
    expect(screen.getByRole("dialog", { name: "¿Cerrar Spotify?" }).textContent).toContain(
      "Se requiere confirmación",
    );
    expect(screen.getByRole("button", { name: "Cerrar Spotify" })).toBeTruthy();
    act(() => {
      useLocaleStore.setState({ locale: "pt-BR" });
    });
    expect(screen.getByRole("dialog", { name: "Fechar Spotify?" }).textContent).toContain(
      "Confirmação necessária",
    );
    expect(screen.getByRole("button", { name: "Fechar Spotify" })).toBeTruthy();
  });

  it("localizes built-in application names by id", async () => {
    useLocaleStore.setState({ locale: "pt-BR" });
    await show(
      fresh({
        subject: {
          kind: "application",
          application: { id: "windows.notepad", displayName: "Notepad", source: "builtIn" },
        },
      }),
    );
    expect(screen.getByRole("dialog", { name: "Fechar Bloco de notas?" })).toBeTruthy();
  });

  it("an expired request can no longer be approved and sends nothing", async () => {
    await show(fresh({ expiresAtMs: Date.now() + 1_500 }));
    arm();
    act(() => {
      vi.advanceTimersByTime(3_000);
    });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.getByRole("status").textContent).toBe("This request expired.");
    expect(decide).not.toHaveBeenCalled();
  });

  it("shows nothing actionable if the core has no confirmation for it", async () => {
    getContext.mockRejectedValue(new Error("notAllowed"));
    render(<ConfirmationSurface />);
    await act(async () => {
      await Promise.resolve();
    });
    expect(screen.queryByRole("button")).toBeNull();
    expect(screen.getByRole("status").textContent).toBe(
      "This request is no longer waiting for approval.",
    );
  });
});

describe("the Command Center has no approval power", () => {
  it("its IPC facade cannot decide or read confirmations", () => {
    expect(Object.keys(sershi)).not.toContain("decideConfirmation");
    expect(Object.keys(sershi).some((k) => /confirmation/i.test(k))).toBe(false);
  });

  it("only the confirmation surface imports the confirmation IPC module", () => {
    const src = resolve(import.meta.dirname, "../src");
    const files: string[] = [];
    const walk = (dir: string) => {
      for (const name of readdirSync(dir)) {
        const path = join(dir, name);
        if (statSync(path).isDirectory()) walk(path);
        else if (/\.tsx?$/.test(name)) files.push(path);
      }
    };
    walk(src);
    const importers = files
      .filter((f) => /ipc\/confirmation"/.test(readFileSync(f, "utf8")))
      .map((f) => relative(src, f).replaceAll("\\", "/"));
    expect(importers).toEqual(["surfaces/confirmation/ConfirmationSurface.tsx"]);
  });

  it("a pending approval shows as waiting, with no approve button", () => {
    vi.useRealTimers();
    useLocaleStore.setState({ locale: "en-US" });
    render(
      <Transcript
        messages={[
          {
            id: 1,
            role: "sershi",
            reply: { kind: "outcome", outcome: confirmationOutcome as CommandOutcome },
          },
        ]}
      />,
    );
    expect(
      // Gate 4.1.1: it names the application, before anything ran.
      screen.getByText(
        "Spotify is waiting for your approval to close. Approve it in the secure confirmation window.",
      ),
    ).toBeTruthy();
    expect(screen.queryByRole("button", { name: /close spotify|approve|allow/i })).toBeNull();
  });
});
