import type { AssistantState } from "@sershi/contracts";
import type { CSSProperties } from "react";

import { useI18n } from "../../i18n";
import { stateLabel } from "../../i18n/domain";
import { stateVisual, type Intensity } from "../../visual/stateVisuals";
import styles from "./Core.module.css";

/**
 * - hero       the Command Center's stage (full detail, casts light)
 * - companion  the floating companion (full detail, contained glow)
 * - compact    inline marks (e.g. the confirmation window header)
 * - preview    static thumbnails; nothing animates
 */
export type CoreVariant = "hero" | "companion" | "compact" | "preview";

const DEFAULT_SIZE: Record<CoreVariant, number> = {
  hero: 232,
  companion: 104,
  compact: 22,
  preview: 56,
};

interface CoreProps {
  state: AssistantState;
  variant?: CoreVariant;
  /** Diameter in CSS pixels; defaults per variant. */
  size?: number;
  /** Contextual presence (companion); see `companionIntensity`. */
  intensity?: Intensity;
  /**
   * Changes whenever the core should replay its one-shot outcome animation
   * (success bloom, error falter). Usually the snapshot revision.
   */
  pulseKey?: number;
  /** Decorative only (a visible label already names the state). */
  decorative?: boolean;
  className?: string;
}

/**
 * The Intelligent Core: SERSHI's presence. A pure renderer of assistant
 * state — it holds no state of its own. How each state looks and moves comes
 * from `visual/stateVisuals.ts`; this component only maps it onto layers.
 * Every layer animates `transform`/`translate` and `opacity` only, which the
 * compositor handles off the main thread.
 */
export function Core({
  state,
  variant = "hero",
  size,
  intensity = "normal",
  pulseKey = 0,
  decorative = false,
  className,
}: CoreProps) {
  const { t } = useI18n();
  const visual = stateVisual(state);
  const detailed = variant === "hero" || variant === "companion";
  return (
    <div
      className={className ? `${styles.core} ${className}` : styles.core}
      data-state={state}
      data-pattern={visual.pattern}
      data-variant={variant}
      data-intensity={intensity}
      style={
        {
          "--core-size": `${size ?? DEFAULT_SIZE[variant]}px`,
          "--energy": visual.energy,
        } as CSSProperties
      }
      {...(decorative
        ? { "aria-hidden": true }
        : { role: "img", "aria-label": t("core.label", { state: stateLabel(t, state) }) })}
    >
      {variant === "hero" && <div className={styles.bloom} />}
      <div className={styles.halo} />
      <div className={styles.field} />
      {detailed && (
        <>
          <div className={styles.orbitPlane} data-orbit="a">
            <div className={styles.orbit}>
              <span className={styles.satellite} />
            </div>
          </div>
          <div className={styles.orbitPlane} data-orbit="b">
            <div className={styles.orbit} />
          </div>
          <div className={styles.compute}>
            <i />
            <i />
          </div>
          <div className={styles.structure}>
            <i />
            <i />
            <i />
            <i />
          </div>
          <div className={styles.drive} />
          <div className={styles.resonance}>
            <i />
            <i />
            <i />
          </div>
          <div className={styles.motes}>
            <i />
            <i />
            <i />
          </div>
        </>
      )}
      <div className={styles.shell} />
      <div className={styles.body}>
        <div className={styles.nucleus} />
        <div className={styles.specular} />
      </div>
      {detailed && <div key={pulseKey} className={styles.flash} />}
    </div>
  );
}
