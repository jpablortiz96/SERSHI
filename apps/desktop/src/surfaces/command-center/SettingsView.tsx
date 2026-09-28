import {
  ASSISTANT_STATES,
  type AssistantState,
  type CapabilityStatus,
  type RiskLevel,
} from "@sershi/contracts";

import type { ReactNode } from "react";

import { STATE_COPY } from "../../components/core/stateCopy";
import { desktopRuntime, sershi } from "../../ipc";
import { useAssistantStore } from "../../state/assistant";
import { useRuntimeInfo } from "../../state/runtime";
import styles from "./Page.module.css";

const CAPABILITY_COPY: Record<CapabilityStatus, string> = {
  available: "Available",
  requiresWindowsValidation: "Requires Windows validation",
  planned: "Planned",
  unsupported: "Unsupported",
};

const RISK_COPY: Record<RiskLevel, string> = {
  safe: "Safe",
  sensitive: "Sensitive",
  highRisk: "High risk",
};

const DESKTOP_ONLY = "Available in the desktop app.";

export function SettingsView() {
  const info = useRuntimeInfo();
  const developerMode = info?.developerBuild ?? !desktopRuntime;

  return (
    <div className={styles.page}>
      <header className={styles.header}>
        <h1 className={styles.title}>Settings</h1>
        <p className={styles.lede}>
          What SERSHI can do, what it is allowed to do, and what it keeps. Editable preferences
          arrive with persistent settings in v0.1.
        </p>
      </header>

      <Section title="Privacy">
        <Row label="What you type" value="Never written to the activity log" />
        <Row label="Conversation" value="Kept in memory for this session only" />
        <Row label="Microphone and screen" value="Not used — voice and vision are not built yet" />
        <Row label="Analytics" value="None. SERSHI sends nothing anywhere." />
      </Section>

      <Section title="Tools & permissions">
        {info ? (
          info.tools.map((tool) => (
            <Row
              key={tool.id}
              label={tool.name}
              detail={<span className="t-mono">{tool.id}</span>}
              value={
                <span className={styles.pills}>
                  <Pill tone={tool.risk === "safe" ? "success" : "warning"}>
                    {RISK_COPY[tool.risk]}
                  </Pill>
                  <span className="t-mono">{tool.permissions.join(", ")}</span>
                </span>
              }
            />
          ))
        ) : (
          <p className={styles.empty}>{DESKTOP_ONLY}</p>
        )}
        <p className={styles.footnote}>
          Every tool passes the policy engine before it runs. Only read-only system information is
          allowed by default; anything else asks first.
        </p>
      </Section>

      <Section title="Platform">
        {info ? (
          info.capabilities.map((c) => (
            <Row
              key={c.id}
              label={c.label}
              value={
                <Pill tone={c.status === "available" ? "success" : "neutral"}>
                  {CAPABILITY_COPY[c.status]}
                  {c.milestone ? ` · ${c.milestone}` : ""}
                </Pill>
              }
            />
          ))
        ) : (
          <p className={styles.empty}>{DESKTOP_ONLY}</p>
        )}
      </Section>

      {developerMode && <StatePreview />}

      <Section title="About">
        <Row label="Version" value={info ? `${info.version} · pre-alpha` : "Browser preview"} />
        <Row label="Platform" value={info?.platform ?? "—"} />
        <Row label="License" value="Apache-2.0" />
        {desktopRuntime && (
          <div className={styles.actions}>
            <button
              type="button"
              className={styles.danger}
              onClick={() => {
                sershi.quit().catch(() => undefined);
              }}
            >
              Quit SERSHI
            </button>
          </div>
        )}
      </Section>
    </div>
  );
}

function StatePreview() {
  const serverPreview = useAssistantStore((s) => s.snapshot.previewState);
  const localPreview = useAssistantStore((s) => s.localPreview);
  const setLocalPreview = useAssistantStore((s) => s.setLocalPreview);
  const active = localPreview ?? serverPreview;

  const preview = (state: AssistantState | null) => {
    if (desktopRuntime) sershi.previewState(state).catch(() => undefined);
    else setLocalPreview(state);
  };

  return (
    <Section title="Developer · State preview">
      <p className={styles.footnote}>
        Visual only: previews how every surface renders a state. Behaviour and policy always use the
        real state. Developer builds only.
      </p>
      <div className={styles.stateGrid} role="group" aria-label="Preview assistant state">
        <button
          type="button"
          aria-pressed={active === null}
          onClick={() => {
            preview(null);
          }}
        >
          Live
        </button>
        {ASSISTANT_STATES.map((s) => (
          <button
            key={s}
            type="button"
            data-state={s}
            aria-pressed={active === s}
            onClick={() => {
              preview(s);
            }}
          >
            <i aria-hidden="true" />
            {STATE_COPY[s].label}
          </button>
        ))}
      </div>
    </Section>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className={styles.section} aria-label={title}>
      <h2 className="t-label">{title}</h2>
      <div className={styles.rows}>{children}</div>
    </section>
  );
}

function Row({ label, detail, value }: { label: string; detail?: ReactNode; value: ReactNode }) {
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

function Pill({
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
