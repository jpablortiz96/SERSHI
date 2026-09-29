import styles from "./Ambient.module.css";

/**
 * The Command Center's atmosphere: a dark observatory lit by the core.
 * Layers: the core's light (state-tinted, local), an off-centre drift of the
 * theme's atmosphere light, a faint floor, orbital guides, grain and a
 * vignette. Purely decorative; one composited transform loop.
 */
export function Ambient() {
  return (
    <div className={styles.ambient} aria-hidden="true">
      <div className={styles.light} />
      <div className={styles.drift} />
      <div className={styles.floor} />
      <div className={styles.guides}>
        <i />
        <i />
        <i />
      </div>
      <div className={styles.grain} />
      <div className={styles.vignette} />
    </div>
  );
}
