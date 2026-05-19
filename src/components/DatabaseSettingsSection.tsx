import { useEffect, useState } from "react";
import { Icon } from "@iconify/react";
import { open, save } from "@tauri-apps/plugin-dialog";
import type {
  DatabaseActivityEntry,
  DatabaseConnectionFields,
  DatabaseCredentialMode,
  DatabaseEngine,
  DatabaseSettings,
  DatabaseSourceConfig,
  DatabaseSslMode,
} from "../types";
import {
  connectionStringPlaceholder,
  databaseEngineLabel,
  databaseEngineShortLabel,
  defaultDatabaseFields,
  defaultPortForEngine,
  defaultSupabaseRest,
  formatTimestamp,
  isDirectSqlEngine,
  nullableNumberFromInput,
  numberFromInput,
  sourceStatusLabel,
  sourceStatusTone,
  parseEnvForDatabaseSources,
  type EnvDatabaseDetection,
} from "../lib/databaseSettings";

const DATABASE_ENGINE_OPTIONS: {
  value: DatabaseEngine;
  label: string;
  shortLabel: string;
  icon: string;
}[] = [
  {
    value: "postgres",
    label: "PostgreSQL",
    shortLabel: "Postgres",
    icon: "logos:postgresql",
  },
  {
    value: "mysql",
    label: "MySQL / MariaDB",
    shortLabel: "MySQL",
    icon: "logos:mysql-icon",
  },
  {
    value: "sqlite",
    label: "SQLite",
    shortLabel: "SQLite",
    icon: "vscode-icons:file-type-sqlite",
  },
  {
    value: "mssql",
    label: "Microsoft SQL Server",
    shortLabel: "MSSQL",
    icon: "simple-icons:microsoftsqlserver",
  },
  {
    value: "supabaseRest",
    label: "Supabase REST",
    shortLabel: "Supabase",
    icon: "simple-icons:supabase",
  },
];

const DATABASE_CREDENTIAL_MODE_OPTIONS: {
  value: DatabaseCredentialMode;
  label: string;
}[] = [
  { value: "connectionString", label: "Connection string" },
  { value: "fields", label: "Separate fields" },
];

const DATABASE_SSL_OPTIONS: { value: DatabaseSslMode; label: string }[] = [
  { value: "disabled", label: "Disabled" },
  { value: "required", label: "Required" },
  { value: "strict", label: "Strict + certificate" },
];

type PickerOption = { value: string; label: string; icon?: string };

type DatabaseSectionProps = {
  settings: DatabaseSettings;
  selectedSource: DatabaseSourceConfig | null;
  activity: DatabaseActivityEntry[];
  loading: boolean;
  saving: boolean;
  testingSourceId: string | null;
  activityLoading: boolean;
  dirty: boolean;
  status: string | null;
  activeCount: number;
  onSelectSource: (id: string) => void;
  onAddSource: (engine: DatabaseEngine) => void;
  onAddSourceFromTemplate: (detection: EnvDatabaseDetection) => void;
  onUpdateSource: (id: string, patch: Partial<DatabaseSourceConfig>) => void;
  onDeleteSource: (id: string) => void;
  onSave: () => void;
  onRefresh: () => void;
  onTestSource: (source: DatabaseSourceConfig) => void;
  onRefreshActivity: (sourceId: string) => void;
  onClearActivity: (sourceId: string) => void;
};

export function DatabaseSection({
  settings,
  selectedSource,
  activity,
  loading,
  saving,
  testingSourceId,
  activityLoading,
  dirty,
  status,
  activeCount,
  onSelectSource,
  onAddSource,
  onAddSourceFromTemplate,
  onUpdateSource,
  onDeleteSource,
  onSave,
  onRefresh,
  onTestSource,
  onRefreshActivity,
  onClearActivity,
}: DatabaseSectionProps) {
  const [addingEngine, setAddingEngine] = useState<DatabaseEngine>("postgres");
  const [envOpen, setEnvOpen] = useState(false);
  const [envText, setEnvText] = useState("");
  const [envDetections, setEnvDetections] = useState<EnvDatabaseDetection[]>(
    [],
  );
  const [envScanned, setEnvScanned] = useState(false);
  const sourceCount = settings.sources.length;

  return (
    <>
      <header className="settings-pane__header">
        <div className="settings-pane__header-text">
          <h1 className="settings-pane__title">Database sources</h1>
          <p className="settings-pane__subtitle">
            {loading
              ? "Loading…"
              : sourceCount === 0
                ? "Declare SQL and Supabase REST sources for the agent."
                : `${activeCount}/${sourceCount} exposed to the agent`}
          </p>
        </div>
        <div className="settings-pane__actions">
          {status && (
            <span
              className="settings-pane__status"
              data-tone={
                status === "Saved" ||
                status === "Connection OK" ||
                status === "Activity cleared"
                  ? "ok"
                  : "error"
              }
            >
              {status}
            </span>
          )}
          <button
            type="button"
            className="settings-pane__btn"
            onClick={onRefresh}
            disabled={loading || saving}
          >
            <Icon icon="solar:refresh-linear" width={13} height={13} />
            <span>{loading ? "Refreshing…" : "Refresh"}</span>
          </button>
          <button
            type="button"
            className="settings-pane__btn"
            data-primary="true"
            onClick={onSave}
            disabled={loading || saving || !dirty}
          >
            <Icon
              icon={saving ? "solar:refresh-linear" : "solar:diskette-linear"}
              width={13}
              height={13}
            />
            <span>{saving ? "Saving…" : "Save"}</span>
          </button>
        </div>
      </header>

      <div className="settings-pane__body settings-pane__body--database">
        <aside className="settings-pane__nav-list settings-pane__database-list">
          <div className="settings-pane__nav-list-head">
            <span>Sources</span>
            <span className="settings-pane__servers-meta">
              {activeCount}/{sourceCount} active
            </span>
          </div>
          <div className="settings-pane__database-add">
            <LocalPicker
              value={addingEngine}
              options={DATABASE_ENGINE_OPTIONS.map((engine) => ({
                value: engine.value,
                label: engine.shortLabel,
                icon: engine.icon,
              }))}
              onSelect={(value) => setAddingEngine(value as DatabaseEngine)}
            />
            <button
              type="button"
              className="settings-pane__btn"
              onClick={() => onAddSource(addingEngine)}
              disabled={loading || saving}
            >
              <Icon icon="solar:add-circle-linear" width={13} height={13} />
              <span>Add</span>
            </button>
          </div>
          <div className="settings-pane__database-env-import">
            <button
              type="button"
              className="settings-pane__btn settings-pane__btn--ghost"
              onClick={() => setEnvOpen((open) => !open)}
            >
              <Icon
                icon={envOpen ? "solar:alt-arrow-down-linear" : "solar:alt-arrow-right-linear"}
                width={13}
                height={13}
              />
              <span>Import from .env (paste)</span>
            </button>
            {envOpen && (
              <div className="settings-pane__database-env-body">
                <textarea
                  className="settings-pane__textarea"
                  rows={6}
                  spellCheck={false}
                  placeholder="POSTGRES_URL=postgres://user:pass@host/db&#10;SUPABASE_URL=https://xyz.supabase.co&#10;SUPABASE_ANON_KEY=…"
                  value={envText}
                  onChange={(event) => {
                    setEnvText(event.target.value);
                    setEnvScanned(false);
                  }}
                />
                <div className="settings-pane__database-env-actions">
                  <button
                    type="button"
                    className="settings-pane__btn"
                    onClick={() => {
                      setEnvDetections(parseEnvForDatabaseSources(envText));
                      setEnvScanned(true);
                    }}
                    disabled={envText.trim().length === 0}
                  >
                    <Icon icon="solar:magnifer-linear" width={13} height={13} />
                    <span>Detect</span>
                  </button>
                  <button
                    type="button"
                    className="settings-pane__btn"
                    onClick={() => {
                      setEnvText("");
                      setEnvDetections([]);
                      setEnvScanned(false);
                    }}
                    disabled={envText.length === 0 && envDetections.length === 0}
                  >
                    <Icon icon="solar:eraser-linear" width={13} height={13} />
                    <span>Clear</span>
                  </button>
                </div>
                {envScanned && envDetections.length === 0 && (
                  <div className="settings-pane__database-env-empty">
                    No database connection detected in the pasted content.
                  </div>
                )}
                {envDetections.length > 0 && (
                  <ul className="settings-pane__database-env-results">
                    {envDetections.map((detection, index) => (
                      <li
                        key={`${detection.engine}-${index}`}
                        className="settings-pane__database-env-result"
                      >
                        <div className="settings-pane__database-env-result-meta">
                          <span className="settings-pane__database-engine-pill">
                            {databaseEngineShortLabel(detection.engine)}
                          </span>
                          <span className="settings-pane__database-env-result-preview">
                            {detection.preview}
                          </span>
                        </div>
                        <button
                          type="button"
                          className="settings-pane__btn"
                          onClick={() => {
                            onAddSourceFromTemplate(detection);
                          }}
                          disabled={loading || saving}
                        >
                          <Icon icon="solar:download-linear" width={13} height={13} />
                          <span>Create source</span>
                        </button>
                      </li>
                    ))}
                  </ul>
                )}
                <p className="settings-pane__database-env-note">
                  Paste only the lines you trust. Nothing is read from the
                  workspace and the `.env` file is never modified.
                </p>
              </div>
            )}
          </div>
          <div className="settings-pane__nav-list-items">
            {settings.sources.map((source) => {
              const statusTone = sourceStatusTone(source);
              return (
                <button
                  type="button"
                  key={source.id}
                  className="settings-pane__nav-list-item settings-pane__database-source-row"
                  data-active={selectedSource?.id === source.id ? "true" : "false"}
                  data-on={source.enabled ? "true" : "false"}
                  onClick={() => onSelectSource(source.id)}
                >
                  <span
                    className="settings-pane__nav-list-item-dot"
                    data-tone={source.enabled ? statusTone : "off"}
                    aria-hidden
                  />
                  <span className="settings-pane__nav-list-item-name">
                    {source.name || "Untitled"}
                  </span>
                  <span className="settings-pane__database-engine-pill">
                    {databaseEngineShortLabel(source.engine)}
                  </span>
                </button>
              );
            })}
            {!loading && settings.sources.length === 0 && (
              <div className="settings-pane__nav-list-empty">
                No database sources yet — choose a type and click Add.
              </div>
            )}
          </div>
        </aside>

        <main className="settings-pane__detail-pane">
          {selectedSource ? (
            <DatabaseSourceEditor
              source={selectedSource}
              allSources={settings.sources}
              activity={activity}
              disabled={loading || saving}
              testing={testingSourceId === selectedSource.id}
              activityLoading={activityLoading}
              onChange={(patch) => onUpdateSource(selectedSource.id, patch)}
              onDelete={() => onDeleteSource(selectedSource.id)}
              onTest={() => onTestSource(selectedSource)}
              onRefreshActivity={() => onRefreshActivity(selectedSource.id)}
              onClearActivity={() => onClearActivity(selectedSource.id)}
            />
          ) : (
            <div className="settings-pane__empty settings-pane__empty--main">
              <Icon icon="solar:database-linear" width={22} height={22} />
              <span className="settings-pane__empty-title">
                {sourceCount === 0 ? "No sources configured" : "Select a source"}
              </span>
              <span className="settings-pane__empty-sub">
                Database credentials stay in local settings and are hidden by default.
              </span>
            </div>
          )}
        </main>
      </div>
    </>
  );
}

function DatabaseSourceEditor({
  source,
  allSources,
  activity,
  disabled,
  testing,
  activityLoading,
  onChange,
  onDelete,
  onTest,
  onRefreshActivity,
  onClearActivity,
}: {
  source: DatabaseSourceConfig;
  allSources: DatabaseSourceConfig[];
  activity: DatabaseActivityEntry[];
  disabled: boolean;
  testing: boolean;
  activityLoading: boolean;
  onChange: (patch: Partial<DatabaseSourceConfig>) => void;
  onDelete: () => void;
  onTest: () => void;
  onRefreshActivity: () => void;
  onClearActivity: () => void;
}) {
  const [confirmDelete, setConfirmDelete] = useState(false);
  const duplicateName = allSources.some(
    (item) => item.id !== source.id && item.name.trim() === source.name.trim(),
  );
  const statusTone = sourceStatusTone(source);
  const statusLabel = sourceStatusLabel(source);

  useEffect(() => {
    setConfirmDelete(false);
  }, [source.id]);

  useEffect(() => {
    if (!confirmDelete) return;
    const id = window.setTimeout(() => setConfirmDelete(false), 3000);
    return () => window.clearTimeout(id);
  }, [confirmDelete]);

  return (
    <div className="settings-pane__detail settings-pane__database-detail">
      <div className="settings-pane__detail-head">
        <input
          type="text"
          className="settings-pane__detail-title-input"
          value={source.name}
          placeholder="Source name"
          onChange={(event) => onChange({ name: event.target.value })}
          disabled={disabled}
          aria-label="Database source name"
        />
        <span className="settings-pane__chip" data-tone={source.enabled ? statusTone : "off"}>
          <span className="settings-pane__chip-dot" />
          {source.enabled ? statusLabel : "disabled"}
        </span>
        <button
          type="button"
          className="settings-pane__switch"
          role="switch"
          aria-checked={source.enabled}
          aria-label={source.enabled ? "Disable source" : "Enable source"}
          data-on={source.enabled ? "true" : "false"}
          disabled={disabled}
          onClick={() => onChange({ enabled: !source.enabled })}
        >
          <span className="settings-pane__switch-thumb" />
        </button>
      </div>

      <div className="settings-pane__detail-body settings-pane__database-form">
        <div className="settings-pane__database-summary">
          <span className="settings-pane__chip" data-tone="pending">
            {databaseEngineLabel(source.engine)}
          </span>
          <span className="settings-pane__chip" data-tone={source.readOnly ? "ok" : "pending"}>
            {source.readOnly ? "Read-only" : "Read + write + DDL"}
          </span>
          <span
            className="settings-pane__chip"
            data-tone={source.requireConfirmationForDestructive ? "ok" : "error"}
          >
            {source.requireConfirmationForDestructive
              ? "Destructive confirmation on"
              : "Destructive confirmation off"}
          </span>
        </div>

        {duplicateName && (
          <div className="settings-pane__editor-error">
            <Icon icon="solar:danger-triangle-linear" width={13} height={13} />
            <span>Source names must be unique; agents use the name to select a source.</span>
          </div>
        )}

        <div className="settings-pane__database-help">
          <Icon icon="solar:shield-warning-linear" width={14} height={14} />
          <span>
            Credentials are stored locally and hidden by default. Remove database variables
            from project <code>.env</code> files once the source is configured here; no
            workspace file is read or modified automatically.
          </span>
        </div>

        <div className="settings-pane__database-grid">
          <label className="settings-pane__field">
            <span>Engine</span>
            <input value={databaseEngineLabel(source.engine)} disabled />
          </label>
          <label className="settings-pane__field">
            <span>Default schema</span>
            <input
              value={source.defaultSchema ?? ""}
              placeholder={source.engine === "postgres" ? "public" : "optional"}
              onChange={(event) => onChange({ defaultSchema: event.target.value })}
              disabled={disabled}
            />
          </label>
          <label className="settings-pane__field">
            <span>Default row limit</span>
            <input
              type="number"
              min={1}
              max={100000}
              value={source.defaultRowLimit}
              onChange={(event) =>
                onChange({ defaultRowLimit: numberFromInput(event.target.value, 1000) })
              }
              disabled={disabled}
            />
          </label>
          <label className="settings-pane__field">
            <span>Timeout (ms)</span>
            <input
              type="number"
              min={100}
              max={300000}
              value={source.defaultTimeoutMs}
              onChange={(event) =>
                onChange({ defaultTimeoutMs: numberFromInput(event.target.value, 30000) })
              }
              disabled={disabled}
            />
          </label>
        </div>

        {source.engine === "sqlite" ? (
          <SqliteFields source={source} disabled={disabled} onChange={onChange} />
        ) : source.engine === "supabaseRest" ? (
          <SupabaseRestFields source={source} disabled={disabled} onChange={onChange} />
        ) : (
          <DirectSqlFields source={source} disabled={disabled} onChange={onChange} />
        )}

        <label className="settings-pane__field">
          <span>Note</span>
          <textarea
            value={source.note}
            rows={3}
            placeholder="Optional context for you. Agents only receive safe source metadata."
            onChange={(event) => onChange({ note: event.target.value })}
            disabled={disabled}
          />
        </label>

        <ToggleRow
          label="Read-only mode"
          hint="When enabled, agent query execution is restricted to read operations."
          checked={source.readOnly}
          disabled={disabled}
          onToggle={() => onChange({ readOnly: !source.readOnly })}
        />

        <ToggleRow
          label="Confirm destructive operations"
          hint="Ask before writes, DDL, drops, or large deletes. Disabling this gives the agent lower-friction write access."
          checked={source.requireConfirmationForDestructive}
          disabled={disabled}
          onToggle={() =>
            onChange({
              requireConfirmationForDestructive:
                !source.requireConfirmationForDestructive,
            })
          }
        />

        {source.lastConnectionStatus.message && (
          <div
            className={
              source.lastConnectionStatus.status === "error"
                ? "settings-pane__tools-error"
                : "settings-pane__database-notice"
            }
          >
            {source.lastConnectionStatus.message}
          </div>
        )}

        <div className="settings-pane__field-actions">
          <button
            type="button"
            className="settings-pane__btn"
            onClick={onTest}
            disabled={disabled || testing}
          >
            <Icon icon="solar:plug-circle-linear" width={13} height={13} />
            <span>{testing ? "Testing…" : "Test connection"}</span>
          </button>
          {confirmDelete ? (
            <>
              <span className="settings-pane__muted">Delete this source?</span>
              <button
                type="button"
                className="settings-pane__btn"
                data-tone="danger"
                onClick={() => {
                  setConfirmDelete(false);
                  onDelete();
                }}
                disabled={disabled}
              >
                Confirm delete
              </button>
              <button
                type="button"
                className="settings-pane__btn"
                onClick={() => setConfirmDelete(false)}
              >
                Cancel
              </button>
            </>
          ) : (
            <button
              type="button"
              className="settings-pane__btn"
              data-tone="danger-ghost"
              onClick={() => setConfirmDelete(true)}
              disabled={disabled}
            >
              <Icon icon="solar:trash-bin-trash-linear" width={13} height={13} />
              <span>Delete source</span>
            </button>
          )}
        </div>

        <DatabaseActivityList
          activity={activity}
          loading={activityLoading}
          onRefresh={onRefreshActivity}
          onClear={onClearActivity}
        />
      </div>
    </div>
  );
}

function DirectSqlFields({
  source,
  disabled,
  onChange,
}: {
  source: DatabaseSourceConfig;
  disabled: boolean;
  onChange: (patch: Partial<DatabaseSourceConfig>) => void;
}) {
  const mode = source.credentialMode ?? "connectionString";
  const fields = source.fields ?? defaultDatabaseFields(source.engine);
  const updateFields = (patch: Partial<DatabaseConnectionFields>) => {
    onChange({ fields: { ...fields, ...patch } });
  };

  if (!isDirectSqlEngine(source.engine)) return null;

  return (
    <div className="settings-pane__database-block">
      <div className="settings-pane__tool-group-head">
        <h2>Credentials</h2>
        <span>TLS is required by default for network sources</span>
      </div>
      <div className="settings-pane__tool-provider-switch" role="group" aria-label="Credential mode">
        {DATABASE_CREDENTIAL_MODE_OPTIONS.map((option) => (
          <button
            type="button"
            key={option.value}
            data-active={mode === option.value ? "true" : "false"}
            onClick={() => onChange({ credentialMode: option.value })}
            disabled={disabled}
          >
            {option.label}
          </button>
        ))}
      </div>
      {mode === "connectionString" ? (
        <SecretField
          label="Connection string"
          value={source.connectionString ?? ""}
          placeholder={connectionStringPlaceholder(source.engine)}
          onChange={(value) => onChange({ connectionString: value })}
          disabled={disabled}
        />
      ) : (
        <>
          <div className="settings-pane__database-grid settings-pane__database-grid--credentials">
            <label className="settings-pane__field">
              <span>Host</span>
              <input
                value={fields.host}
                placeholder="localhost"
                onChange={(event) => updateFields({ host: event.target.value })}
                disabled={disabled}
              />
            </label>
            <label className="settings-pane__field">
              <span>Port</span>
              <input
                type="number"
                min={1}
                max={65535}
                value={fields.port ?? ""}
                placeholder={String(defaultPortForEngine(source.engine) ?? "")}
                onChange={(event) =>
                  updateFields({ port: nullableNumberFromInput(event.target.value) })
                }
                disabled={disabled}
              />
            </label>
            <label className="settings-pane__field">
              <span>User</span>
              <input
                value={fields.user}
                placeholder="database user"
                autoComplete="off"
                onChange={(event) => updateFields({ user: event.target.value })}
                disabled={disabled}
              />
            </label>
            <label className="settings-pane__field">
              <span>Database</span>
              <input
                value={fields.database}
                placeholder="database name"
                onChange={(event) => updateFields({ database: event.target.value })}
                disabled={disabled}
              />
            </label>
          </div>
          <SecretField
            label="Password"
            value={fields.password}
            placeholder="Stored locally"
            onChange={(value) => updateFields({ password: value })}
            disabled={disabled}
          />
          <label className="settings-pane__field">
            <span>SSL / TLS</span>
            <LocalPicker
              value={fields.sslMode}
              options={DATABASE_SSL_OPTIONS}
              onSelect={(value) => updateFields({ sslMode: value as DatabaseSslMode })}
            />
          </label>
          {fields.sslMode === "disabled" && (
            <div className="settings-pane__database-warning">
              TLS is disabled for this network source. Only use this for trusted local networks.
            </div>
          )}
          {fields.sslMode === "strict" && (
            <label className="settings-pane__field">
              <span>Certificate / CA path</span>
              <input
                value={fields.sslCertificate ?? ""}
                placeholder="/path/to/ca.pem"
                onChange={(event) => updateFields({ sslCertificate: event.target.value })}
                disabled={disabled}
              />
            </label>
          )}
        </>
      )}
    </div>
  );
}

function SqliteFields({
  source,
  disabled,
  onChange,
}: {
  source: DatabaseSourceConfig;
  disabled: boolean;
  onChange: (patch: Partial<DatabaseSourceConfig>) => void;
}) {
  const sqlite = source.sqlite ?? { filePath: "", createIfMissing: false };
  const updateSqlite = (patch: Partial<typeof sqlite>) => {
    onChange({ sqlite: { ...sqlite, ...patch } });
  };
  const pickFile = async () => {
    try {
      const selected = await open({ directory: false, multiple: false });
      if (typeof selected === "string") updateSqlite({ filePath: selected });
    } catch {
      // User cancelled.
    }
  };
  const pickNewFile = async () => {
    try {
      const selected = await save({ defaultPath: "database.sqlite" });
      if (typeof selected === "string") {
        updateSqlite({ filePath: selected, createIfMissing: true });
      }
    } catch {
      // User cancelled.
    }
  };

  return (
    <div className="settings-pane__database-block">
      <div className="settings-pane__tool-group-head">
        <h2>SQLite file</h2>
        <span>Existing file or create on test/use</span>
      </div>
      <label className="settings-pane__field">
        <span>File path</span>
        <div className="settings-pane__database-file-row">
          <input
            value={sqlite.filePath}
            placeholder="/absolute/path/database.sqlite"
            onChange={(event) => updateSqlite({ filePath: event.target.value })}
            disabled={disabled}
          />
          <button type="button" className="settings-pane__btn" onClick={pickFile} disabled={disabled}>
            Browse
          </button>
          <button type="button" className="settings-pane__btn" onClick={pickNewFile} disabled={disabled}>
            New file
          </button>
        </div>
      </label>
      <ToggleRow
        label="Create if missing"
        hint="Allow the backend to create the SQLite file when testing or querying."
        checked={sqlite.createIfMissing}
        disabled={disabled}
        onToggle={() => updateSqlite({ createIfMissing: !sqlite.createIfMissing })}
      />
    </div>
  );
}

function SupabaseRestFields({
  source,
  disabled,
  onChange,
}: {
  source: DatabaseSourceConfig;
  disabled: boolean;
  onChange: (patch: Partial<DatabaseSourceConfig>) => void;
}) {
  const rest = source.supabaseRest ?? defaultSupabaseRest();
  const updateRest = (patch: Partial<typeof rest>) => {
    onChange({ supabaseRest: { ...rest, ...patch } });
  };

  return (
    <div className="settings-pane__database-block">
      <div className="settings-pane__tool-group-head">
        <h2>Supabase REST</h2>
        <span>PostgREST tables, views, and optional RPC</span>
      </div>
      <label className="settings-pane__field">
        <span>Project URL</span>
        <input
          value={rest.projectUrl}
          placeholder="https://project-ref.supabase.co"
          onChange={(event) => updateRest({ projectUrl: event.target.value })}
          disabled={disabled}
        />
      </label>
      <SecretField
        label="Anon key"
        value={rest.anonKey}
        placeholder="eyJ..."
        onChange={(value) => updateRest({ anonKey: value })}
        disabled={disabled}
      />
      <SecretField
        label="Service role key"
        value={rest.serviceRoleKey}
        placeholder="Optional; bypasses RLS"
        onChange={(value) => updateRest({ serviceRoleKey: value })}
        disabled={disabled}
      />
      <div className="settings-pane__database-warning">
        Service role keys bypass Supabase Row Level Security. Keep service-role use off unless
        you intentionally want privileged operations.
      </div>
      <ToggleRow
        label="Use service role key"
        hint="Use the privileged key for agent REST calls instead of the anon key."
        checked={rest.useServiceRole}
        disabled={disabled || !rest.serviceRoleKey}
        onToggle={() => updateRest({ useServiceRole: !rest.useServiceRole })}
      />
      <ToggleRow
        label="Allow RPC / SQL functions"
        hint="Let the agent call exposed Supabase RPC functions when it needs server-side SQL."
        checked={rest.allowRpc}
        disabled={disabled}
        onToggle={() => updateRest({ allowRpc: !rest.allowRpc })}
      />
    </div>
  );
}

function SecretField({
  label,
  value,
  placeholder,
  onChange,
  disabled,
}: {
  label: string;
  value: string;
  placeholder?: string;
  onChange: (value: string) => void;
  disabled?: boolean;
}) {
  const [revealed, setRevealed] = useState(false);

  useEffect(() => {
    setRevealed(false);
  }, [label]);

  return (
    <label className="settings-pane__tool-credential">
      <span className="settings-pane__tool-credential-label">{label}</span>
      <div className="settings-pane__tool-credential-field">
        <input
          type={revealed ? "text" : "password"}
          value={value}
          placeholder={placeholder}
          autoComplete="off"
          spellCheck={false}
          onChange={(event) => onChange(event.target.value)}
          disabled={disabled}
        />
        <div className="settings-pane__tool-credential-actions">
          <button
            type="button"
            className="settings-pane__icon-btn"
            onClick={() => setRevealed((current) => !current)}
            title={revealed ? "Hide secret" : "Show secret"}
            aria-label={revealed ? "Hide secret" : "Show secret"}
            disabled={disabled || !value}
          >
            <Icon
              icon={revealed ? "solar:eye-closed-linear" : "solar:eye-linear"}
              width={13}
              height={13}
            />
          </button>
        </div>
      </div>
    </label>
  );
}

function ToggleRow({
  label,
  hint,
  checked,
  disabled,
  onToggle,
}: {
  label: string;
  hint: string;
  checked: boolean;
  disabled?: boolean;
  onToggle: () => void;
}) {
  return (
    <div className="settings-pane__tool-toggle-row">
      <div className="settings-pane__tool-toggle-text">
        <span className="settings-pane__tool-toggle-label">{label}</span>
        <span className="settings-pane__tool-toggle-hint">{hint}</span>
      </div>
      <button
        type="button"
        className="settings-pane__switch"
        role="switch"
        aria-checked={checked}
        aria-label={label}
        data-on={checked ? "true" : "false"}
        disabled={disabled}
        onClick={onToggle}
      >
        <span className="settings-pane__switch-thumb" />
      </button>
    </div>
  );
}

function DatabaseActivityList({
  activity,
  loading,
  onRefresh,
  onClear,
}: {
  activity: DatabaseActivityEntry[];
  loading: boolean;
  onRefresh: () => void;
  onClear: () => void;
}) {
  return (
    <section className="settings-pane__database-activity">
      <div className="settings-pane__tool-group-head">
        <h2>Activity log</h2>
        <span>{loading ? "loading…" : `${activity.length} recent`}</span>
      </div>
      <div className="settings-pane__database-activity-actions">
        <button type="button" className="settings-pane__btn" onClick={onRefresh} disabled={loading}>
          <Icon icon="solar:refresh-linear" width={13} height={13} />
          <span>Refresh</span>
        </button>
        <button
          type="button"
          className="settings-pane__btn"
          data-tone="danger-ghost"
          onClick={onClear}
          disabled={loading || activity.length === 0}
        >
          Clear log
        </button>
      </div>
      <div className="settings-pane__database-activity-list">
        {activity.map((entry) => (
          <div key={entry.id} className="settings-pane__database-activity-row">
            <div className="settings-pane__database-activity-top">
              <span
                className="settings-pane__chip"
                data-tone={entry.status === "success" ? "ok" : entry.status === "pending" ? "pending" : "error"}
              >
                {entry.status}
              </span>
              <span>{formatTimestamp(entry.timestampMs)}</span>
              {entry.durationMs != null && <span>{entry.durationMs} ms</span>}
              {entry.rowsAffected != null && <span>{entry.rowsAffected} affected</span>}
              {entry.rowsReturned != null && <span>{entry.rowsReturned} rows</span>}
            </div>
            <code>{entry.queryPreview || entry.operation || "Database operation"}</code>
            {entry.error && <div className="settings-pane__provider-error">{entry.error}</div>}
          </div>
        ))}
        {!loading && activity.length === 0 && (
          <div className="settings-pane__muted">
            No activity yet. Executed agent queries will appear here with previews only.
          </div>
        )}
      </div>
    </section>
  );
}

function LocalPicker({ value, options, onSelect }: {
  value: string;
  options: PickerOption[];
  onSelect: (value: string) => void;
}) {
  const [openPicker, setOpenPicker] = useState(false);
  const selected = options.find((option) => option.value === value);

  return (
    <div className="settings-pane__picker">
      <button
        type="button"
        className="settings-pane__picker-btn"
        data-open={openPicker ? "true" : "false"}
        onClick={() => setOpenPicker((current) => !current)}
      >
        <span className="settings-pane__picker-label">
          {selected?.icon && <Icon icon={selected.icon} width={12} height={12} />}
          <span>{selected?.label ?? "—"}</span>
        </span>
        <Icon icon="solar:alt-arrow-down-linear" width={11} height={11} />
      </button>
      {openPicker && (
        <div className="settings-pane__picker-pop" role="menu">
          {options.map((option) => (
            <button
              type="button"
              key={option.value}
              className="settings-pane__picker-row"
              data-selected={option.value === value ? "true" : "false"}
              onClick={() => {
                onSelect(option.value);
                setOpenPicker(false);
              }}
            >
              <span className="settings-pane__picker-row-label">
                {option.icon && <Icon icon={option.icon} width={12} height={12} />}
                <span>{option.label}</span>
              </span>
              {option.value === value && (
                <Icon icon="solar:check-read-linear" width={12} height={12} />
              )}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
