import { useI18n, type MessageKey } from "../../i18n";
import { currentWindow, sershi } from "../../ipc";
import { useAssistantStore, type Connection } from "../../state/assistant";
import { CloseIcon, Mark, MaximizeIcon, MinimizeIcon } from "./icons";
import styles from "./TitleBar.module.css";

export type View = "home" | "activity" | "settings";

export const VIEWS: { id: View; label: MessageKey & `nav.${View}` }[] = [
  { id: "home", label: "nav.home" },
  { id: "activity", label: "nav.activity" },
  { id: "settings", label: "nav.settings" },
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
}

/**
 * Custom window chrome. The bar itself is the drag region; controls are
 * ordinary buttons. Closing hides the Command Center — SERSHI stays present
 * in the companion.
 */
export function TitleBar({ view, onNavigate }: TitleBarProps) {
  const connection = useAssistantStore((s) => s.connection);
  const { t } = useI18n();

  return (
    <header className={styles.bar} data-tauri-drag-region>
      <div className={styles.brand} data-tauri-drag-region>
        <span className={styles.mark}>
          <Mark />
        </span>
        <span className={styles.wordmark}>SERSHI</span>
        <span className={styles.phase}>{t("app.phase")}</span>
      </div>

      <nav className={styles.nav} aria-label={t("nav.label")}>
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
            {t(v.label)}
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
          <button
            type="button"
            aria-label={t("window.hide")}
            data-variant="close"
            onClick={() => {
              // Rust hides the window and cancels any pending approval;
              // SERSHI keeps running in the companion and tray.
              sershi.hideCommandCenter().catch(() => undefined);
            }}
          >
            <CloseIcon />
          </button>
        </div>
      </div>
    </header>
  );
}
