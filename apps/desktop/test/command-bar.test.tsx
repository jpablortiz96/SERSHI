import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";

import { CommandBar } from "../src/components/command/CommandBar";
import { desktopRuntime, IpcFailure, sershi } from "../src/ipc";
import { useConversation } from "../src/state/conversation";

describe("command bar (browser preview)", () => {
  beforeEach(() => {
    useConversation.setState({ messages: [], pending: false });
  });

  it("runs outside the desktop shell in tests", () => {
    expect(desktopRuntime).toBe(false);
  });

  it("cannot send an empty command and offers voice only as a labelled future feature", () => {
    render(<CommandBar />);
    expect(screen.getByRole("button", { name: "Send" })).toHaveProperty("disabled", true);
    const mic = screen.getByRole("button", { name: /voice input, coming in v0\.3/i });
    expect(mic).toHaveProperty("disabled", true);
  });

  it("submits on Enter and answers honestly that the core is not connected", async () => {
    render(<CommandBar />);
    const input = screen.getByRole("textbox", { name: "Command" });
    fireEvent.change(input, { target: { value: "How much memory am I using?" } });
    fireEvent.submit(input);

    await screen.findByDisplayValue("");
    const { messages } = useConversation.getState();
    expect(messages.map((m) => m.role)).toEqual(["user", "sershi"]);
    expect(messages[1]?.status).toBe("offline");
    expect(messages[1]?.text).toMatch(/core isn't connected/);
  });

  it("recalls the previous command with ArrowUp", () => {
    render(<CommandBar />);
    const input = screen.getByRole<HTMLInputElement>("textbox", { name: "Command" });
    fireEvent.change(input, { target: { value: "cpu" } });
    fireEvent.submit(input);
    fireEvent.keyDown(input, { key: "ArrowUp" });
    expect(input.value).toBe("cpu");
  });
});

describe("IPC client outside the desktop", () => {
  it("refuses calls instead of inventing results", async () => {
    await expect(sershi.getSystemSnapshot()).rejects.toBeInstanceOf(IpcFailure);
  });
});
