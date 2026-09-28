import type { Tokens } from "./tokens";

/**
 * Naming: nested keys join with "-", camelCase becomes kebab-case.
 *   color.signalDeep        → --color-signal-deep
 *   type.hero.size          → --type-hero-size
 *   motion.duration.fast    → --duration-fast   (motion.* is flattened)
 *   motion.ease.enter       → --ease-enter
 */
const kebab = (key: string) => key.replace(/([a-z0-9])([A-Z])/g, "$1-$2").toLowerCase();

type TokenTree = { readonly [key: string]: string | TokenTree };

function flatten(tree: TokenTree, prefix: string[], out: Map<string, string>): void {
  for (const [key, value] of Object.entries(tree)) {
    const path = [...prefix, kebab(key)];
    if (typeof value === "string") {
      const name = `--${(path[0] === "motion" ? path.slice(1) : path).join("-")}`;
      if (out.has(name)) {
        throw new Error(`Duplicate design token variable ${name}`);
      }
      out.set(name, value);
    } else {
      flatten(value, path, out);
    }
  }
}

/** Converts a token tree into CSS custom properties. */
export function toCssVariables(tokens: Tokens): Map<string, string> {
  const out = new Map<string, string>();
  flatten(tokens, [], out);
  return out;
}

/** Renders variables as a CSS rule (for docs, tests and static export). */
export function toCssText(tokens: Tokens, selector = ":root"): string {
  const lines = [...toCssVariables(tokens)].map(([name, value]) => `  ${name}: ${value};`);
  return `${selector} {\n${lines.join("\n")}\n}\n`;
}
