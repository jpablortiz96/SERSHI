import type { ReactNode } from "react";

import styles from "./Page.module.css";

export function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className={styles.section} aria-label={title}>
      <h2 className="t-label">{title}</h2>
      <div className={styles.rows}>{children}</div>
    </section>
  );
}

export function Row({
  label,
  detail,
  value,
}: {
  label: string;
  detail?: ReactNode;
  value: ReactNode;
}) {
  return (
    <div className={styles.row}>
      <div>
        <p className={styles.rowLabel}>{label}</p>
        {detail && <p className={styles.rowDetail}>{detail}</p>}
      </div>
      <div className={styles.rowValue}>{value}</div>
    </div>
  );
}

export function Pill({
  tone,
  children,
}: {
  tone: "success" | "warning" | "neutral";
  children: ReactNode;
}) {
  return (
    <span className={styles.pill} data-tone={tone}>
      {children}
    </span>
  );
}
