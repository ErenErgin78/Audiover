import { useCallback } from "react";
import { useAudioStore } from "../store/audioStore";
import { translations, LANGUAGES, type LanguageCode, type TranslationSchema } from "../i18n";
import { api } from "./useApi";

export function useI18n() {
  const language = useAudioStore((s) => s.language);
  const setStoreLanguage = useAudioStore((s) => s.setLanguage);

  const t: TranslationSchema = translations[language] ?? translations.tr;

  const setLanguage = useCallback(
    async (lang: LanguageCode) => {
      setStoreLanguage(lang);
      try {
        await api.setLanguage(lang);
      } catch (e) {
        console.error("Failed to persist language:", e);
      }
    },
    [setStoreLanguage]
  );

  return {
    t,
    language,
    setLanguage,
    languages: LANGUAGES,
  };
}
