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

export function playNotificationSound(): void {
  try {
    const isSoundEnabled = localStorage.getItem("sinew.sound-enabled") !== "false";
    if (!isSoundEnabled) return;

    const AudioContextClass = window.AudioContext || (window as any).webkitAudioContext;
    if (!AudioContextClass) return;

    const ctx = new AudioContextClass();
    const now = ctx.currentTime;

    // Note 1: E5 (659.25 Hz)
    const osc1 = ctx.createOscillator();
    const gain1 = ctx.createGain();
    osc1.type = "sine";
    osc1.frequency.setValueAtTime(659.25, now);

    gain1.gain.setValueAtTime(0, now);
    gain1.gain.linearRampToValueAtTime(0.08, now + 0.03); // Attack
    gain1.gain.exponentialRampToValueAtTime(0.001, now + 0.3); // Decay

    osc1.connect(gain1);
    gain1.connect(ctx.destination);

    // Note 2: A5 (880 Hz) starting slightly later
    const osc2 = ctx.createOscillator();
    const gain2 = ctx.createGain();
    osc2.type = "sine";
    osc2.frequency.setValueAtTime(880.00, now + 0.07);

    gain2.gain.setValueAtTime(0, now);
    gain2.gain.setValueAtTime(0, now + 0.07);
    gain2.gain.linearRampToValueAtTime(0.08, now + 0.1); // Attack
    gain2.gain.exponentialRampToValueAtTime(0.001, now + 0.4); // Decay

    osc2.connect(gain2);
    gain2.connect(ctx.destination);

    osc1.start(now);
    osc1.stop(now + 0.35);
    osc2.start(now);
    osc2.stop(now + 0.45);
  } catch (err) {
    console.error("Failed to play notification sound", err);
  }
}


