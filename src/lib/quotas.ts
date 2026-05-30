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
  if (percentage > 50) return "#9ca3af";
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

  if (providerId === "google" || providerId.startsWith("google:")) {
    try {
      const quota = await api.getAntigravityQuota(providerId);
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
      
      return {
        kind: "rateLimits",
        percentage: minPercent(windows),
        isReal: true,
        label: quota?.workspaceName || (quota?.planType ? `Codex ${quota.planType}` : "Codex"),
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

  if (providerId === "cursor") {
    try {
      const quota = await api.getCursorUsage();
      const windows: QuotaWindow[] = [
        {
          label: "Auto + Composer",
          remainingPercent:
            typeof quota?.autoPercentUsed === "number"
              ? Math.max(0, 100 - quota.autoPercentUsed)
              : null,
          usedPercent:
            typeof quota?.autoPercentUsed === "number" ? quota.autoPercentUsed : null,
        },
        {
          label: "API",
          remainingPercent:
            typeof quota?.apiPercentUsed === "number"
              ? Math.max(0, 100 - quota.apiPercentUsed)
              : null,
          usedPercent:
            typeof quota?.apiPercentUsed === "number" ? quota.apiPercentUsed : null,
        },
      ];
      return {
        kind: "rateLimits",
        percentage: windows[0]?.remainingPercent ?? null,
        isReal: true,
        label: "Cursor Pro+",
        source: "Cursor GetCurrentPeriodUsage",
        windows,
      };
    } catch (err) {
      return unavailableQuota(`Impossible de lire Cursor: ${String(err)}`);
    }
  }

  if (providerId === "deepseek") {
    try {
      const balance = await api.getDeepSeekBalance();
      const infos = balance?.balance_infos || [];
      const usdInfo = infos.find((i: any) => i.currency === "USD") || infos[0];
      const cnyInfo = infos.find((i: any) => i.currency === "CNY");
      
      const activeInfo = usdInfo || cnyInfo;
      if (!activeInfo) {
        return unavailableQuota("Pas d'informations de balance trouvees");
      }

      const totalBalance = parseFloat(activeInfo.total_balance || "0");
      const toppedUpBalance = parseFloat(activeInfo.topped_up_balance || "0");
      const grantedBalance = parseFloat(activeInfo.granted_balance || "0");
      const currency = activeInfo.currency || "USD";

      const totalEver = toppedUpBalance + grantedBalance;
      
      // DeepSeek total_balance is often equal to the sum of topped_up + granted initially,
      // and when it changes, we don't have a reliable "starting" amount.
      // So percentage is mostly meaningless (always 100%).
      if (totalBalance >= 0) {
        return {
          kind: "credits",
          percentage: null,
          isReal: true,
          label: `Solde DeepSeek (${currency})`,
          source: "DeepSeek /user/balance",
          creditLimit: null,
          creditUsed: null,
          creditRemaining: totalBalance,
        };
      }

      return {
        kind: "credits",
        percentage: null,
        isReal: true,
        label: `Solde DeepSeek: $${totalBalance.toFixed(2)} ${currency}`,
        source: "DeepSeek /user/balance",
        creditLimit: null,
        creditUsed: null,
        creditRemaining: totalBalance,
      };
    } catch (err) {
      return unavailableQuota(`Impossible de lire la balance DeepSeek: ${String(err)}`);
    }
  }

  if (providerId === "anthropic" || providerId.startsWith("anthropic:")) {
    try {
      const quota = await api.getAnthropicUsage();
      const windows: QuotaWindow[] = [];

      const windowKeys: { [key: string]: string } = {
        five_hour: "Session 5h",
        seven_day: "Hebdomadaire",
        seven_day_sonnet: "Sonnet hebdo",
        seven_day_opus: "Opus hebdo",
        seven_day_oauth_apps: "Applis OAuth hebdo",
        seven_day_omelette: "Claude Design",
        seven_day_omelette_promotional: "Claude Design (promo)",
        seven_day_cowork: "Co-work hebdo",
      };

      for (const [key, label] of Object.entries(windowKeys)) {
        const win = quota[key];
        if (win && typeof win.utilization === "number") {
          const usedPercent = win.utilization;
          const remainingPercent = Math.max(0, 100 - usedPercent);
          
          let resetTime: string | null = null;
          let resetAtMs: number | null = null;
          if (win.resets_at) {
            try {
              const d = new Date(win.resets_at);
              resetAtMs = d.getTime();
              const now = new Date();
              const isToday = d.toDateString() === now.toDateString();
              if (isToday) {
                resetTime = d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
              } else {
                resetTime = d.toLocaleDateString([], { day: 'numeric', month: 'short' }) + " " + d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
              }
            } catch (e) {
              console.error("Failed to parse resets_at date:", e);
            }
          }

          windows.push({
            label,
            remainingPercent,
            usedPercent,
            resetAt: resetAtMs,
            resetTime,
          });
        }
      }

      if (quota.extra_usage && quota.extra_usage.is_enabled) {
        const limitCents = quota.extra_usage.monthly_limit || 0;
        const usedCents = quota.extra_usage.used_credits || 0;
        const remainingCents = Math.max(0, limitCents - usedCents);
        const limitUsd = (limitCents / 100).toFixed(2);
        const usedUsd = (usedCents / 100).toFixed(2);
        const remainingUsd = (remainingCents / 100).toFixed(2);
        const currency = quota.extra_usage.currency || 'USD';
        
        const pct = limitCents > 0 ? Math.max(0, 100 - (usedCents / limitCents) * 100) : 100;
        
        windows.push({
          label: `Extra: $${remainingUsd} / $${limitUsd} ${currency}`,
          remainingPercent: pct,
          usedPercent: limitCents > 0 ? (usedCents / limitCents) * 100 : 0,
        });
      }

      if (!windows.length) {
        return unavailableQuota("Aucune donnee de quota disponible");
      }

      return {
        kind: "rateLimits",
        percentage: minPercent(windows),
        isReal: true,
        label: "Claude Pro / Max",
        source: "Anthropic /api/oauth/usage",
        windows,
      };
    } catch (err) {
      return unavailableQuota(`Impossible de lire Claude: ${String(err)}`);
    }
  }

  return unavailableQuota("Quota reel non expose par ce fournisseur");
}

export function deductLocalQuota(_providerId: string) {
  // Real quota endpoints are polled instead of simulated locally.
}
