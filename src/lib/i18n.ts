import { useEffect, useState } from "react";

export type AppLanguage = "en" | "fr";

const LANGUAGE_KEY = "sinew:language";
export const LANGUAGE_CHANGED_EVENT = "sinew:language-changed";

function normalizeLanguage(value: string | null): AppLanguage {
  return value === "fr" ? "fr" : "en";
}

export function getLanguage(): AppLanguage {
  try {
    return normalizeLanguage(window.localStorage.getItem(LANGUAGE_KEY));
  } catch {
    return "en";
  }
}

export function setLanguage(language: AppLanguage): void {
  try {
    window.localStorage.setItem(LANGUAGE_KEY, language);
  } catch {
    // Non-fatal: the current window still receives the language event.
  }
  window.dispatchEvent(
    new CustomEvent<AppLanguage>(LANGUAGE_CHANGED_EVENT, { detail: language }),
  );
}

export function useLanguage(): AppLanguage {
  const [language, setCurrentLanguage] = useState(getLanguage);

  useEffect(() => {
    const onLanguageChange = (event: Event) => {
      const detail = (event as CustomEvent<AppLanguage>).detail;
      setCurrentLanguage(normalizeLanguage(detail));
    };

    window.addEventListener(LANGUAGE_CHANGED_EVENT, onLanguageChange);
    return () => window.removeEventListener(LANGUAGE_CHANGED_EVENT, onLanguageChange);
  }, []);

  return language;
}