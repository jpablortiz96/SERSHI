import { useEffect, useState } from "react";

import { Ambient } from "../../components/shell/Ambient";
import { TitleBar, VIEWS, type View } from "../../components/shell/TitleBar";
import { connectActivity } from "../../state/activity";
import { connectAssistant, useDisplayState } from "../../state/assistant";
import { ActivityView } from "./ActivityView";
import styles from "./CommandCenter.module.css";
import { HomeView } from "./HomeView";
import { SettingsView } from "./SettingsView";

export function CommandCenter() {
  const [view, setView] = useState<View>("home");
  const state = useDisplayState();

  useEffect(() => {
    const disconnectAssistant = connectAssistant();
    const disconnectActivity = connectActivity();
    return () => {
      disconnectAssistant();
      disconnectActivity();
    };
  }, []);

  // Ctrl+1…3 switches views.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const index = Number(e.key) - 1;
      const target = VIEWS[index];
      if (e.ctrlKey && !e.altKey && target) {
        e.preventDefault();
        setView(target.id);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
    };
  }, []);

  return (
    <div className={styles.app} data-state={state}>
      <Ambient />
      <TitleBar view={view} onNavigate={setView} />
      <main key={view} className={styles.view}>
        {view === "home" && (
          <HomeView
            onViewActivity={() => {
              setView("activity");
            }}
          />
        )}
        {view === "activity" && <ActivityView />}
        {view === "settings" && <SettingsView />}
      </main>
    </div>
  );
}
