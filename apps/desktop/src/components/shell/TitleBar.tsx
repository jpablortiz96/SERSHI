import { currentWindow } from "../../ipc";
import { useAssistantStore, type Connection } from "../../state/assistant";
import { CloseIcon, Mark, MaximizeIcon, MinimizeIcon } from "./icons";
import styles from "./TitleBar.module.css";

export type View = "home" | "activity" | "settings";

export const VIEWS: { id: View; label: string }[] = [
  { id: "home", label: "Home" },
  { id: "activity", label: "Activity" },
  { id: "settings", label: "Settings" },
];

const CONNECTION_COPY: Record<Connection, string> = {
  connecting: "Connecting to core",
  live: "Local core",
  browserPreview: "Browser preview",
  failed: "Core unavailable",
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

  return (
    <header className={styles.bar} data-tauri-drag-region>
      <div className={styles.brand} data-tauri-drag-region>
        <span className={styles.mark}>
          <Mark />
        </span>
        <span className={styles.wordmark}>SERSHI</span>
        <span className={styles.phase}>Pre-alpha</span>
      </div>

      <nav className={styles.nav} aria-label="Command Center">
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
            {v.label}
          </button>
        ))}
      </nav>

      <div className={styles.end} data-tauri-drag-region>
        <span className={styles.connection} data-connection={connection}>
          <i aria-hidden="true" />
          {CONNECTION_COPY[connection]}
        </span>
        <div className={styles.controls}>
          <button type="button" aria-label="Minimize" onClick={currentWindow.minimize}>
            <MinimizeIcon />
          </button>
          <button type="button" aria-label="Maximize" onClick={currentWindow.toggleMaximize}>
            <MaximizeIcon />
          </button>
          <button
            type="button"
            aria-label="Hide Command Center"
            data-variant="close"
            onClick={currentWindow.hide}
          >
            <CloseIcon />
          </button>
        </div>
      </div>
    </header>
  );
}
