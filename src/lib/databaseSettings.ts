import type {
  DatabaseActivityEntry,
  DatabaseActivityStatus,
  DatabaseConnectionFields,
  DatabaseConnectionStatusState,
  DatabaseCredentialMode,
  DatabaseEngine,
  DatabaseSettings,
  DatabaseSourceConfig,
  DatabaseSslMode,
  DatabaseSupabaseRestConfig,
} from "../types";

const DATABASE_ENGINES: DatabaseEngine[] = [
  "postgres",
  "mysql",
  "sqlite",
  "mssql",
  "supabaseRest",
];

const DIRECT_SQL_ENGINES = new Set<DatabaseEngine>([
  "postgres",
  "mysql",
  "mssql",
]);

export function createDatabaseSource(
  engine: DatabaseEngine,
  existing: DatabaseSourceConfig[],
): DatabaseSourceConfig {
  const index = existing.length + 1;
  const baseName = uniqueDatabaseName(defaultNameForEngine(engine), existing);
  const source: DatabaseSourceConfig = {
    id: randomDatabaseId(index),
    name: baseName,
    engine,
    enabled: true,
    note: "",
    defaultSchema: defaultSchemaForEngine(engine),
    defaultRowLimit: 1000,
    defaultTimeoutMs: 30000,
    readOnly: false,
    requireConfirmationForDestructive: true,
    lastConnectionStatus: { status: "untested" },
  };

  if (engine === "sqlite") {
    source.sqlite = { filePath: "", createIfMissing: false };
  } else if (engine === "supabaseRest") {
    source.supabaseRest = defaultSupabaseRest();
  } else {
    source.credentialMode = "connectionString";
    source.connectionString = "";
    source.fields = defaultDatabaseFields(engine);
  }

  return source;
}

export type EnvDatabaseDetection = {
  engine: DatabaseEngine;
  suggestedName: string;
  preview: string;
  prefill: Partial<DatabaseSourceConfig>;
};

/**
 * Parse a pasted `.env` content and detect database connection candidates.
 * Never returns raw secrets in `preview`.
 */
export function parseEnvForDatabaseSources(input: string): EnvDatabaseDetection[] {
  const detections: EnvDatabaseDetection[] = [];
  if (typeof input !== "string" || input.trim().length === 0) {
    return detections;
  }
  const env = new Map<string, string>();
  for (const rawLine of input.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line || line.startsWith("#")) continue;
    const eq = line.indexOf("=");
    if (eq <= 0) continue;
    const key = line.slice(0, eq).trim();
    let value = line.slice(eq + 1).trim();
    if (
      (value.startsWith("\"") && value.endsWith("\"")) ||
      (value.startsWith("'") && value.endsWith("'"))
    ) {
      value = value.slice(1, -1);
    }
    if (!key || !value) continue;
    env.set(key, value);
  }

  const supabaseUrl = pickEnv(env, [
    "SUPABASE_URL",
    "NEXT_PUBLIC_SUPABASE_URL",
    "PUBLIC_SUPABASE_URL",
    "VITE_SUPABASE_URL",
  ]);
  const supabaseAnon = pickEnv(env, [
    "SUPABASE_ANON_KEY",
    "NEXT_PUBLIC_SUPABASE_ANON_KEY",
    "PUBLIC_SUPABASE_ANON_KEY",
    "VITE_SUPABASE_ANON_KEY",
  ]);
  const supabaseService = pickEnv(env, [
    "SUPABASE_SERVICE_ROLE_KEY",
    "SUPABASE_SERVICE_KEY",
    "SUPABASE_SECRET_KEY",
  ]);
  if (supabaseUrl) {
    detections.push({
      engine: "supabaseRest",
      suggestedName: "Supabase",
      preview: `Supabase REST → ${supabaseUrl}${supabaseAnon ? " (anon key)" : ""}${supabaseService ? " · service role" : ""}`,
      prefill: {
        engine: "supabaseRest",
        supabaseRest: {
          projectUrl: supabaseUrl,
          anonKey: supabaseAnon ?? "",
          serviceRoleKey: supabaseService ?? "",
          useServiceRole: false,
          allowRpc: false,
        },
      },
    });
  }

  for (const [key, value] of env.entries()) {
    if (!value || key.startsWith("SUPABASE_")) continue;
    if (/^postgres(ql)?:\/\//i.test(value)) {
      detections.push({
        engine: "postgres",
        suggestedName: friendlyNameFromKey(key, "Postgres"),
        preview: `Postgres → ${redactConnectionString(value)} (from ${key})`,
        prefill: {
          engine: "postgres",
          credentialMode: "connectionString",
          connectionString: value,
        },
      });
      continue;
    }
    if (/^mysql:\/\//i.test(value)) {
      detections.push({
        engine: "mysql",
        suggestedName: friendlyNameFromKey(key, "MySQL"),
        preview: `MySQL → ${redactConnectionString(value)} (from ${key})`,
        prefill: {
          engine: "mysql",
          credentialMode: "connectionString",
          connectionString: value,
        },
      });
      continue;
    }
    if (/^mssql:\/\/|Server=.+;Database=/i.test(value)) {
      detections.push({
        engine: "mssql",
        suggestedName: friendlyNameFromKey(key, "MSSQL"),
        preview: `MSSQL → ${redactConnectionString(value)} (from ${key})`,
        prefill: {
          engine: "mssql",
          credentialMode: "connectionString",
          connectionString: value,
        },
      });
      continue;
    }
    if (/\.(db|sqlite|sqlite3)$/i.test(value) && !value.includes("://")) {
      detections.push({
        engine: "sqlite",
        suggestedName: friendlyNameFromKey(key, "SQLite"),
        preview: `SQLite → ${value} (from ${key})`,
        prefill: {
          engine: "sqlite",
          sqlite: { filePath: value, createIfMissing: false },
        },
      });
    }
  }

  return dedupeDetections(detections);
}

export function createDatabaseSourceFromTemplate(
  template: Partial<DatabaseSourceConfig>,
  suggestedName: string,
  existing: DatabaseSourceConfig[],
): DatabaseSourceConfig {
  const engine = normalizeEngine(template.engine ?? "postgres");
  const base = createDatabaseSource(engine, existing);
  const merged: DatabaseSourceConfig = {
    ...base,
    ...template,
    engine,
    id: base.id,
    name: uniqueDatabaseName(suggestedName || base.name, existing),
    enabled: true,
    lastConnectionStatus: { status: "untested" },
  };
  if (engine === "sqlite") {
    merged.sqlite = {
      filePath: template.sqlite?.filePath ?? base.sqlite?.filePath ?? "",
      createIfMissing:
        template.sqlite?.createIfMissing ?? base.sqlite?.createIfMissing ?? false,
    };
    delete merged.connectionString;
    delete merged.credentialMode;
    delete merged.fields;
    delete merged.supabaseRest;
  } else if (engine === "supabaseRest") {
    merged.supabaseRest = {
      ...defaultSupabaseRest(),
      ...(template.supabaseRest ?? {}),
    };
    delete merged.connectionString;
    delete merged.credentialMode;
    delete merged.fields;
    delete merged.sqlite;
  } else {
    merged.credentialMode = template.credentialMode ?? "connectionString";
    merged.connectionString = template.connectionString ?? "";
    merged.fields = template.fields ?? defaultDatabaseFields(engine);
    delete merged.sqlite;
    delete merged.supabaseRest;
  }
  return merged;
}

function pickEnv(env: Map<string, string>, keys: string[]): string | null {
  for (const key of keys) {
    const value = env.get(key);
    if (value && value.trim().length > 0) return value.trim();
  }
  return null;
}

function friendlyNameFromKey(key: string, fallback: string): string {
  const cleaned = key
    .replace(/^(NEXT_PUBLIC|PUBLIC|VITE)_/i, "")
    .replace(/_(URL|DSN|DATABASE_URL|CONNECTION_STRING|CONN|URI)$/i, "")
    .replace(/_/g, " ")
    .trim();
  if (!cleaned) return fallback;
  return cleaned
    .toLowerCase()
    .split(/\s+/)
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(" ");
}

function redactConnectionString(value: string): string {
  try {
    if (!/^[a-z]+:\/\//i.test(value)) {
      return value
        .replace(/(Password|Pwd)\s*=\s*([^;]+)/gi, "$1=***")
        .replace(/(User\s*Id|UID)\s*=\s*([^;]+)/gi, "$1=***");
    }
    const url = new URL(value);
    if (url.password) url.password = "***";
    if (url.username) url.username = `${url.username.slice(0, 2)}***`;
    return url.toString();
  } catch {
    return "(masked connection string)";
  }
}

function dedupeDetections(detections: EnvDatabaseDetection[]): EnvDatabaseDetection[] {
  const seen = new Set<string>();
  const out: EnvDatabaseDetection[] = [];
  for (const item of detections) {
    const key = `${item.engine}|${item.preview}`;
    if (seen.has(key)) continue;
    seen.add(key);
    out.push(item);
  }
  return out;
}

export function normalizeDatabaseSettings(settings: DatabaseSettings): DatabaseSettings {
  const seenIds = new Map<string, number>();
  return {
    sources: (settings.sources ?? []).map((source, index) => {
      const normalized = normalizeDatabaseSource(source, index);
      const count = seenIds.get(normalized.id) ?? 0;
      seenIds.set(normalized.id, count + 1);
      return count === 0
        ? normalized
        : { ...normalized, id: `${normalized.id}-${count + 1}` };
    }),
  };
}

export function normalizeDatabaseSource(
  source: Partial<DatabaseSourceConfig>,
  index: number,
): DatabaseSourceConfig {
  const engine = normalizeEngine(source.engine);
  const fallback = createDatabaseSource(engine, []);
  const status = normalizeStatus(source.lastConnectionStatus?.status);
  const next: DatabaseSourceConfig = {
    ...fallback,
    ...source,
    id: stringValue(source.id) || randomDatabaseId(index + 1),
    name: stringValue(source.name) || `${defaultNameForEngine(engine)} ${index + 1}`,
    engine,
    enabled: source.enabled !== false,
    note: source.note ?? "",
    defaultSchema: source.defaultSchema ?? defaultSchemaForEngine(engine),
    defaultRowLimit: clampPositiveInteger(source.defaultRowLimit, 1000, 100000),
    defaultTimeoutMs: clampPositiveInteger(source.defaultTimeoutMs, 30000, 300000),
    readOnly: source.readOnly === true,
    requireConfirmationForDestructive:
      source.requireConfirmationForDestructive !== false,
    lastConnectionStatus: {
      status,
      message: source.lastConnectionStatus?.message ?? null,
      timestampMs: source.lastConnectionStatus?.timestampMs ?? null,
    },
  };

  if (engine === "sqlite") {
    next.credentialMode = undefined;
    next.connectionString = undefined;
    next.fields = undefined;
    next.supabaseRest = undefined;
    next.sqlite = {
      filePath: source.sqlite?.filePath ?? "",
      createIfMissing: source.sqlite?.createIfMissing === true,
    };
  } else if (engine === "supabaseRest") {
    next.credentialMode = undefined;
    next.connectionString = undefined;
    next.fields = undefined;
    next.sqlite = undefined;
    next.supabaseRest = {
      ...defaultSupabaseRest(),
      ...(source.supabaseRest ?? {}),
      useServiceRole: source.supabaseRest?.useServiceRole === true,
      allowRpc: source.supabaseRest?.allowRpc === true,
    };
  } else {
    next.sqlite = undefined;
    next.supabaseRest = undefined;
    next.credentialMode = normalizeCredentialMode(source.credentialMode);
    next.connectionString = source.connectionString ?? "";
    next.fields = normalizeDatabaseFields(source.fields, engine);
  }

  return next;
}

export function validateDatabaseSettings(settings: DatabaseSettings): void {
  const seenNames = new Set<string>();
  for (const source of settings.sources) {
    const name = source.name.trim();
    if (!name) throw new Error("Every database source needs a name.");
    const key = name.toLowerCase();
    if (seenNames.has(key)) {
      throw new Error(`Database source names must be unique: ${name}`);
    }
    seenNames.add(key);
    if (source.defaultRowLimit < 1) {
      throw new Error(`${name}: default row limit must be at least 1.`);
    }
    if (source.defaultTimeoutMs < 100) {
      throw new Error(`${name}: timeout must be at least 100 ms.`);
    }
  }
}

export function databaseSettingsFingerprint(settings: DatabaseSettings): string {
  return JSON.stringify(normalizeDatabaseSettings(settings));
}

export function databaseSettingsEventDetail(settings: DatabaseSettings) {
  return {
    activeCount: settings.sources.filter((source) => source.enabled).length,
    sourceCount: settings.sources.length,
  };
}

export function normalizeDatabaseActivityEntry(
  entry: DatabaseActivityEntry,
): DatabaseActivityEntry {
  return {
    id: stringValue(entry.id) || `activity-${entry.timestampMs || Date.now()}`,
    sourceId: stringValue(entry.sourceId),
    timestampMs: Number.isFinite(entry.timestampMs) ? entry.timestampMs : Date.now(),
    status: normalizeActivityStatus(entry.status),
    operation: entry.operation ?? null,
    queryPreview: redactGenericSecretText(entry.queryPreview ?? ""),
    durationMs: entry.durationMs ?? null,
    rowsAffected: entry.rowsAffected ?? null,
    rowsReturned: entry.rowsReturned ?? null,
    error: entry.error ? redactGenericSecretText(entry.error) : null,
  };
}

export function defaultDatabaseFields(engine: DatabaseEngine): DatabaseConnectionFields {
  return {
    host: "",
    port: defaultPortForEngine(engine),
    user: "",
    password: "",
    database: "",
    sslMode: DIRECT_SQL_ENGINES.has(engine) ? "required" : "disabled",
    sslCertificate: "",
  };
}

export function defaultSupabaseRest(): DatabaseSupabaseRestConfig {
  return {
    projectUrl: "",
    anonKey: "",
    serviceRoleKey: "",
    useServiceRole: false,
    allowRpc: false,
  };
}

export function defaultPortForEngine(engine: DatabaseEngine): number | null {
  if (engine === "postgres") return 5432;
  if (engine === "mysql") return 3306;
  if (engine === "mssql") return 1433;
  return null;
}

export function databaseEngineLabel(engine: DatabaseEngine): string {
  if (engine === "postgres") return "PostgreSQL";
  if (engine === "mysql") return "MySQL / MariaDB";
  if (engine === "sqlite") return "SQLite";
  if (engine === "mssql") return "Microsoft SQL Server";
  return "Supabase REST";
}

export function databaseEngineShortLabel(engine: DatabaseEngine): string {
  if (engine === "postgres") return "Postgres";
  if (engine === "mysql") return "MySQL";
  if (engine === "sqlite") return "SQLite";
  if (engine === "mssql") return "MSSQL";
  return "Supabase";
}

export function sourceStatusTone(
  source: DatabaseSourceConfig,
): "ok" | "error" | "pending" | "off" {
  if (!source.enabled) return "off";
  const status = source.lastConnectionStatus.status;
  if (status === "ok") return "ok";
  if (status === "error") return "error";
  return "pending";
}

export function sourceStatusLabel(source: DatabaseSourceConfig): string {
  if (!source.enabled) return "disabled";
  const status = source.lastConnectionStatus.status;
  if (status === "ok") return "OK";
  if (status === "error") return "error";
  return "not tested";
}

export function connectionStringPlaceholder(engine: DatabaseEngine): string {
  if (engine === "postgres") return "postgres://user:password@host:5432/db";
  if (engine === "mysql") return "mysql://user:password@host:3306/db";
  if (engine === "mssql") return "sqlserver://user:password@host:1433/db";
  return "";
}

export function isDirectSqlEngine(engine: DatabaseEngine): boolean {
  return DIRECT_SQL_ENGINES.has(engine);
}

export function numberFromInput(value: string, fallback: number): number {
  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) ? parsed : fallback;
}

export function nullableNumberFromInput(value: string): number | null {
  if (value.trim() === "") return null;
  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) ? parsed : null;
}

export function formatTimestamp(timestampMs: number): string {
  if (!Number.isFinite(timestampMs) || timestampMs <= 0) return "—";
  return new Date(timestampMs).toLocaleString();
}

export function sanitizeDatabaseMessage(
  message: string,
  subject: DatabaseSettings | DatabaseSourceConfig,
): string {
  let sanitized = redactGenericSecretText(message);
  const sources = "sources" in subject ? subject.sources : [subject];
  for (const source of sources) {
    const secrets = knownSecrets(source);
    for (const secret of secrets) {
      sanitized = sanitized.split(secret).join("••••");
    }
  }
  return sanitized;
}

export function errorMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

function normalizeDatabaseFields(
  fields: DatabaseConnectionFields | undefined,
  engine: DatabaseEngine,
): DatabaseConnectionFields {
  const fallback = defaultDatabaseFields(engine);
  return {
    host: fields?.host ?? fallback.host,
    port: fields?.port ?? fallback.port,
    user: fields?.user ?? fallback.user,
    password: fields?.password ?? fallback.password,
    database: fields?.database ?? fallback.database,
    sslMode: normalizeSslMode(fields?.sslMode),
    sslCertificate: fields?.sslCertificate ?? "",
  };
}

function normalizeEngine(engine: unknown): DatabaseEngine {
  if (engine === "postgresql") return "postgres";
  if (engine === "supabase_rest" || engine === "supabaseREST") return "supabaseRest";
  return DATABASE_ENGINES.includes(engine as DatabaseEngine)
    ? (engine as DatabaseEngine)
    : "postgres";
}

function normalizeCredentialMode(mode: unknown): DatabaseCredentialMode {
  return mode === "fields" ? "fields" : "connectionString";
}

function normalizeSslMode(mode: unknown): DatabaseSslMode {
  if (mode === "disabled" || mode === "strict") return mode;
  return "required";
}

function normalizeStatus(status: unknown): DatabaseConnectionStatusState {
  if (status === "ok" || status === "error") return status;
  return "untested";
}

function normalizeActivityStatus(status: unknown): DatabaseActivityStatus {
  if (
    status === "success" ||
    status === "error" ||
    status === "cancelled" ||
    status === "pending"
  ) {
    return status;
  }
  return "pending";
}

function clampPositiveInteger(
  value: unknown,
  fallback: number,
  max: number,
): number {
  const parsed = typeof value === "number" ? value : Number.parseInt(String(value ?? ""), 10);
  if (!Number.isFinite(parsed) || parsed < 1) return fallback;
  return Math.min(Math.floor(parsed), max);
}

function stringValue(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
}

function randomDatabaseId(index: number): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return `database-${Date.now()}-${index}`;
}

function defaultSchemaForEngine(engine: DatabaseEngine): string {
  if (engine === "postgres" || engine === "supabaseRest") return "public";
  if (engine === "mssql") return "dbo";
  return "";
}

function defaultNameForEngine(engine: DatabaseEngine): string {
  if (engine === "postgres") return "postgres_source";
  if (engine === "mysql") return "mysql_source";
  if (engine === "sqlite") return "sqlite_source";
  if (engine === "mssql") return "mssql_source";
  return "supabase_source";
}

function uniqueDatabaseName(
  base: string,
  existing: DatabaseSourceConfig[],
): string {
  const taken = new Set(existing.map((source) => source.name.trim().toLowerCase()));
  if (!taken.has(base.toLowerCase())) return base;
  let counter = 2;
  let candidate = `${base}_${counter}`;
  while (taken.has(candidate.toLowerCase())) {
    counter += 1;
    candidate = `${base}_${counter}`;
  }
  return candidate;
}

function knownSecrets(source: DatabaseSourceConfig): string[] {
  return [
    source.connectionString,
    source.fields?.password,
    source.supabaseRest?.anonKey,
    source.supabaseRest?.serviceRoleKey,
  ].filter((value): value is string => Boolean(value && value.length >= 4));
}

function redactGenericSecretText(value: string): string {
  return value
    .replace(/(:\/\/[^\s:/]+:)([^@\s]+)(@)/g, "$1••••$3")
    .replace(/(password|passwd|pwd|api[_-]?key|service[_-]?role[_-]?key)=([^\s;&]+)/gi, "$1=••••")
    .replace(/(Authorization:\s*Bearer\s+)([^\s]+)/gi, "$1••••");
}
