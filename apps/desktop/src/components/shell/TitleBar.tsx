import { useLayoutEffect, useRef, type ComponentType } from "react";

import { useI18n, type MessageKey } from "../../i18n";
import { currentWindow } from "../../ipc";
import { useAssistantStore, type Connection } from "../../state/assistant";
import {
  ActivityIcon,
  CloseIcon,
  HomeIcon,
  Mark,
  MaximizeIcon,
  MinimizeIcon,
  SettingsIcon,
} from "./icons";
import styles from "./TitleBar.module.css";

export type View = "home" | "activity" | "settings";

export const VIEWS: { id: View; label: MessageKey & `nav.${View}`; icon: ComponentType }[] = [
  { id: "home", label: "nav.home", icon: HomeIcon },
  { id: "activity", label: "nav.activity", icon: ActivityIcon },
  { id: "settings", label: "nav.settings", icon: SettingsIcon },
];

const CONNECTION_KEYS: Record<Connection, MessageKey & `connection.${Connection}`> = {
  connecting: "connection.connecting",
  live: "connection.live",
  browserPreview: "connection.browserPreview",
  failed: "connection.failed",
};

interface TitleBarProps {
  view: View;
  onNavigate: (view: View) => void;
  /** Hide the Command Center (SERSHI keeps running). */
  onHide: () => void;
}

/**
 * Custom window chrome. The bar itself is the drag region; controls are
 * ordinary buttons. Closing hides the Command Center — SERSHI stays present
 * in the companion.
 */
export function TitleBar({ view, onNavigate, onHide }: TitleBarProps) {
  const connection = useAssistantStore((s) => s.connection);
  const { t, locale } = useI18n();
  const nav = useRef<HTMLElement>(null);

  // The selection indicator slides to the active tab (tab widths depend on
  // the language, so they are measured).
  useLayoutEffect(() => {
    const el = nav.current;
    const active = el?.querySelector<HTMLElement>('[aria-current="page"]');
    if (!el || !active) return;
    el.style.setProperty("--indicator-x", `${active.offsetLeft}px`);
    el.style.setProperty("--indicator-w", `${active.offsetWidth}px`);
  }, [view, locale]);

  return (
    <header className={styles.bar} data-tauri-drag-region>
      <div className={styles.brand} data-tauri-drag-region>
        <span className={styles.mark}>
          <Mark />
        </span>
        <span className={styles.wordmark}>SERSHI</span>
        <span className={styles.phase}>{t("app.phase")}</span>
      </div>

      <nav ref={nav} className={styles.nav} aria-label={t("nav.label")}>
        <span className={styles.indicator} aria-hidden="true" />
        {VIEWS.map((v, i) => (
          <button
            key={v.id}
            type="button"
            className={styles.tab}
            aria-current={view === v.id ? "page" : undefined}
            aria-keyshortcuts={`Control+${i + 1}`}
            onClick={() => {
              onNavigate(v.id);
            }}
          >
            <v.icon />
            <span>{t(v.label)}</span>
          </button>
        ))}
      </nav>

      <div className={styles.end} data-tauri-drag-region>
        <span className={styles.connection} data-connection={connection}>
          <i aria-hidden="true" />
          {t(CONNECTION_KEYS[connection])}
        </span>
        <div className={styles.controls}>
          <button type="button" aria-label={t("window.minimize")} onClick={currentWindow.minimize}>
            <MinimizeIcon />
          </button>
          <button
            type="button"
            aria-label={t("window.maximize")}
            onClick={currentWindow.toggleMaximize}
          >
            <MaximizeIcon />
          </button>
          <button type="button" aria-label={t("window.hide")} data-variant="close" onClick={onHide}>
            <CloseIcon />
          </button>
        </div>
      </div>
    </header>
  );
}
