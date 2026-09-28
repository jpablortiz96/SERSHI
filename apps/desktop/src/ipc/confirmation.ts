/**
 * IPC for the trusted confirmation surface only. Deliberately not part of the
 * `sershi` facade: no other surface imports this module (enforced by
 * test/confirmation-surface.test.tsx), and the shell grants these two
 * commands to the confirmation window alone (capabilities/confirmation.json).
 *
 * The decision carries the confirmation id and the human's choice — nothing
 * that could name a tool, target, input or risk.
 */
import { isConfirmationRequest, isNullPayload, type ConfirmationChoice } from "@sershi/contracts";

import { call } from "./client";

export { desktopRuntime } from "./client";

export const confirmationSurface = {
  /** The confirmation this window was opened for. */
  getContext: () => call("get_confirmation_context", {}, isConfirmationRequest),
  decide: (confirmationId: string, decision: ConfirmationChoice) =>
    call("decide_confirmation", { decision: { confirmationId, decision } }, isNullPayload),
};
