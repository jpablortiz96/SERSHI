/**
 * Companion appearances.
 *
 *   AssistantState → companion renderer (props only) → what appears on the desktop
 *
 * A renderer is a pure visual: it receives the state and a few presentation
 * props, and returns pixels. It has no IPC, no business logic and no
 * behaviour of its own — the companion shell (surfaces/companion) handles
 * clicks and drags, and its window capability is the same whatever renders
 * inside it. Changing appearance can never change what SERSHI may do.
 *
 * Orbital is the only renderer today; Settings lists only real renderers.
 * Future appearances (e.g. Aurora, a 2D character, a minimal mark) and
 * downloadable companion packs are documented in docs/VISUAL_EXPERIENCE.md:
 * packs are data (images, sprites, vector art, animation metadata, tokens,
 * audio), never executable code.
 */
import type { AssistantState } from "@sershi/contracts";
import type { ComponentType } from "react";

import { Core } from "../components/core/Core";
import type { MessageKey } from "../i18n";
import type { CompanionAppearance } from "../i18n/preferences";
import type { Intensity } from "./stateVisuals";

/** Everything a companion renderer receives. Presentation only. */
export interface CompanionRendererProps {
  state: AssistantState;
  /** Visible diameter in CSS pixels. */
  size: number;
  intensity: Intensity;
  /** Changes when a one-shot outcome animation should replay. */
  pulseKey: number;
}

export interface CompanionRenderer {
  id: CompanionAppearance;
  label: MessageKey & `settings.appearance.companions.${CompanionAppearance}`;
  component: ComponentType<CompanionRendererProps>;
}

function OrbitalRenderer({ state, size, intensity, pulseKey }: CompanionRendererProps) {
  return (
    <Core
      state={state}
      variant="companion"
      size={size}
      intensity={intensity}
      pulseKey={pulseKey}
      decorative
    />
  );
}

export const COMPANION_RENDERERS: Record<CompanionAppearance, CompanionRenderer> = {
  orbital: {
    id: "orbital",
    label: "settings.appearance.companions.orbital",
    component: OrbitalRenderer,
  },
};

/** The renderer for an appearance id; anything unknown falls back to Orbital. */
export function companionRenderer(id: string): CompanionRenderer {
  const renderers: Record<string, CompanionRenderer | undefined> = COMPANION_RENDERERS;
  return renderers[id] ?? COMPANION_RENDERERS.orbital;
}
