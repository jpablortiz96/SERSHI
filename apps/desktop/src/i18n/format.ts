import { isLocale } from "./detect";
import { DEFAULT_LOCALE, type Locale } from "./types";

const GB = 1024 ** 3;

export interface PercentParts {
  number: string;
  /** The sign plus any locale spacing, e.g. "%" or " %". */
  sign: string;
}

/**
 * Locale-aware formatting built on the platform `Intl` APIs. Formatters are
 * created once per locale.
 */
export function createFormatters(requested: Locale) {
  const locale = isLocale(requested) ? requested : DEFAULT_LOCALE;
  const decimal1 = new Intl.NumberFormat(locale, {
    minimumFractionDigits: 1,
    maximumFractionDigits: 1,
  });
  const integer = new Intl.NumberFormat(locale, { maximumFractionDigits: 0 });
  const percent = new Intl.NumberFormat(locale, { style: "percent", maximumFractionDigits: 0 });
  const time = new Intl.DateTimeFormat(locale, { timeStyle: "medium" });
  const unit = (u: "day" | "hour" | "minute" | "millisecond", display: "narrow" | "short") =>
    new Intl.NumberFormat(locale, { style: "unit", unit: u, unitDisplay: display });
  const days = unit("day", "narrow");
  const hours = unit("hour", "narrow");
  const minutes = unit("minute", "narrow");
  const millis = unit("millisecond", "short");

  return {
    integer: (value: number) => integer.format(value),
    /** Bytes as gigabytes: one decimal below 100 GB, whole numbers above. */
    gigabytes: (bytes: number) => {
      const gb = bytes / GB;
      return gb >= 100 ? integer.format(gb) : decimal1.format(gb);
    },
    /** `value` is 0–100. */
    percent: (value: number) => percent.format(value / 100),
    percentParts: (value: number): PercentParts => {
      const parts = percent.formatToParts(value / 100);
      return {
        number: parts
          .filter((p) => p.type !== "percentSign" && p.type !== "literal")
          .map((p) => p.value)
          .join(""),
        sign: parts
          .filter((p) => p.type === "percentSign" || p.type === "literal")
          .map((p) => p.value)
          .join(""),
      };
    },
    time: (epochMs: number) => time.format(epochMs),
    milliseconds: (ms: number) => millis.format(ms),
    uptime: (totalSeconds: number) => {
      const d = Math.floor(totalSeconds / 86_400);
      const h = Math.floor((totalSeconds % 86_400) / 3_600);
      const m = Math.floor((totalSeconds % 3_600) / 60);
      if (d > 0) return `${days.format(d)} ${hours.format(h)}`;
      if (h > 0) return `${hours.format(h)} ${minutes.format(m)}`;
      return minutes.format(m);
    },
  };
}

export type Formatters = ReturnType<typeof createFormatters>;
