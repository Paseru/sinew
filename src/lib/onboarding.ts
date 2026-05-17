const ONBOARDING_DISMISSED_KEY = "sinew:onboarding:v2-complete";
export const ONBOARDING_REPLAY_EVENT = "sinew:onboarding-replay";

export function shouldShowOnboarding(): boolean {
  try {
    return window.localStorage.getItem(ONBOARDING_DISMISSED_KEY) !== "true";
  } catch {
    return true;
  }
}

export function markOnboardingDismissed(): void {
  try {
    window.localStorage.setItem(ONBOARDING_DISMISSED_KEY, "true");
  } catch {
    // Non-fatal: the guide can reappear if storage is unavailable.
  }
}

export function replayOnboarding(): void {
  try {
    window.localStorage.removeItem(ONBOARDING_DISMISSED_KEY);
  } catch {
    // Non-fatal: the replay event still lets the current window show the guide.
  }
  window.dispatchEvent(new CustomEvent(ONBOARDING_REPLAY_EVENT));
}