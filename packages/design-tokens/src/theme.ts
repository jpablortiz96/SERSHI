import { toCssVariables } from "./css";
import { tokens as baseTokens, type Tokens } from "./tokens";

type DeepPartial<T> = { [K in keyof T]?: T[K] extends string ? string : DeepPartial<T[K]> };

/** A theme is a partial override of the base tokens (see docs/DESIGN_SYSTEM.md#themes). */
export type ThemeOverrides = DeepPartial<Tokens>;

function merge(base: unknown, override: unknown): unknown {
  // Leaves are replaced only by leaves, so malformed theme files (e.g. JSON
  // from a theme package) can never change the token tree's shape.
  if (typeof base === "string") return typeof override === "string" ? override : base;
  if (typeof base !== "object" || base === null || typeof override !== "object" || !override) {
    return base;
  }
  const result: Record<string, unknown> = { ...(base as Record<string, unknown>) };
  for (const [key, value] of Object.entries(override)) {
    // Unknown keys are ignored: themes may only restyle, never invent tokens.
    if (key in result) result[key] = merge(result[key], value);
  }
  return result;
}

export function createTheme(overrides: ThemeOverrides = {}): Tokens {
  // `merge` only replaces existing string leaves, so the result keeps the
  // exact shape of the base tokens.
  return merge(baseTokens, overrides) as Tokens;
}

/**
 * Applies a theme by setting custom properties on `root`. Uses the CSSOM
 * rather than injecting a <style> element, which keeps the app compatible
 * with a strict `style-src 'self'` Content-Security-Policy.
 */
export function applyTheme(root: HTMLElement, theme: Tokens = baseTokens): void {
  for (const [name, value] of toCssVariables(theme)) {
    root.style.setProperty(name, value);
  }
}
