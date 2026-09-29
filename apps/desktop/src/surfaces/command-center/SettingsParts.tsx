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

interface ChoicesProps<T extends string> {
  name: string;
  label: string;
  detail: string;
  options: readonly T[];
  value: T;
  onChange: (value: T) => void;
  optionLabel: (value: T) => string;
  optionDetail?: (value: T) => string | undefined;
}

export function Choices<T extends string>({
  name,
  label,
  detail,
  options,
  value,
  onChange,
  optionLabel,
  optionDetail,
}: ChoicesProps<T>) {
  const labelId = `${name}-label`;
  return (
    <div className={styles.rowStacked}>
      <div>
        <p className={styles.rowLabel} id={labelId}>
          {label}
        </p>
        <p className={styles.rowDetail}>{detail}</p>
      </div>
      <div className={styles.choices} role="radiogroup" aria-labelledby={labelId}>
        {options.map((option) => {
          const extra = optionDetail?.(option);
          return (
            <label key={option} className={styles.choice}>
              <input
                type="radio"
                name={name}
                value={option}
                checked={value === option}
                onChange={() => {
                  onChange(option);
                }}
              />
              <span className={styles.choiceText}>
                <span>{optionLabel(option)}</span>
                {extra && <span className={styles.choiceDetail}>{extra}</span>}
              </span>
            </label>
          );
        })}
      </div>
    </div>
  );
}
