import { tr } from "./tr";
import { en } from "./en";
import type { TranslationSchema } from "./types";

export type LanguageCode = "tr" | "en";

export interface LanguageOption {
  code: LanguageCode;
  label: string;
  flag: string;
}

export const LANGUAGES: LanguageOption[] = [
  { code: "tr", label: "Türkçe", flag: "🇹🇷" },
  { code: "en", label: "English", flag: "🇬🇧" },
];

export const translations: Record<LanguageCode, TranslationSchema> = {
  tr,
  en,
};

export { tr, en };
export type { TranslationSchema };
