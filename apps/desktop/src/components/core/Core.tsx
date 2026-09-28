import type { AssistantState } from "@sershi/contracts";
import type { CSSProperties } from "react";

import styles from "./Core.module.css";
import { STATE_COPY } from "./stateCopy";

interface CoreProps {
  state: AssistantState;
  /** Rendered diameter in CSS pixels. */
  size: number;
  /**
   * Changes whenever the core should replay its one-shot outcome animation
   * (success ripple, error contraction). Usually the snapshot revision.
   */
  pulseKey?: number;
  className?: string;
}

/**
 * The Intelligent Core: SERSHI's default companion visual.
 *
 * A pure renderer of assistant state — it holds no state of its own, so any
 * future visual pack can replace it by implementing the same props
 * (docs/MOTION_SYSTEM.md#companion-renderers). Every layer animates only
 * `transform` and `opacity`, which the compositor handles off the main
 * thread; state changes re-tint through the registered `--state-color`.
 */
export function Core({ state, size, pulseKey = 0, className }: CoreProps) {
  return (
    <div
      className={className ? `${styles.core} ${className}` : styles.core}
      data-state={state}
      style={{ "--core-size": `${size}px` } as CSSProperties}
      role="img"
      aria-label={`SERSHI — ${STATE_COPY[state].label}`}
    >
      <div className={styles.halo} />
      <div className={styles.field} />
      <div className={styles.orbitPlane} data-orbit="a">
        <div className={styles.orbit}>
          <span className={styles.satellite} />
        </div>
      </div>
      <div className={styles.orbitPlane} data-orbit="b">
        <div className={styles.orbit} />
      </div>
      <div className={styles.cognition} />
      <div className={styles.drive} />
      <div className={styles.pulses}>
        <i />
        <i />
        <i />
      </div>
      <div className={styles.body}>
        <div className={styles.nucleus} />
      </div>
      <div key={pulseKey} className={styles.flash} />
    </div>
  );
}
