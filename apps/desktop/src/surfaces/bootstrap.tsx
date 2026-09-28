import "@fontsource-variable/geist";
import "@fontsource-variable/geist-mono";
import "../styles/global.css";

import { applyTheme } from "@sershi/design-tokens";
import { StrictMode, type ReactNode } from "react";
import { createRoot } from "react-dom/client";

import { desktopRuntime } from "../ipc";

/** Shared start-up for every SERSHI window: tokens first, then React. */
export function mount(app: ReactNode): void {
  const html = document.documentElement;
  applyTheme(html);
  if (!desktopRuntime) html.dataset.runtime = "browser";

  const root = document.getElementById("root");
  if (!root) throw new Error("SERSHI root element missing");
  createRoot(root).render(<StrictMode>{app}</StrictMode>);
}
