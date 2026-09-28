interface SparklineProps {
  values: number[];
  /** Upper bound of the value range (lower bound is 0). */
  max?: number;
  width?: number;
  height?: number;
}

/** A quiet line of recent values. Renders nothing until there are two points. */
export function Sparkline({ values, max = 100, width = 200, height = 34 }: SparklineProps) {
  if (values.length < 2) return <svg width={width} height={height} aria-hidden="true" />;
  const step = width / (values.length - 1);
  const points = values.map((v, i) => {
    const y = height - 2 - (Math.min(Math.max(v, 0), max) / max) * (height - 4);
    return `${(i * step).toFixed(1)},${y.toFixed(1)}`;
  });
  const line = points.join(" ");
  return (
    <svg width={width} height={height} viewBox={`0 0 ${width} ${height}`} aria-hidden="true">
      <defs>
        <linearGradient id="spark-fill" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="var(--color-signal)" stopOpacity="0.22" />
          <stop offset="1" stopColor="var(--color-signal)" stopOpacity="0" />
        </linearGradient>
      </defs>
      <polygon points={`0,${height} ${line} ${width},${height}`} fill="url(#spark-fill)" />
      <polyline
        points={line}
        fill="none"
        stroke="var(--color-signal)"
        strokeOpacity="0.8"
        strokeWidth="1.25"
        strokeLinejoin="round"
        strokeLinecap="round"
      />
    </svg>
  );
}
