/**
 * The only module that talks to Tauri. Everything else imports the typed
 * `sershi` facade from `src/ipc`; ESLint forbids importing @tauri-apps/api
 * anywhere else, so the UI cannot grow ad-hoc privileged calls.
 */
import {
  COMMAND_NAMES,
  type CommandMap,
  type CommandName,
  type EventMap,
  type EventName,
  type IpcError,
} from "@sershi/contracts";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** True inside the SERSHI desktop shell; false in a plain browser (design preview). */
export const desktopRuntime: boolean = isTauri();

/** A failed IPC call, with a message that is safe to display. */
export class IpcFailure extends Error {
  constructor(
    readonly code: IpcError["code"] | "notConnected" | "invalidResponse",
    message: string,
  ) {
    super(message);
    this.name = "IpcFailure";
  }
}

function isIpcError(value: unknown): value is IpcError {
  return (
    typeof value === "object" &&
    value !== null &&
    "code" in value &&
    "message" in value &&
    typeof value.message === "string"
  );
}

type Guard<T> = (value: unknown) => value is T;

export async function call<C extends CommandName>(
  command: C,
  args: CommandMap[C]["args"],
  guard?: Guard<CommandMap[C]["result"]>,
): Promise<CommandMap[C]["result"]> {
  if (!desktopRuntime) {
    throw new IpcFailure("notConnected", "The SERSHI desktop runtime isn't connected.");
  }
  if (!(COMMAND_NAMES as readonly string[]).includes(command)) {
    throw new IpcFailure("notAllowed", `Unknown command ${command}.`);
  }
  let result: unknown;
  try {
    result = await invoke(command, args);
  } catch (error) {
    if (isIpcError(error)) throw new IpcFailure(error.code, error.message);
    throw new IpcFailure("internal", "SERSHI's core didn't respond.");
  }
  if (guard && !guard(result)) {
    throw new IpcFailure("invalidResponse", `Unexpected response from ${command}.`);
  }
  // Commands without a guard return `null` or static data whose shape is fixed
  // by the Rust signature and the generated contract types.
  return result as CommandMap[C]["result"];
}

/**
 * Subscribes to a core event. Payloads failing `guard` are dropped rather than
 * rendered. Returns an unsubscribe function (a no-op outside the desktop).
 */
export function subscribe<E extends EventName>(
  event: E,
  guard: Guard<EventMap[E]>,
  handler: (payload: EventMap[E]) => void,
): () => void {
  if (!desktopRuntime) return () => undefined;
  let disposed = false;
  let unlisten: (() => void) | undefined;
  listen<unknown>(event, ({ payload }) => {
    if (guard(payload)) handler(payload);
  })
    .then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    })
    .catch(() => undefined);
  return () => {
    disposed = true;
    unlisten?.();
  };
}
