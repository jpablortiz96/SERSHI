import { enUS } from "./locales/en-US";
import { es419 } from "./locales/es-419";
import { ptBR } from "./locales/pt-BR";
import type { Locale, Messages } from "./types";

/** Every locale's messages. Adding a locale starts here (docs/LOCALIZATION.md). */
export const MESSAGES: Record<Locale, Messages> = {
  "en-US": enUS,
  "es-419": es419,
  "pt-BR": ptBR,
};
