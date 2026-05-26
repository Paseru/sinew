export type AppLocale = "en" | "fr";

export const LOCALE_STORAGE_KEY = "sinew.locale";
export const LOCALE_CHANGED_EVENT = "sinew:locale-changed";
export const DEFAULT_LOCALE: AppLocale = "en";

export function normalizeLocale(value: string | null | undefined): AppLocale {
  return value === "fr" ? "fr" : "en";
}

export function getAppLocale(): AppLocale {
  try {
    return normalizeLocale(window.localStorage.getItem(LOCALE_STORAGE_KEY));
  } catch {
    return DEFAULT_LOCALE;
  }
}

export function setAppLocale(locale: AppLocale): void {
  try {
    window.localStorage.setItem(LOCALE_STORAGE_KEY, locale);
  } catch {
    // Non-fatal: the UI can still update for this session.
  }
  window.dispatchEvent(new CustomEvent<AppLocale>(LOCALE_CHANGED_EVENT, { detail: locale }));
}
