export function formatTurnDuration(durationMs: number): string {
  const totalSeconds = Math.max(0, Math.round(durationMs / 1000));
  if (totalSeconds < 1) return "<1s";
  if (totalSeconds < 60) return `${totalSeconds}s`;

  const totalMinutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  if (totalMinutes < 60) {
    return seconds === 0 ? `${totalMinutes}m` : `${totalMinutes}m ${seconds}s`;
  }

  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  return minutes === 0 ? `${hours}h` : `${hours}h ${minutes}m`;
}

export function formatFullTokenCount(value: number): string {
  const digits = String(Math.max(0, Math.round(value)));
  return digits.replace(/\B(?=(\d{3})+(?!\d))/g, " ");
}

export function formatCompactTokenCount(value: number): string {
  const rounded = Math.max(0, Math.round(value));
  if (rounded < 1_000) return rounded.toLocaleString("en-US");
  const compact = rounded / 1_000;
  const digits = compact < 10 ? 1 : 0;
  return `${compact.toFixed(digits).replace(/\.0$/, "")}K`;
}

export function hashString(value: string): string {
  let hash = 2166136261;
  for (let i = 0; i < value.length; i++) {
    hash ^= value.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }
  return (hash >>> 0).toString(36);
}

