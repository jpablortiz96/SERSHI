/**
 * Settings › Security (Gate 4.1): the configurable permissions as the core
 * reports them. The core is authoritative and stores them; this store only
 * shows them and asks for changes. A less restrictive setting is applied
 * only after the trusted confirmation window approves it — this store
 * cannot apply it, and neither can voice or the Agent Brain.
 */
import type {
  AppPermissionStatus,
  ConfigurablePermission,
  PermissionSetting,
  PermissionStatus,
} from "@sershi/contracts";
import { create } from "zustand";

import { desktopRuntime, sershi } from "../ipc";

export type PermissionNotice = "pending" | "busy" | null;

interface PermissionStore {
  settings: PermissionStatus[] | null;
  /** Per-application close settings (Gate 4.1.1). */
  apps: AppPermissionStatus[];
  /** The application whose change waits in the confirmation window. */
  pendingApp: string | null;
  requestApp: (appId: string, setting: PermissionSetting) => Promise<void>;
  /** The permission whose change waits in the confirmation window. */
  pending: ConfigurablePermission | null;
  notice: PermissionNotice;
  refresh: () => Promise<void>;
  request: (permission: ConfigurablePermission, setting: PermissionSetting) => Promise<void>;
}

export const usePermissions = create<PermissionStore>((set) => ({
  settings: null,
  apps: [],
  pendingApp: null,
  pending: null,
  notice: null,

  refresh: async () => {
    if (!desktopRuntime) return;
    try {
      set({
        settings: await sershi.getPermissionSettings(),
        apps: await sershi.getApplicationPermissions(),
      });
    } catch {
      // Unknown; the section shows nothing to change.
    }
  },

  requestApp: async (appId, setting) => {
    if (!desktopRuntime) return;
    try {
      const change = await sershi.requestAppPermissionChange(appId, setting);
      if (change === "needsConfirmation") {
        set({ pendingApp: appId, notice: "pending" });
      } else {
        set({ pendingApp: null, notice: null, apps: await sershi.getApplicationPermissions() });
      }
    } catch {
      set({ notice: "busy" });
    }
  },

  request: async (permission, setting) => {
    if (!desktopRuntime) return;
    try {
      const change = await sershi.requestPermissionChange(permission, setting);
      if (change === "needsConfirmation") {
        set({ pending: permission, notice: "pending" });
      } else {
        set({ pending: null, notice: null, settings: await sershi.getPermissionSettings() });
      }
    } catch {
      set({ notice: "busy" });
    }
  },
}));

/** Follows decisions made in the confirmation window. Returns cleanup. */
export function connectPermissions(): () => void {
  if (!desktopRuntime) return () => undefined;
  void usePermissions.getState().refresh();
  const stopSettings = sershi.onPermissions((settings) => {
    usePermissions.setState({ settings, pending: null, notice: null });
  });
  const stopApps = sershi.onAppPermissions((apps) => {
    usePermissions.setState({ apps, pendingApp: null, notice: null });
  });
  return () => {
    stopSettings();
    stopApps();
  };
}
