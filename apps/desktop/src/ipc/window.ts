/** Window chrome actions for the current window. Permissions are scoped per window in src-tauri/capabilities. */
import { getCurrentWindow } from "@tauri-apps/api/window";

import { desktopRuntime } from "./client";

function run(action: () => Promise<void>): void {
  if (desktopRuntime) action().catch(() => undefined);
}

export const currentWindow = {
  startDragging: () => {
    run(() => getCurrentWindow().startDragging());
  },
  minimize: () => {
    run(() => getCurrentWindow().minimize());
  },
  toggleMaximize: () => {
    run(() => getCurrentWindow().toggleMaximize());
  },
};
