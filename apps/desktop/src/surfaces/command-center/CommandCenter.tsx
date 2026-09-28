import { useEffect, useState } from "react";

import { Ambient } from "../../components/shell/Ambient";
import { TitleBar, VIEWS, type View } from "../../components/shell/TitleBar";
import { useI18n } from "../../i18n";
import { desktopRuntime, sershi } from "../../ipc";
import { connectActivity } from "../../state/activity";
import { connectAssistant, useDisplayState } from "../../state/assistant";
import { useConversation } from "../../state/conversation";
import { ActivityView } from "./ActivityView";
import styles from "./CommandCenter.module.css";
import { HomeView } from "./HomeView";
import { SettingsView } from "./SettingsView";

export function CommandCenter() {
  const [view, setView] = useState<View>("home");
  const state = useDisplayState();
  const { t } = useI18n();

  useEffect(() => {
    const disconnectAssistant = connectAssistant();
    const disconnectActivity = connectActivity();
    // Approvals happen in the trusted confirmation window; their outcomes
    // (approved, cancelled, expired) arrive here for the transcript.
    const stopOutcomes = sershi.onCommandOutcome((outcome) => {
      useConversation.getState().addReply({ kind: "outcome", outcome });
    });
    // Summon (companion, tray, shortcut) always lands on the command input.
    const stopFocus = sershi.onFocusCommand(() => {
      setView("home");
    });
    return () => {
      disconnectAssistant();
      disconnectActivity();
      stopFocus();
      stopOutcomes();
    };
  }, []);

  // The tray menu follows the interface language.
  useEffect(() => {
    if (!desktopRuntime) return;
    sershi
      .setTrayLabels({ open: t("tray.open"), hide: t("tray.hide"), quit: t("tray.quit") })
      .catch(() => undefined);
  }, [t]);

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
