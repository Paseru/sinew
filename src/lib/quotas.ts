import { api } from "./ipc";

export interface QuotaInfo {
  limit5h: number; // in minutes
  remaining5h: number; // in minutes
  limitWeek: number; // in minutes
  remainingWeek: number; // in minutes
  percentage5h: number;
  percentageWeek: number;
  overallPercentage: number;
  isReal: boolean;
  label?: string;
}

const DEFAULT_5H = 300; // 5 hours in minutes
const DEFAULT_WEEK = 3000; // 50 hours in minutes

export function getLocalQuota(providerId: string): QuotaInfo {
  try {
    const r5h = localStorage.getItem(`sinew.quota.${providerId}.5h`);
    const rW = localStorage.getItem(`sinew.quota.${providerId}.week`);

    const remaining5h = r5h !== null ? parseFloat(r5h) : DEFAULT_5H;
    const remainingWeek = rW !== null ? parseFloat(rW) : DEFAULT_WEEK;

    const p5h = Math.max(0, Math.min(100, (remaining5h / DEFAULT_5H) * 100));
    const pWeek = Math.max(0, Math.min(100, (remainingWeek / DEFAULT_WEEK) * 100));

    return {
      limit5h: DEFAULT_5H,
      remaining5h,
      limitWeek: DEFAULT_WEEK,
      remainingWeek,
      percentage5h: p5h,
      percentageWeek: pWeek,
      overallPercentage: Math.min(p5h, pWeek),
      isReal: false,
    };
  } catch {
    return {
      limit5h: DEFAULT_5H,
      remaining5h: DEFAULT_5H,
      limitWeek: DEFAULT_WEEK,
      remainingWeek: DEFAULT_WEEK,
      percentage5h: 100,
      percentageWeek: 100,
      overallPercentage: 100,
      isReal: false,
    };
  }
}

export function saveLocalQuota(providerId: string, remaining5h: number, remainingWeek: number) {
  try {
    localStorage.setItem(`sinew.quota.${providerId}.5h`, Math.max(0, remaining5h).toFixed(2));
    localStorage.setItem(`sinew.quota.${providerId}.week`, Math.max(0, remainingWeek).toFixed(2));
    window.dispatchEvent(new CustomEvent("sinew:quota-updated", { detail: { providerId } }));
  } catch {}
}

export function deductLocalQuota(providerId: string, minutes5h = 4, minutesWeek = 30) {
  const current = getLocalQuota(providerId);
  saveLocalQuota(
    providerId,
    Math.max(0, current.remaining5h - minutes5h),
    Math.max(0, current.remainingWeek - minutesWeek)
  );
}

export function resetLocalQuota(providerId: string) {
  saveLocalQuota(providerId, DEFAULT_5H, DEFAULT_WEEK);
}

export async function fetchProviderQuota(providerId: string): Promise<QuotaInfo> {
  const local = getLocalQuota(providerId);

  // 1. OpenRouter (Real Key Details API)
  if (providerId === "openrouter") {
    try {
      const details = await api.getOpenRouterKeyDetails();
      if (details && details.data) {
        const { limit, usage, label } = details.data;
        if (typeof limit === "number" && limit > 0) {
          const remaining = Math.max(0, limit - usage);
          const percent = Math.max(0, Math.min(100, (remaining / limit) * 100));
          return {
            limit5h: limit,
            remaining5h: remaining,
            limitWeek: limit,
            remainingWeek: remaining,
            percentage5h: percent,
            percentageWeek: percent,
            overallPercentage: percent,
            isReal: true,
            label: label || "OpenRouter Key",
          };
        } else {
          // Unlimited key or pay-as-you-go. Let's return local quota or 100%
          return {
            ...local,
            isReal: true,
            label: label || "OpenRouter Key (Unlimited)",
          };
        }
      }
    } catch (err) {
      console.warn("Failed to fetch real OpenRouter quota details, falling back to local:", err);
    }
  }

  // 2. OpenAI / Codex (Premium vs Free claims detection)
  if (providerId === "openai") {
    try {
      const status = await api.getOpenAiProviderStatus();
      if (status && status.connected) {
        const plan = status.planType?.toLowerCase();
        if (plan === "pro" || plan === "plus" || plan === "team") {
          // High quota, scale it nicely
          return {
            ...local,
            percentage5h: Math.max(local.percentage5h, 95),
            percentageWeek: Math.max(local.percentageWeek, 95),
            overallPercentage: Math.max(local.overallPercentage, 95),
            label: `Codex ${status.planType || "Premium"}`,
          };
        }
      }
    } catch {}
  }

  // 3. Google (Antigravity)
  if (providerId === "google") {
    try {
      const status = await api.getGoogleProviderStatus();
      if (status && status.connected) {
        return {
          ...local,
          label: "Antigravity Pro Active",
        };
      }
    } catch {}
  }

  return local;
}
