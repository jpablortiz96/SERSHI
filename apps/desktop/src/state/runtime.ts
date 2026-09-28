import type { RuntimeInfo } from "@sershi/contracts";
import { useEffect, useState } from "react";

import { desktopRuntime, sershi } from "../ipc";

let cached: Promise<RuntimeInfo> | null = null;

/** Static runtime facts (version, platform, capabilities, tools). Fetched once. */
export function useRuntimeInfo(): RuntimeInfo | null {
  const [info, setInfo] = useState<RuntimeInfo | null>(null);
  useEffect(() => {
    if (!desktopRuntime) return;
    cached ??= sershi.getRuntimeInfo();
    let active = true;
    cached
      .then((value) => {
        if (active) setInfo(value);
      })
      .catch(() => {
        cached = null;
      });
    return () => {
      active = false;
    };
  }, []);
  return info;
}
