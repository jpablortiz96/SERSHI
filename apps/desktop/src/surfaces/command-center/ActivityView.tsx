import { KIND_TONE } from "../../components/activity/ActivityItem";
import { formatClock } from "../../lib/format";
import { useActivity } from "../../state/activity";
import styles from "./Page.module.css";

export function ActivityView() {
  const entries = useActivity((s) => s.entries);

  return (
    <div className={styles.page}>
      <header className={styles.header}>
        <h1 className={styles.title}>Activity</h1>
        <p className={styles.lede}>
          Every action SERSHI takes is recorded here, including what policy blocked. What you type
          and the contents of your files are never written to this log.
        </p>
      </header>

      {entries.length === 0 ? (
        <p className={styles.empty}>No activity in this session yet.</p>
      ) : (
        <table className={styles.table}>
          <thead>
            <tr>
              <th scope="col">Time</th>
              <th scope="col">Event</th>
              <th scope="col">Tool</th>
              <th scope="col" className={styles.num}>
                Duration
              </th>
            </tr>
          </thead>
          <tbody>
            {entries.map((e) => (
              <tr key={e.id} data-tone={KIND_TONE[e.kind]}>
                <td className="t-mono">{formatClock(e.atMs)}</td>
                <td>
                  <i className={styles.tone} aria-hidden="true" />
                  {e.summary}
                </td>
                <td className="t-mono">{e.toolId ?? "—"}</td>
                <td className={`t-mono ${styles.num}`}>
                  {e.durationMs != null ? `${e.durationMs} ms` : "—"}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      <p className={styles.footnote}>
        Activity is kept in memory for this session. Persistent, exportable history arrives with
        local storage in v0.1.
      </p>
    </div>
  );
}
