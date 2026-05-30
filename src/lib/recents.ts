import type { RecentWorkspace } from "../types";
import { invoke } from "@tauri-apps/api/core";

const RECENTS_KEY = "sinew.recentWorkspaces";
const LAST_KEY = "sinew.lastWorkspace";
const MAX_RECENTS = 12;

export function loadRecents(): RecentWorkspace[] {
  try {
    const raw = localStorage.getItem(RECENTS_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as RecentWorkspace[];
    if (!Array.isArray(parsed)) return [];
    return parsed
      .filter((r) => r && typeof r.path === "string")
      .sort((a, b) => b.lastOpenedMs - a.lastOpenedMs);
  } catch {
    return [];
  }
}

export function recordRecent(path: string, name: string): RecentWorkspace[] {
  const now = Date.now();
  const existing = loadRecents().filter((r) => r.path !== path);
  const next: RecentWorkspace[] = [
    { path, name, lastOpenedMs: now },
    ...existing,
  ].slice(0, MAX_RECENTS);
  try {
    localStorage.setItem(RECENTS_KEY, JSON.stringify(next));
    localStorage.setItem(LAST_KEY, path);
  } catch {
    // ignore quota errors
  }
  
  invoke("record_recent_workspace_command", { path, name }).catch(() => {});
  
  return next;
}

export function loadLastWorkspace(): string | null {
  try {
    return localStorage.getItem(LAST_KEY);
  } catch {
    return null;
  }
}

export async function loadLastWorkspaceAsync(): Promise<string | null> {
  try {
    const recents = await invoke<RecentWorkspace[]>("get_recent_workspaces_command");
    if (recents && recents.length > 0) {
      return recents[0].path;
    }
  } catch {
    // fallback
  }
  return loadLastWorkspace();
}

export function clearLastWorkspace(): void {
  try {
    localStorage.removeItem(LAST_KEY);
  } catch {}
}

export function removeRecent(path: string): RecentWorkspace[] {
  const existing = loadRecents().filter((r) => r.path !== path);
  try {
    localStorage.setItem(RECENTS_KEY, JSON.stringify(existing));
    const last = localStorage.getItem(LAST_KEY);
    if (last === path) {
      localStorage.removeItem(LAST_KEY);
    }
  } catch {}

  invoke("clear_recent_workspaces_command", { path }).catch(() => {});

  return existing;
}

export function deriveName(path: string): string {
  const trimmed = path.replace(/\/$/, "");
  const idx = trimmed.lastIndexOf("/");
  return idx >= 0 ? trimmed.slice(idx + 1) || trimmed : trimmed;
}
