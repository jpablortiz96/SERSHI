/**
 * The state → visual mapping. One table decides how every assistant state
 * looks and moves; the Core, the companion and the ambient field all read
 * it, so a state can never be styled differently in two places.
 *
 * State colour comes from the `state.*` design tokens (via `data-state`).
 * Everything else — motion pattern, energy, glyph — comes from here.
 */
import type { AssistantState } from "@sershi/contracts";

/**
 * Motion patterns rendered by the Core (docs/VISUAL_EXPERIENCE.md):
 * - dormant    nothing moves; dimmed
 * - rest       very slow breath, tiny drift — alive but forgettable
 * - attend     core gathers, halo sharpens, orbits align
 * - receive    resonance gathers inward (listening; visual only until voice)
 * - compute    counter-rotating segmented rings, inner light compresses
 * - structure  arcs assemble into ordered geometry, nodes appear, orbits hold
 * - drive      one directional sweep, energy passes outward
 * - resonate   rhythmic outward displacement (speaking; visual only until TTS)
 * - bloom      brief expansion and a soft ring, then settle
 * - caution    slow amber pulse
 * - await      steady amber invitation; nothing spins
 * - falter     core contracts, orbits lose alignment, brief coral ring
 */
export type MotionPattern =
  | "dormant"
  | "rest"
  | "attend"
  | "receive"
  | "compute"
  | "structure"
  | "drive"
  | "resonate"
  | "bloom"
  | "caution"
  | "await"
  | "falter";

/**
 * A shape per state so meaning never depends on colour alone (shown next
 * to the state label and in the state preview).
 */
export type StateGlyph =
  | "hollow"
  | "dot"
  | "focus"
  | "waves"
  | "segments"
  | "nodes"
  | "arrow"
  | "check"
  | "triangle"
  | "hourglass"
  | "cross";

export interface StateVisual {
  pattern: MotionPattern;
  /** Light output, 0–1: halo and bloom strength. */
  energy: number;
  /** SERSHI is doing work; the companion stays visibly active. */
  busy: boolean;
  /**
   * A meaningful event the user should notice (completion, error, approval
   * needed). Idle states never request attention.
   */
  attention: boolean;
  glyph: StateGlyph;
}

export const STATE_VISUALS: Record<AssistantState, StateVisual> = {
  sleeping: { pattern: "dormant", energy: 0.15, busy: false, attention: false, glyph: "hollow" },
  idle: { pattern: "rest", energy: 0.45, busy: false, attention: false, glyph: "dot" },
  awake: { pattern: "attend", energy: 0.7, busy: false, attention: false, glyph: "focus" },
  listening: { pattern: "receive", energy: 0.7, busy: true, attention: false, glyph: "waves" },
  thinking: { pattern: "compute", energy: 0.75, busy: true, attention: false, glyph: "segments" },
  planning: { pattern: "structure", energy: 0.75, busy: true, attention: false, glyph: "nodes" },
  executing: { pattern: "drive", energy: 0.9, busy: true, attention: false, glyph: "arrow" },
  speaking: { pattern: "resonate", energy: 0.75, busy: true, attention: false, glyph: "waves" },
  success: { pattern: "bloom", energy: 0.8, busy: false, attention: true, glyph: "check" },
  warning: { pattern: "caution", energy: 0.7, busy: false, attention: true, glyph: "triangle" },
  awaitingConfirmation: {
    pattern: "await",
    energy: 0.8,
    busy: false,
    attention: true,
    glyph: "hourglass",
  },
  error: { pattern: "falter", energy: 0.6, busy: false, attention: true, glyph: "cross" },
};

export function stateVisual(state: AssistantState): StateVisual {
  return STATE_VISUALS[state];
}

/** How present the companion is (see `companionIntensity`). */
export type Intensity = "calm" | "normal" | "present";

/**
 * Contextual presence: the companion is quieter while the Command Center is
 * on screen (the Command Center carries the conversation) and more present
 * when it is the only sign of SERSHI. Busy states and events that need the
 * user always show at full presence. Uses only SERSHI's own state — nothing
 * about the user's screen or activity.
 */
export function companionIntensity(
  state: AssistantState,
  commandCenterVisible: boolean,
): Intensity {
  const visual = STATE_VISUALS[state];
  if (visual.busy || visual.attention) return "present";
  return commandCenterVisible ? "calm" : "normal";
}
