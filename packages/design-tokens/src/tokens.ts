/**
 * SERSHI design tokens — the single source of truth for every visual value.
 *
 * Components never hard-code colors, sizes or timings: they read the CSS
 * custom properties generated from this object (see `css.ts`). Themes (v0.8)
 * override a subset of these values at runtime.
 *
 * Rationale for each value lives in docs/DESIGN_SYSTEM.md.
 */

export const tokens = {
  /** Raw palette. Prefer the semantic groups below in components. */
  color: {
    ink0: "#040507",
    ink1: "#07080b",
    ink2: "#0b0d12",
    ink3: "#10131a",
    ink4: "#161a23",
    ink5: "#1e2330",
    frost: "#eef3f8",
    ice: "#cfefff",
    signal: "#56d4f5",
    signalDeep: "#1fa6d6",
    volt: "#5aa9ff",
    aura: "#9a8cff",
    auraSoft: "#b7a9ff",
    mint: "#5fe3b0",
    ember: "#f2b870",
    coral: "#f2777a",
    slate: "#5c6a80",
  },

  surface: {
    /** Window background — the deepest layer. */
    base: "#07080b",
    /** Rails, panels and the stage floor. */
    raised: "#0b0d12",
    /** Inputs, popovers and interactive wells. */
    overlay: "#10131a",
    /** Hover wash for interactive surfaces. */
    hover: "rgb(255 255 255 / 0.04)",
    /** Pressed / selected wash. */
    active: "rgb(255 255 255 / 0.07)",
    /** Controlled translucency; the only glass in the system. */
    glass: "rgb(16 19 26 / 0.72)",
    /** Barely-there fill for chips and quiet controls. */
    tint: "rgb(255 255 255 / 0.02)",
  },

  text: {
    primary: "#eef3f8",
    secondary: "#a3adbd",
    tertiary: "#767f91",
    disabled: "#454c5a",
    inverse: "#07080b",
    accent: "#9fe3fb",
  },

  border: {
    faint: "rgb(255 255 255 / 0.05)",
    subtle: "rgb(255 255 255 / 0.08)",
    default: "rgb(255 255 255 / 0.12)",
    strong: "rgb(255 255 255 / 0.2)",
    focus: "#56d4f5",
    /** Top-edge light on raised controls. */
    highlight: "rgb(255 255 255 / 0.06)",
    /** Bottom-edge shade on key caps and pressed controls. */
    shade: "rgb(0 0 0 / 0.4)",
  },

  /** The primary action fill (send, approve) and the text on it. */
  action: {
    primary: "#eef3f8",
    onPrimary: "#07080b",
  },

  /**
   * One hue per assistant state. The companion, the ambient light and every
   * status indicator derive from these, so a state reads the same everywhere.
   */
  state: {
    sleeping: "#5c6a80",
    idle: "#9fdfff",
    awake: "#cfefff",
    listening: "#56d4f5",
    /** Recognising captured speech (the microphone is already off). */
    transcribing: "#78b4ff",
    thinking: "#9a8cff",
    planning: "#b7a9ff",
    executing: "#5aa9ff",
    speaking: "#7fe6f2",
    success: "#5fe3b0",
    warning: "#f2b870",
    /** Waiting for the user's approval: calmer and paler than warning. */
    awaitingConfirmation: "#ecc98a",
    error: "#f2777a",
    offline: "#6b7486",
    private: "#c9d2de",
    disabled: "#454c5a",
  },

  /**
   * Themeable atmosphere: the light around SERSHI. The Command Center's
   * ambient field and the core's bloom read these, never the raw palette, so
   * a theme can re-light SERSHI without touching components.
   */
  atmosphere: {
    /** Slow off-centre glow in the background. */
    primary: "#9a8cff",
    /** Secondary cool light, used sparingly for depth. */
    secondary: "#5aa9ff",
    /** Observatory floor under the stage. */
    floor: "#0b0d12",
    /** Edge darkening. */
    vignette: "rgb(0 0 0 / 0.55)",
    /** Orbital guide rings around the stage. */
    guide: "rgb(255 255 255 / 0.035)",
    /** Film grain strength (opacity). */
    grain: "0.05",
  },

  /**
   * Themeable core materials. State colour still comes from `state.*`; these
   * define the light the core is made of.
   */
  core: {
    /** Brightest point of the core. */
    specular: "#ffffff",
    /** Inner light the state colour mixes towards. */
    inner: "#eef3f8",
    /** Secondary orbit tint. */
    orbit: "#9a8cff",
    /** Halo strength (0–1) at normal intensity. */
    halo: "0.34",
  },

  radius: {
    xs: "4px",
    sm: "8px",
    md: "12px",
    lg: "16px",
    xl: "22px",
    "2xl": "28px",
    pill: "999px",
  },

  /** 4px base grid. */
  space: {
    "0": "0px",
    "0-5": "2px",
    "1": "4px",
    "2": "8px",
    "3": "12px",
    "4": "16px",
    "5": "20px",
    "6": "24px",
    "8": "32px",
    "10": "40px",
    "12": "48px",
    "16": "64px",
    "20": "80px",
  },

  shadow: {
    /** Hairline top light + tight contact shadow. */
    e1: "inset 0 1px 0 rgb(255 255 255 / 0.04), 0 1px 2px rgb(0 0 0 / 0.4)",
    e2: "inset 0 1px 0 rgb(255 255 255 / 0.05), 0 12px 32px -12px rgb(0 0 0 / 0.7)",
    e3: "inset 0 1px 0 rgb(255 255 255 / 0.06), 0 32px 80px -24px rgb(0 0 0 / 0.8)",
  },

  blur: {
    sm: "8px",
    md: "16px",
    lg: "28px",
  },

  opacity: {
    faint: "0.04",
    subtle: "0.08",
    muted: "0.16",
    medium: "0.32",
    strong: "0.56",
    disabled: "0.4",
  },

  font: {
    display: '"Geist Variable", "Segoe UI Variable Display", "Segoe UI", system-ui, sans-serif',
    ui: '"Geist Variable", "Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif',
    mono: '"Geist Mono Variable", "Cascadia Mono", Consolas, ui-monospace, monospace',
  },

  /** Type scale: size / line-height / weight / tracking. */
  type: {
    hero: { size: "44px", line: "48px", weight: "300", tracking: "-0.035em" },
    display: { size: "30px", line: "36px", weight: "320", tracking: "-0.025em" },
    title: { size: "20px", line: "26px", weight: "480", tracking: "-0.012em" },
    heading: { size: "15px", line: "22px", weight: "560", tracking: "-0.005em" },
    body: { size: "14px", line: "21px", weight: "400", tracking: "0em" },
    small: { size: "13px", line: "19px", weight: "420", tracking: "0.002em" },
    caption: { size: "12px", line: "16px", weight: "450", tracking: "0.01em" },
    label: { size: "10.5px", line: "14px", weight: "600", tracking: "0.14em" },
    mono: { size: "12px", line: "18px", weight: "420", tracking: "0em" },
    metric: { size: "26px", line: "28px", weight: "300", tracking: "-0.03em" },
  },

  motion: {
    duration: {
      instant: "80ms",
      fast: "160ms",
      default: "240ms",
      slow: "420ms",
      cinematic: "720ms",
      /** Window arriving (summon, open). */
      window: "260ms",
      /** Window leaving before the native hide. */
      windowExit: "160ms",
      /** Ambient loops: breathing, drift. */
      breath: "5600ms",
      /** Idle life: very slow, barely noticeable. */
      ambient: "9600ms",
      drift: "28000ms",
    },
    ease: {
      /** General UI movement. */
      standard: "cubic-bezier(0.2, 0, 0, 1)",
      /** Elements arriving: fast start, long settle. */
      enter: "cubic-bezier(0.16, 1, 0.3, 1)",
      /** Elements leaving: gentle start, quick finish. */
      exit: "cubic-bezier(0.4, 0, 1, 1)",
      /** Soft overshoot for confirmations (success ripple, press release). */
      spring: "cubic-bezier(0.34, 1.4, 0.64, 1)",
      /** Symmetric sine for ambient loops. */
      breathe: "cubic-bezier(0.45, 0, 0.55, 1)",
      linear: "linear",
    },
  },

  z: {
    base: "0",
    raised: "10",
    sticky: "100",
    overlay: "1000",
    modal: "1100",
    toast: "1200",
    tooltip: "1300",
  },

  layout: {
    titlebar: "44px",
    rail: "264px",
    /** Rails narrow on smaller windows (see Command Center breakpoints). */
    railCompact: "224px",
    stageMax: "640px",
    readable: "68ch",
  },
} as const;

export type Tokens = typeof tokens;
export type StateToken = keyof Tokens["state"];
