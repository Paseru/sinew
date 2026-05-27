import { api } from "./ipc";

export type QuotaKind = "rateLimits" | "credits" | "groups" | "unavailable";

export interface QuotaWindow {
  label: string;
  remainingPercent: number | null;
  usedPercent?: number | null;
  windowMinutes?: number | null;
  resetAt?: number | null;
  resetTime?: string | null;
}

export interface QuotaInfo {
  kind: QuotaKind;
  percentage: number | null;
  isReal: boolean;
  label?: string;
  error?: string;
  source?: string;
  windows?: QuotaWindow[];
  groups?: QuotaWindow[];
  creditLimit?: number | null;
  creditUsed?: number | null;
  creditRemaining?: number | null;
}

export function quotaColor(percentage: number | null | undefined) {
  if (percentage == null) return "#64748b";
  if (percentage > 80) return "#10b981";
  if (percentage > 50) return "#3b82f6";
  if (percentage > 20) return "#ec4899";
  return "#ef4444";
}

export function unavailableQuota(label = "Quota non disponible"): QuotaInfo {
  return {
    kind: "unavailable",
    percentage: null,
    isReal: false,
    label,
  };
}

export const quotaCache = new Map<string, { data: QuotaInfo; timestamp: number }>();

export function getCachedQuota(providerId: string): QuotaInfo | null {
  const cached = quotaCache.get(providerId);
  if (cached && Date.now() - cached.timestamp < 1000 * 60 * 5) {
    return cached.data;
  }
  return null;
}

function minPercent(items: QuotaWindow[]) {
  const values = items
    .map((item) => item.remainingPercent)
    .filter((value): value is number => typeof value === "number");
  if (!values.length) return null;
  return Math.min(...values);
}

function codexWindow(label: string, input: any): QuotaWindow | null {
  if (!input) return null;
  return {
    label,
    remainingPercent: typeof input.remainingPercent === "number" ? input.remainingPercent : null,
    usedPercent: typeof input.usedPercent === "number" ? input.usedPercent : null,
    windowMinutes: typeof input.windowMinutes === "number" ? input.windowMinutes : null,
    resetAt: typeof input.resetAt === "number" ? input.resetAt : null,
  };
}

export async function fetchProviderQuota(providerId: string): Promise<QuotaInfo> {
  const result = await doFetchProviderQuota(providerId);
  quotaCache.set(providerId, { data: result, timestamp: Date.now() });
  try {
    window.dispatchEvent(new CustomEvent("sinew:quota-updated"));
  } catch (err) {
    console.error("Failed to dispatch sinew:quota-updated event:", err);
  }
  return result;
}

async function doFetchProviderQuota(providerId: string): Promise<QuotaInfo> {
  if (providerId === "openrouter") {
    try {
      const details = await api.getOpenRouterKeyDetails();
      const data = details?.data;
      if (!data) return unavailableQuota("OpenRouter n'a pas renvoye de donnees de quota");

      const limit = typeof data.limit === "number" ? data.limit : null;
      const usage = typeof data.usage === "number" ? data.usage : null;
      const label = typeof data.label === "string" ? data.label : "OpenRouter key";

      if (limit && limit > 0 && usage != null) {
        const remaining = Math.max(0, limit - usage);
        const percentage = Math.max(0, Math.min(100, (remaining / limit) * 100));
        return {
          kind: "credits",
          percentage,
          isReal: true,
          label,
          source: "OpenRouter /auth/key",
          creditLimit: limit,
          creditUsed: usage,
          creditRemaining: remaining,
        };
      }

      return {
        kind: "credits",
        percentage: null,
        isReal: true,
        label: `${label} - illimite ou sans limite configuree`,
        source: "OpenRouter /auth/key",
        creditLimit: limit,
        creditUsed: usage,
        creditRemaining: null,
      };
    } catch (err) {
      return unavailableQuota(`Impossible de lire OpenRouter: ${String(err)}`);
    }
  }

  if (providerId === "google") {
    try {
      const quota = await api.getAntigravityQuota();
      const groups: QuotaWindow[] = Array.isArray(quota?.groups)
        ? quota.groups.map((group: any) => ({
            label: group.label || group.group,
            remainingPercent:
              typeof group.remainingPercent === "number" ? group.remainingPercent : null,
            resetTime: typeof group.resetTime === "string" ? group.resetTime : null,
          }))
        : [];
      if (!groups.length) return unavailableQuota("Antigravity n'a pas renvoye de quota modele");
      return {
        kind: "groups",
        percentage: minPercent(groups),
        isReal: true,
        label: quota?.projectId ? `Projet ${quota.projectId}` : "Antigravity",
        source: "Antigravity fetchAvailableModels",
        groups,
      };
    } catch (err) {
      return unavailableQuota(`Impossible de lire Antigravity: ${String(err)}`);
    }
  }

  if (providerId === "openai" || providerId.startsWith("openai:")) {
    try {
      const quota = await api.getOpenAiCodexRateLimits(providerId === "openai" ? undefined : providerId);
      const windows = [
        codexWindow("Fenetre courte", quota?.primary),
        codexWindow("Fenetre longue", quota?.secondary),
      ].filter((value): value is QuotaWindow => Boolean(value));
      if (!windows.length) return unavailableQuota("Codex n'a pas renvoye de fenetre de quota");
      
      const primaryPercent = quota?.primary && typeof quota.primary.remainingPercent === "number"
        ? quota.primary.remainingPercent
        : null;

      return {
        kind: "rateLimits",
        percentage: primaryPercent !== null ? primaryPercent : minPercent(windows),
        isReal: true,
        label: quota?.planType ? `Codex ${quota.planType}` : "Codex",
        source: "ChatGPT Codex /wham/usage",
        windows,
      };
    } catch (err) {
      const errStr = String(err);
      if (errStr.includes("403 Forbidden") || errStr.includes("404 Not Found")) {
        return unavailableQuota("Quota illimite ou invisible (Business/Enterprise)");
      }
      return unavailableQuota(`Impossible de lire Codex: ${errStr}`);
    }
  }

  return unavailableQuota("Quota reel non expose par ce fournisseur");
}

export function deductLocalQuota(_providerId: string) {
  // Real quota endpoints are polled instead of simulated locally.
}
