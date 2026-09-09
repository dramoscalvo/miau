import en from "./en.json";
import es from "./es.json";

export const locales = ["en", "es"] as const;
export type Locale = (typeof locales)[number];
export const messages: Record<Locale, typeof en> = { en, es };
