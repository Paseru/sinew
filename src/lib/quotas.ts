import { api } from "./ipc";

export type QuotaKind = "time" | "credits" | "unavailable";

export interface QuotaInfo {
  kind: QuotaKind;
  percentage: number | null;
  isReal: boolean;
  label?: string;
  error?: string;
  source?: string;
  limit5h: number;
  remaining5h: number;
  limitWeek: number;
  remainingWeek: number;
  percentage5h: number;
  percentageWeek: number;
  overallPercentage: number;
  creditLimit?: number | null;
  creditUsed?: number | null;
  creditRemaining?: number | null;
}

function withLegacyFields(input: Omit<QuotaInfo, "limit5h" | "remaining5h" | "limitWeek" | "remainingWeek" | "percentage5h" | "percentageWeek" | "overallPercentage"> & Partial<Pick<QuotaInfo, "limit5h" | "remaining5h" | "limitWeek" | "remainingWeek" | "percentage5h" | "percentageWeek" | "overallPercentage">>): QuotaInfo {
  const limit5h = input.limit5h ?? input.creditLimit ?? 300;
  const remaining5h = input.remaining5h ?? input.creditRemaining ?? limit5h;
  const limitWeek = input.limitWeek ?? input.creditLimit ?? 3000;
  const remainingWeek = input.remainingWeek ?? input.creditRemaining ?? limitWeek;
  const percentage = input.percentage ?? Math.min(
    limit5h > 0 ? (remaining5h / limit5h) * 100 : 100,
    limitWeek > 0 ? (remainingWeek / limitWeek) * 100 : 100,
  );
  const percentage5h = input.percentage5h ?? Math.max(0, Math.min(100, limit5h > 0 ? (remaining5h / limit5h) * 100 : percentage));
  const percentageWeek = input.percentageWeek ?? Math.max(0, Math.min(100, limitWeek > 0 ? (remainingWeek / limitWeek) * 100 : percentage));
  const overallPercentage = input.overallPercentage ?? Math.max(0, Math.min(100, percentage ?? Math.min(percentage5h, percentageWeek)));

  return {
    ...input,
    percentage,
    limit5h,
    remaining5h,
    limitWeek,
    remainingWeek,
    percentage5h,
    percentageWeek,
    overallPercentage,
  };
}

export function quotaColor(percentage: number | null | undefined) {
  if (percentage == null) return "#64748b";
  if (percentage > 80) return "#10b981";
  if (percentage > 50) return "#3b82f6";
  if (percentage > 20) return "#ec4899";
  return "#ef4444";
}

export function unavailableQuota(label = "Quota non exposé par ce fournisseur"): QuotaInfo {
  return withLegacyFields({
    kind: "unavailable",
    percentage: null,
    isReal: false,
    label,
  });
}

export function getProviderLimits(providerId: string) {
  if (providerId === "anthropic") return { limit5h: 300, limitWeek: 1200 };
  if (providerId === "google") return { limit5h: 300, limitWeek: 600 };
  if (providerId === "kimi") return { limit5h: 300, limitWeek: 900 };
  if (providerId.startsWith("openai")) return { limit5h: 300, limitWeek: 1800 };
  return { limit5h: 300, limitWeek: 3000 };
}

export function getLocalQuota(providerId: string): QuotaInfo {
  const { limit5h, limitWeek } = getProviderLimits(providerId);
  try {
    const r5h = localStorage.getItem(`sinew.quota.${providerId}.5h`);
    const rW = localStorage.getItem(`sinew.quota.${providerId}.week`);
    const remaining5h = r5h !== null ? parseFloat(r5h) : limit5h;
    const remainingWeek = rW !== null ? parseFloat(rW) : limitWeek;
    return withLegacyFields({
      kind: "time",
      percentage: null,
      isReal: false,
      limit5h,
      remaining5h,
      limitWeek,
      remainingWeek,
    });
  } catch {
    return withLegacyFields({
      kind: "time",
      percentage: null,
      isReal: false,
      limit5h,
      remaining5h: limit5h,
      limitWeek,
      remainingWeek: limitWeek,
    });
  }
}

export function saveLocalQuota(providerId: string, remaining5h: number, remainingWeek: number) {
  try {
    localStorage.setItem(`sinew.quota.${providerId}.5h`, Math.max(0, remaining5h).toFixed(2));
    localStorage.setItem(`sinew.quota.${providerId}.week`, Math.max(0, remainingWeek).toFixed(2));
    window.dispatchEvent(new CustomEvent("sinew:quota-updated", { detail: { providerId } }));
  } catch {}
}

export function resetLocalQuota(providerId: string) {
  const { limit5h, limitWeek } = getProviderLimits(providerId);
  saveLocalQuota(providerId, limit5h, limitWeek);
}

export async function fetchProviderQuota(providerId: string): Promise<QuotaInfo> {
  if (providerId === "openrouter") {
    try {
      const details = await api.getOpenRouterKeyDetails();
      const data = details?.data;
      if (!data) return unavailableQuota("OpenRouter n'a pas renvoyé de données de quota");

      const limit = typeof data.limit === "number" ? data.limit : null;
      const usage = typeof data.usage === "number" ? data.usage : null;
      const label = typeof data.label === "string" ? data.label : "OpenRouter key";

      if (limit && limit > 0 && usage != null) {
        const remaining = Math.max(0, limit - usage);
        const percentage = Math.max(0, Math.min(100, (remaining / limit) * 100));
        return withLegacyFields({
          kind: "credits",
          percentage,
          isReal: true,
          label,
          source: "OpenRouter /auth/key",
          limit5h: limit,
          remaining5h: remaining,
          limitWeek: limit,
          remainingWeek: remaining,
          creditLimit: limit,
          creditUsed: usage,
          creditRemaining: remaining,
        });
      }

      return withLegacyFields({
        kind: "credits",
        percentage: null,
        isReal: true,
        label: `${label} · illimité ou sans limite configurée`,
        source: "OpenRouter /auth/key",
        creditLimit: limit,
        creditUsed: usage,
        creditRemaining: null,
      });
    } catch (err) {
      return unavailableQuota(`Impossible de lire OpenRouter: ${String(err)}`);
    }
  }

  return unavailableQuota();
}

export function deductLocalQuota(providerId: string, minutes5h = 4, minutesWeek = 30) {
  const current = getLocalQuota(providerId);
  saveLocalQuota(
    providerId,
    Math.max(0, current.remaining5h - minutes5h),
    Math.max(0, current.remainingWeek - minutesWeek),
  );
}
