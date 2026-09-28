import type { ActivityEntry } from "@sershi/contracts";
import { create } from "zustand";

import { desktopRuntime, sershi } from "../ipc";

const PAGE = 100;

interface ActivityStore {
  entries: ActivityEntry[];
  add: (entry: ActivityEntry) => void;
  replace: (entries: ActivityEntry[]) => void;
}

/** Newest first, de-duplicated by id. */
export const useActivity = create<ActivityStore>((set) => ({
  entries: [],
  add: (entry) => {
    set((s) =>
      s.entries.some((e) => e.id === entry.id)
        ? s
        : { entries: [entry, ...s.entries].slice(0, PAGE) },
    );
  },
  replace: (entries) => {
    set((s) => {
      const merged = new Map([...entries, ...s.entries].map((e) => [e.id, e]));
      return { entries: [...merged.values()].sort((a, b) => b.id - a.id).slice(0, PAGE) };
    });
  },
}));

export function connectActivity(): () => void {
  if (!desktopRuntime) return () => undefined;
  const { add, replace } = useActivity.getState();
  const unsubscribe = sershi.onActivity(add);
  sershi
    .listActivity(PAGE)
    .then(replace)
    .catch(() => undefined);
  return unsubscribe;
}
