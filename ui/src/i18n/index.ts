import { createSignal } from "solid-js";
import { en, type MessageKey, type Messages } from "./en";
import { ko } from "./ko";

export type Locale = "en" | "ko";

const catalogs: Record<Locale, Messages> = { en, ko };

const [locale, setLocale] = createSignal<Locale>("en");

/** Looks up a UI string in the current locale. Reactive inside components. */
export function t(key: MessageKey): string {
  return catalogs[locale()][key];
}

export { locale, setLocale };
export type { MessageKey };
