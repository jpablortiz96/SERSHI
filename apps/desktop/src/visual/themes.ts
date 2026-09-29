/**
 * Theme registry — the foundation for future visual customisation (v0.8
 * theme packs). A theme is a partial override of the semantic design tokens
 * (`surface`, `text`, `state`, `atmosphere`, `core`, …); components only read
 * tokens, so a theme re-lights SERSHI without touching component logic.
 *
 * Today there is exactly one theme. There is no downloading, no marketplace
 * and no user-supplied theme files; `createTheme` already ignores unknown or
 * malformed keys so future packs cannot change the token tree's shape.
 */
import { createTheme, type ThemeOverrides, type Tokens } from "@sershi/design-tokens";

import type { MessageKey } from "../i18n";

export interface ThemeDefinition {
  id: string;
  label: MessageKey;
  overrides: ThemeOverrides;
}

export const THEMES = [
  { id: "sershi-dark", label: "settings.appearance.themes.sershiDark", overrides: {} },
] as const satisfies readonly ThemeDefinition[];

export type ThemeId = (typeof THEMES)[number]["id"];

export const DEFAULT_THEME: ThemeId = "sershi-dark";

export function themeTokens(id: ThemeId = DEFAULT_THEME): Tokens {
  const themes: readonly ThemeDefinition[] = THEMES;
  const theme = themes.find((t) => t.id === id) ?? THEMES[0];
  return createTheme(theme.overrides);
}
