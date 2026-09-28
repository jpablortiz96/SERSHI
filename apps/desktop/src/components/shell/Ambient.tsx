import styles from "./Ambient.module.css";

/**
 * The Command Center's ambient layer: state-tinted light, a slow drift, fixed
 * orbital guides around the stage and a whisper of grain. Purely decorative;
 * GPU cost is one composited transform loop.
 */
export function Ambient() {
  return (
    <div className={styles.ambient} aria-hidden="true">
      <div className={styles.light} />
      <div className={styles.drift} />
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
