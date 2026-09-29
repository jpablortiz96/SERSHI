/**
 * Theme registry — SERSHI Dark and SERSHI Light, each a partial override of
 * the semantic design tokens (`surface`, `text`, `border`, `action`, `state`,
 * `atmosphere`, `core`, `shadow`). Components only read tokens, so switching
 * theme re-lights SERSHI without any component knowing which theme is active.
 *
 * Future theme packs will be data (token overrides, images), never code:
 * `createTheme` ignores unknown or malformed keys so a pack cannot change the
 * token tree's shape. See docs/VISUAL_EXPERIENCE.md#themes-and-packs.
 */
import {
  createTheme,
  darkTheme,
  lightTheme,
  type ThemeOverrides,
  type Tokens,
} from "@sershi/design-tokens";

import type { MessageKey } from "../i18n";
import type { ThemePreference } from "../i18n/preferences";

export type ThemeId = "dark" | "light";

export interface ThemeDefinition {
  id: ThemeId;
  label: MessageKey & `settings.appearance.themes.${ThemeId}`;
  overrides: ThemeOverrides;
}

export const THEMES: Record<ThemeId, ThemeDefinition> = {
  dark: { id: "dark", label: "settings.appearance.themes.dark", overrides: darkTheme },
  light: { id: "light", label: "settings.appearance.themes.light", overrides: lightTheme },
};

/** Which theme a preference shows, given the system's light/dark mode. */
export function resolveTheme(preference: ThemePreference, systemDark: boolean): ThemeId {
  if (preference === "system") return systemDark ? "dark" : "light";
  return preference;
}

export function themeTokens(id: ThemeId = "dark"): Tokens {
  return createTheme(THEMES[id].overrides);
}
