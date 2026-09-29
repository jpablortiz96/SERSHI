import type { AssistantState } from "@sershi/contracts";

import { stateVisual, type StateGlyph as GlyphShape } from "../../visual/stateVisuals";
import styles from "./StateGlyph.module.css";

/** A glyph part: a stroked path, or a circle (filled when `fill`). */
type Part = { d: string } | { cx: number; cy: number; r: number; fill?: boolean };

/** Shapes on a 12px grid (decorative: a visible label always names the state). */
const SHAPES: Record<GlyphShape, readonly Part[]> = {
  hollow: [{ cx: 6, cy: 6, r: 3.25 }],
  dot: [{ cx: 6, cy: 6, r: 2.25, fill: true }],
  focus: [
    { cx: 6, cy: 6, r: 4.25 },
    { cx: 6, cy: 6, r: 1.5, fill: true },
  ],
  waves: [{ d: "M2 6c1-2 2-2 3 0s2 2 3 0 2-2 2 0" }],
  segments: [{ d: "M6 1.75a4.25 4.25 0 0 1 4.25 4.25" }, { d: "M6 10.25A4.25 4.25 0 0 1 1.75 6" }],
  nodes: [
    { cx: 6, cy: 2.5, r: 1.1, fill: true },
    { cx: 9.5, cy: 6, r: 1.1, fill: true },
    { cx: 6, cy: 9.5, r: 1.1, fill: true },
    { cx: 2.5, cy: 6, r: 1.1, fill: true },
  ],
  arrow: [{ d: "M2.5 6h7M6.75 3.25 9.5 6 6.75 8.75" }],
  check: [{ d: "M2.75 6.25 5 8.5l4.25-5" }],
  triangle: [{ d: "M6 2 10.25 9.75H1.75Z" }],
  hourglass: [{ d: "M3 2h6M3 10h6M3.5 2c0 2.5 5 3 5 4s-5 1.5-5 4M8.5 2c0 2.5-5 3-5 4s5 1.5 5 4" }],
  cross: [{ d: "M3.25 3.25l5.5 5.5M8.75 3.25l-5.5 5.5" }],
};

function renderPart(part: Part, key: number) {
  if ("d" in part) return <path key={key} d={part.d} />;
  return (
    <circle key={key} cx={part.cx} cy={part.cy} r={part.r} data-fill={part.fill || undefined} />
  );
}

/** One of SERSHI's 12px status shapes, drawn in `--tone` or `--state-color`. */
export function Glyph({ shape, className }: { shape: GlyphShape; className?: string }) {
  return (
    <svg
      className={className ? `${styles.glyph} ${className}` : styles.glyph}
      data-glyph={shape}
      width="12"
      height="12"
      viewBox="0 0 12 12"
      aria-hidden="true"
      focusable="false"
    >
      {SHAPES[shape].map(renderPart)}
    </svg>
  );
}

/** A small shape for a state, so state never depends on colour alone. */
export function StateGlyph({ state, className }: { state: AssistantState; className?: string }) {
  return <Glyph shape={stateVisual(state).glyph} className={className} />;
}
