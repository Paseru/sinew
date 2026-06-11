import { useCallback, useEffect, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { Icon } from "@iconify/react";
import { api } from "../lib/ipc";
import type { RemoteDevice, RemoteStatus } from "../types";

type Props = {
  initialStatus?: RemoteStatus | null;
  onStatusChange?: (status: RemoteStatus) => void;
};

const REMOTE_STATUS_EVENT = "remote-status-changed";

export function RemotePanel({ initialStatus = null, onStatusChange }: Props) {
  const [status, setStatus] = useState<RemoteStatus | null>(initialStatus);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const applyStatus = useCallback(
    (next: RemoteStatus) => {
      setStatus(next);
      onStatusChange?.(next);
    },
    [onStatusChange],
  );

  const refresh = useCallback(async () => {
    try {
      applyStatus(await api.remoteGetStatus());
      setError(null);
    } catch (err) {
      setError(String(err));
    }
  }, [applyStatus]);

  useEffect(() => {
    if (initialStatus) setStatus(initialStatus);
  }, [initialStatus]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    let cancelled = false;
    let unlisten: UnlistenFn | null = null;
    void listen<RemoteStatus>(REMOTE_STATUS_EVENT, (event) => {
      applyStatus(event.payload);
    }).then((nextUnlisten) => {
      if (cancelled) nextUnlisten();
      else unlisten = nextUnlisten;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [applyStatus]);

  const runAction = useCallback(
    async (action: () => Promise<RemoteStatus>) => {
      setBusy(true);
      try {
        applyStatus(await action());
        setError(null);
      } catch (err) {
        setError(String(err));
      } finally {
        setBusy(false);
      }
    },
    [applyStatus],
  );

  const setEnabled = useCallback(
    (enabled: boolean) => runAction(() => api.remoteSetEnabled(enabled)),
    [runAction],
  );
  const startPairing = useCallback(
    () => runAction(() => api.remoteStartPairing()),
    [runAction],
  );
  const stopPairing = useCallback(
    () => runAction(() => api.remoteStopPairing()),
    [runAction],
  );

  const revokeDevice = useCallback(
    (device: RemoteDevice) => {
      if (
        !window.confirm(
          `Revoke ${device.name}? It loses access immediately and must pair again.`,
        )
      ) {
        return;
      }
      void runAction(() => api.remoteRevokeDevice(device.id));
    },
    [runAction],
  );

  const enabled = status?.enabled ?? false;
  const pairing = status?.pairing ?? null;
  const devices = status?.devices ?? [];
  const activeDevices = devices.filter((device) => !device.revokedAtMs);
  const revokedDevices = devices.filter((device) => device.revokedAtMs);
  const onlineCount = activeDevices.filter((device) => device.connected).length;

  return (
    <div className="remote-panel">
      <div className="remote-panel__inner">
        <header className="remote-panel__head">
          <div className="remote-panel__head-text">
            <h1>Remote</h1>
            <p>
              Drive Sinew chat from a paired phone over remote.sinew-ide.com.
              Messages stay end-to-end encrypted between this PC and your devices.
            </p>
          </div>
          <button
            type="button"
            className="remote-panel__switch"
            role="switch"
            aria-checked={enabled}
            data-on={enabled ? "true" : "false"}
            disabled={busy || !status}
            onClick={() => setEnabled(!enabled)}
            title={enabled ? "Disable Remote" : "Enable Remote"}
          >
            <span className="remote-panel__switch-thumb" />
          </button>
        </header>

        <div className="remote-panel__status" role="list">
          <StatusChip
            icon="solar:cloud-linear"
            label="Relay"
            value={status?.relayConnected ? "Connected" : "Offline"}
            tone={status?.relayConnected ? "ok" : "off"}
          />
          <StatusChip
            icon="solar:smartphone-linear"
            label="Devices"
            value={
              activeDevices.length === 0
                ? "None paired"
                : `${onlineCount}/${activeDevices.length} online`
            }
            tone={onlineCount > 0 ? "ok" : "off"}
          />
          <StatusChip
            icon="solar:shield-check-linear"
            label="Reachable"
            value={status?.reachable ? "Yes" : "No"}
            tone={status?.reachable ? "ok" : "off"}
          />
        </div>

        <section className="remote-panel__block">
          <div className="remote-panel__block-head">
            <div>
              <h2>Pairing</h2>
              <p>The 6-digit code is accepted only while this screen is open.</p>
            </div>
            <button
              type="button"
              className="settings-pane__btn"
              data-primary={enabled && !pairing ? "true" : "false"}
              disabled={busy || !enabled}
              onClick={pairing ? stopPairing : startPairing}
            >
              {pairing ? "Close" : "Open pairing"}
            </button>
          </div>

          {pairing ? (
            <div className="remote-panel__pairing">
              <div className="remote-panel__code" aria-label="Pairing code">
                {pairing.code.split("").map((digit, index) => (
                  <span key={`${digit}-${index}`}>{digit}</span>
                ))}
              </div>
              <QrCode value={pairing.qrUrl} />
              <div className="remote-panel__pairing-copy">
                <span>Scan the code, or open this link on the phone:</span>
                <a href={pairing.qrUrl}>{pairing.qrUrl}</a>
                <small>
                  {pairing.attemptsRemaining} attempt
                  {pairing.attemptsRemaining === 1 ? "" : "s"} left before lockout.
                </small>
              </div>
            </div>
          ) : (
            <p className="remote-panel__hint">
              {enabled
                ? "Open pairing to reveal a fresh code and QR for a new phone."
                : "Enable Remote first, then open pairing to add a device."}
            </p>
          )}
        </section>

        <section className="remote-panel__block">
          <div className="remote-panel__block-head">
            <div>
              <h2>Paired devices</h2>
              <p>Revoke instantly if a phone is lost, sold, or replaced.</p>
            </div>
          </div>

          {activeDevices.length === 0 ? (
            <p className="remote-panel__hint">No devices are paired yet.</p>
          ) : (
            <div className="remote-panel__devices">
              {activeDevices.map((device) => (
                <DeviceRow
                  key={device.id}
                  device={device}
                  busy={busy}
                  onRevoke={() => revokeDevice(device)}
                />
              ))}
            </div>
          )}

          {revokedDevices.length > 0 && (
            <details className="remote-panel__revoked">
              <summary>
                {revokedDevices.length} revoked device
                {revokedDevices.length === 1 ? "" : "s"}
              </summary>
              <div className="remote-panel__devices">
                {revokedDevices.map((device) => (
                  <DeviceRow key={device.id} device={device} busy revoked />
                ))}
              </div>
            </details>
          )}
        </section>

        {error && (
          <button
            type="button"
            className="remote-panel__error"
            onClick={() => setError(null)}
            title="Dismiss"
          >
            <Icon icon="solar:danger-triangle-linear" width={14} height={14} />
            <span>{error}</span>
          </button>
        )}
      </div>
    </div>
  );
}

function StatusChip({
  icon,
  label,
  value,
  tone,
}: {
  icon: string;
  label: string;
  value: string;
  tone: "ok" | "off";
}) {
  return (
    <div className="remote-panel__chip" data-tone={tone} role="listitem">
      <Icon icon={icon} width={15} height={15} />
      <span className="remote-panel__chip-label">{label}</span>
      <span className="remote-panel__chip-value">{value}</span>
    </div>
  );
}

function DeviceRow({
  device,
  busy,
  revoked = false,
  onRevoke,
}: {
  device: RemoteDevice;
  busy: boolean;
  revoked?: boolean;
  onRevoke?: () => void;
}) {
  const connected = !revoked && device.connected;
  return (
    <div
      className="remote-panel__device"
      data-revoked={revoked ? "true" : "false"}
      data-connected={connected ? "true" : "false"}
    >
      <span className="remote-panel__device-icon">
        <Icon icon="solar:smartphone-linear" width={16} height={16} />
      </span>
      <div className="remote-panel__device-text">
        <strong>{device.name}</strong>
        <span>
          {revoked
            ? "Revoked"
            : connected
              ? "Connected now"
              : device.lastSeenAtMs
                ? `Last seen ${formatDate(device.lastSeenAtMs)}`
                : `Paired ${formatDate(device.pairedAtMs)}`}
        </span>
      </div>
      <div className="remote-panel__device-actions">
        {device.pushEnabled && !revoked && (
          <span className="remote-panel__device-push" title="Push notifications enabled">
            <Icon icon="solar:bell-linear" width={12} height={12} />
            Push
          </span>
        )}
        {!revoked && (
          <button
            type="button"
            className="settings-pane__btn"
            disabled={busy}
            onClick={onRevoke}
          >
            Revoke
          </button>
        )}
      </div>
    </div>
  );
}

function QrCode({ value }: { value: string }) {
  const src = `https://api.qrserver.com/v1/create-qr-code/?size=200x200&margin=0&data=${encodeURIComponent(value)}`;
  return (
    <div className="remote-panel__qr">
      <img src={src} alt="Pairing QR code" width={112} height={112} />
    </div>
  );
}

function formatDate(ms: number) {
  try {
    return new Intl.DateTimeFormat(undefined, {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    }).format(new Date(ms));
  } catch {
    return new Date(ms).toLocaleString();
  }
}
