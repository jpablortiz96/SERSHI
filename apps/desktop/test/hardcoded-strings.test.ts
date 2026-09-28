/**
 * Guards against user-facing English creeping back into components. User
 * copy belongs in src/i18n/locales. This is a heuristic scan, kept simple so
 * it stays reliable: JSX text nodes and literal user-facing attributes.
 */
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative, resolve } from "node:path";

import { describe, expect, it } from "vitest";

const SRC = resolve(import.meta.dirname, "../src");
const SCANNED = ["components", "surfaces"];

/** Literal text allowed in markup: brand, license name, separators and glyphs. */
const ALLOWED_TEXT = new Set(["SERSHI", "Apache-2.0", "/", "↑", "—", "·"]);

function files(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    return statSync(path).isDirectory() ? files(path) : path.endsWith(".tsx") ? [path] : [];
  });
}

describe("no hard-coded user-facing strings", () => {
  const sources = SCANNED.flatMap((d) => files(join(SRC, d)));

  it("scans the application surfaces", () => {
    expect(sources.length).toBeGreaterThan(10);
  });

  it.each(sources.map((f) => [relative(SRC, f), f]))("%s", (_name, file) => {
    const source = readFileSync(file, "utf8");
    const offenders: string[] = [];

    // Text between JSX tags, e.g. <p>Hello there</p>
    for (const [, text = ""] of source.matchAll(/>([^<>{}]*[A-Za-z][^<>{}]*)</g)) {
      const trimmed = text.trim();
      if (trimmed && !ALLOWED_TEXT.has(trimmed) && !/^[\s=&|?:()!.]+$/.test(trimmed)) {
        // Skip TypeScript generics/comparisons that look like tags (e.g. `a > b && c < d`).
        if (!/[;=]|=>|\)\s*$/.test(trimmed)) offenders.push(trimmed);
      }
    }
    // Literal user-facing attributes
    for (const [, attr, value = ""] of source.matchAll(
      /\b(aria-label|aria-description|placeholder|title|alt|data-tip)="([^"]*)"/g,
    )) {
      if (/[A-Za-z]/.test(value) && !ALLOWED_TEXT.has(value)) offenders.push(`${attr}="${value}"`);
    }
    expect(offenders).toEqual([]);
  });
});
