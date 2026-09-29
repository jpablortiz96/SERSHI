import "@fontsource-variable/geist";
import "@fontsource-variable/geist-mono";
import "../styles/global.css";

import { applyTheme } from "@sershi/design-tokens";
import { StrictMode, type ReactNode } from "react";
import { createRoot } from "react-dom/client";

import { connectLocale } from "../i18n";
// The low-level client only: the trusted confirmation window shares this
// bootstrap and must not bundle the Command Center's IPC facade (voice
// included).
import { desktopRuntime } from "../ipc/client";
import { connectAppearance } from "../visual/appearance";

/** Shared start-up for every SERSHI window: tokens first, then React. */
export function mount(app: ReactNode): void {
  const html = document.documentElement;
  // Base tokens first so nothing renders unstyled; connectAppearance then
  // applies the chosen theme.
  applyTheme(html);
  if (!desktopRuntime) html.dataset.runtime = "browser";
  // Live for the whole window lifetime; no cleanup needed.
  connectLocale();
  connectAppearance();

  const root = document.getElementById("root");
  if (!root) throw new Error("SERSHI root element missing");
  createRoot(root).render(<StrictMode>{app}</StrictMode>);
}
