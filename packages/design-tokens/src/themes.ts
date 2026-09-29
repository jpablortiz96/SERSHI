/**
 * Built-in SERSHI themes, as partial overrides of the semantic tokens.
 *
 * SERSHI Dark is the base token set. SERSHI Light is a separate design,
 * not an inversion: a soft cool off-white environment, dark ink, quieter
 * atmosphere and deeper state hues that stay readable on light surfaces.
 * The core keeps its bright inner light, so it still reads as the room's
 * light source.
 */
import type { ThemeOverrides } from "./theme";

export const darkTheme: ThemeOverrides = {};

export const lightTheme: ThemeOverrides = {
  color: {
    signal: "#0e86ad",
    signalDeep: "#0a6d8f",
  },
  surface: {
    base: "#eceff4",
    raised: "#f3f5f8",
    overlay: "#f8f9fb",
    hover: "rgb(15 23 42 / 0.05)",
    active: "rgb(15 23 42 / 0.08)",
    glass: "rgb(248 249 251 / 0.8)",
    tint: "rgb(15 23 42 / 0.025)",
  },
  text: {
    primary: "#101827",
    secondary: "#3e4a5c",
    tertiary: "#586475",
    disabled: "#9aa3b1",
    inverse: "#f8f9fb",
    accent: "#0a6d8f",
  },
  border: {
    faint: "rgb(15 23 42 / 0.07)",
    subtle: "rgb(15 23 42 / 0.11)",
    default: "rgb(15 23 42 / 0.17)",
    strong: "rgb(15 23 42 / 0.28)",
    focus: "#0a7aa6",
    highlight: "rgb(255 255 255 / 0.7)",
    shade: "rgb(15 23 42 / 0.12)",
  },
  action: {
    primary: "#101827",
    onPrimary: "#f8f9fb",
  },
  state: {
    sleeping: "#7b879a",
    idle: "#2b8fc2",
    awake: "#3d86b8",
    listening: "#0e8fb5",
    transcribing: "#3a73c9",
    thinking: "#6b5bdc",
    planning: "#7b69e2",
    executing: "#2c79e0",
    speaking: "#148c9f",
    success: "#138f6b",
    warning: "#b8741a",
    awaitingConfirmation: "#a8792a",
    error: "#cf3f47",
    offline: "#8a93a3",
    private: "#687385",
    disabled: "#a3acb9",
  },
  atmosphere: {
    primary: "#8b7cf0",
    secondary: "#5a9ae6",
    floor: "#e3e8ef",
    vignette: "rgb(60 72 96 / 0.12)",
    guide: "rgb(15 23 42 / 0.06)",
    grain: "0.035",
  },
  core: {
    specular: "#ffffff",
    inner: "#ffffff",
    orbit: "#7b69e2",
    halo: "0.28",
  },
  shadow: {
    e1: "inset 0 1px 0 rgb(255 255 255 / 0.7), 0 1px 2px rgb(15 23 42 / 0.08)",
    e2: "inset 0 1px 0 rgb(255 255 255 / 0.7), 0 12px 32px -14px rgb(15 23 42 / 0.22)",
    e3: "inset 0 1px 0 rgb(255 255 255 / 0.8), 0 32px 80px -28px rgb(15 23 42 / 0.3)",
  },
};
