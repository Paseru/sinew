const fs = require('fs');
let code = fs.readFileSync('C:\\dev\\sinew\\src\\components\\SettingsPane.tsx.bak', 'utf8');

// 1. Add onHide to ProviderCardProps
code = code.replace(
  "busyConnectLabel?: string;\n};",
  "busyConnectLabel?: string;\n  onHide?: () => void;\n};"
);

// 2. Add onHide to OpenRouterProviderCardProps
code = code.replace(
  "onChanged: () => void;\n};",
  "onChanged: () => void;\n  onHide?: () => void;\n};"
);

// 3. Add onHide to DeepSeekProviderCardProps
code = code.replace(
  "onChanged: () => void;\n};\n",
  "onChanged: () => void;\n  onHide?: () => void;\n};\n"
);

// 4. Update ProviderCard component to render onHide button
code = code.replace(
  "connectLabel,",
  "connectLabel,\n  onHide,"
);
code = code.replace(
  "busyConnectLabel,",
  "busyConnectLabel,\n  onHide,"
);

const hideBtnHtml = `
            <span className="settings-pane__chip" data-tone={statusTone}>
              <span className="settings-pane__chip-dot" />
              {statusLabel}
            </span>
            {onHide && !connected && !connecting && (
              <button
                type="button"
                onClick={(e) => { e.stopPropagation(); onHide(); }}
                className="settings-pane__btn"
                style={{ marginLeft: "auto", background: "transparent", border: "none", padding: "4px" }}
                title="Masquer ce fournisseur"
              >
                <Icon icon="lucide:eye-off" width={14} height={14} style={{ color: "var(--text-3)" }} />
              </button>
            )}
`;

code = code.replace(
  /<span className="settings-pane__chip" data-tone=\{statusTone\}>\s*<span className="settings-pane__chip-dot" \/>\s*\{statusLabel\}\s*<\/span>/,
  hideBtnHtml.trim()
);

// 5. Update OpenRouterProviderCard
code = code.replace(
  "onChanged,\n}: OpenRouterProviderCardProps",
  "onChanged,\n  onHide,\n}: OpenRouterProviderCardProps"
);
const orHideBtnHtml = `
          <span className="settings-pane__chip" data-tone={statusTone}>
            <span className="settings-pane__chip-dot" />
            {statusLabel}
          </span>
          {onHide && !displayStatus.connected && (
              <button
                type="button"
                onClick={(e) => { e.stopPropagation(); onHide(); }}
                className="settings-pane__btn"
                style={{ marginLeft: "auto", background: "transparent", border: "none", padding: "4px" }}
                title="Masquer ce fournisseur"
              >
                <Icon icon="lucide:eye-off" width={14} height={14} style={{ color: "var(--text-3)" }} />
              </button>
            )}
`;
code = code.replace(
  /<span className="settings-pane__chip" data-tone=\{statusTone\}>\s*<span className="settings-pane__chip-dot" \/>\s*\{statusLabel\}\s*<\/span>/,
  orHideBtnHtml.trim()
);

// 6. Update DeepSeekProviderCard
code = code.replace(
  "onChanged,\n}: DeepSeekProviderCardProps",
  "onChanged,\n  onHide,\n}: DeepSeekProviderCardProps"
);
const dsHideBtnHtml = `
          <span className="settings-pane__chip" data-tone={statusTone}>
            <span className="settings-pane__chip-dot" />
            {statusLabel}
          </span>
          {onHide && !displayStatus.connected && (
              <button
                type="button"
                onClick={(e) => { e.stopPropagation(); onHide(); }}
                className="settings-pane__btn"
                style={{ marginLeft: "auto", background: "transparent", border: "none", padding: "4px" }}
                title="Masquer ce fournisseur"
              >
                <Icon icon="lucide:eye-off" width={14} height={14} style={{ color: "var(--text-3)" }} />
              </button>
            )}
`;
code = code.replace(
  /<span className="settings-pane__chip" data-tone=\{statusTone\}>\s*<span className="settings-pane__chip-dot" \/>\s*\{statusLabel\}\s*<\/span>/,
  dsHideBtnHtml.trim()
);

// 7. Update ProvidersSection
const providersSectionRegex = /function ProvidersSection\(\{[\s\S]*?const cursorStatus: CursorComposerAuthStatus = \{/;

const psReplacement = `function ProvidersSection({
  openAiStatus,
  openAiAccounts,
  unconnectedAccounts,
  secondaryModels,
  onUpdateSecondaryModel,
  secondaryThinking,
  onUpdateSecondaryThinking,
  secondaryFast,
  onUpdateSecondaryFast,
  onSaveOpenAiAccessToken,
  onDisconnectOpenAiAccount,
  anthropicStatus,
  googleStatus,
  googleAccounts,
  unconnectedGoogleAccounts,
  onConnectGoogleWithKey,
  onAddGoogleAccount,
  onRemoveUnconnectedGoogleAccount,
  onDisconnectGoogleAccount,
  cursorComposerStatus,
  kimiStatus,
  deepSeekStatus,
  openRouterStatus,
  openRouterModels,
  loading,
  busy,
  message,
  onRefresh,
  onConnect,
  onConnectWithKey,
  onAddOpenAiAccount,
  onRemoveUnconnectedAccount,
  onCancel,
  onDisconnect,
  onConnectAnthropic,
  onCancelAnthropic,
  onDisconnectAnthropic,
  onConnectGoogle,
  onCancelGoogle,
  onDisconnectGoogle,
  onConnectCursor,
  onCancelCursorComposer,
  onDisconnectCursorComposer,
  onConnectKimi,
  onCancelKimi,
  onDisconnectKimi,
  onDisconnectOpenRouter,
  onOpenRouterStatusChange,
  onOpenRouterModelsChange,
  onOpenRouterChanged,
  onDisconnectDeepSeek,
  onDeepSeekStatusChange,
  onDeepSeekChanged,
}: ProvidersSectionProps) {
  const ALL_PROVIDERS = [
    { id: "openai", name: "OpenAI" },
    { id: "anthropic", name: "Anthropic" },
    { id: "google", name: "Google" },
    { id: "kimi", name: "Kimi" },
    { id: "cursor", name: "Cursor" },
    { id: "deepseek", name: "DeepSeek" },
    { id: "openrouter", name: "OpenRouter" },
  ];

  const [visibleProviders, setVisibleProviders] = useState<string[]>(() => {
    try {
      const stored = localStorage.getItem("sinew.visible-providers");
      if (stored) return JSON.parse(stored);
    } catch {}
    return ["openai", "google", "deepseek"];
  });

  const hideProvider = (id: string) => {
    const next = visibleProviders.filter((p) => p !== id);
    setVisibleProviders(next);
    localStorage.setItem("sinew.visible-providers", JSON.stringify(next));
  };

  const showProvider = (id: string) => {
    const next = [...new Set([...visibleProviders, id])];
    setVisibleProviders(next);
    localStorage.setItem("sinew.visible-providers", JSON.stringify(next));
  };

  const isConnected = (id: string) => {
    switch(id) {
      case "openai": return openAiStatus?.connected || openAiAccounts.length > 0 || unconnectedAccounts.length > 0;
      case "anthropic": return anthropicStatus?.connected;
      case "google": return googleStatus?.connected || googleAccounts.length > 0 || unconnectedGoogleAccounts.length > 0;
      case "kimi": return kimiStatus?.connected;
      case "cursor": return cursorComposerStatus?.connected;
      case "deepseek": return deepSeekStatus?.connected;
      case "openrouter": return openRouterStatus?.connected;
      default: return false;
    }
  };

  const shouldShow = (id: string) => visibleProviders.includes(id) || isConnected(id);
  const hiddenProvidersList = ALL_PROVIDERS.filter((p) => !shouldShow(p.id));

  const cursorStatus: CursorComposerAuthStatus = {`;

code = code.replace(providersSectionRegex, psReplacement);

// Update rendering of ProviderCards
// 1. Anthropic
code = code.replace(
  /<ProviderCard\s+name="Anthropic"[\s\S]*?providerId="anthropic"\s*\/>/,
  `{shouldShow("anthropic") && (<ProviderCard
          name="Anthropic"
          icon="simple-icons:anthropic"
          description="Use OAuth to connect your Claude account for Anthropic models."
          status={anthropicStatus}
          connectedMeta={["Claude OAuth"]}
          loading={loading}
          busy={busy}
          onConnect={onConnectAnthropic}
          onCancel={onCancelAnthropic}
          onDisconnect={onDisconnectAnthropic}
          providerId="anthropic"
          onHide={() => hideProvider("anthropic")}
        />)}`
);

// 2. OpenAI
code = code.replace(
  /<ProviderCard\s+name="OpenAI"[\s\S]*?<\/ProviderCard>/,
  `{shouldShow("openai") && (<ProviderCard
          name="OpenAI"
          icon="simple-icons:openai"
          description="Use OAuth to connect your ChatGPT account for OpenAI models."
          status={openAiStatus}
          connectedMeta={[
            openAiStatus?.email || "Signed in",
            openAiStatus?.planType ?? null,
          ]}
          loading={loading}
          busy={busy}
          onConnect={onConnect}
          onCancel={onCancel}
          onDisconnect={onDisconnect}
          showPlus={true}
          onPlus={onAddOpenAiAccount}
          providerId="openai"
          onHide={() => hideProvider("openai")}
        >
          {!openAiStatus?.connected && (
            <div style={{ display: "grid", gap: "8px", marginTop: "12px", padding: "10px", background: "var(--bg-2)", borderRadius: "6px", border: "1px solid var(--line-1)" }}>
              <span style={{ fontSize: "11px", fontWeight: "bold", color: "var(--text-2)", textTransform: "uppercase", letterSpacing: "0.05em" }}>
                Or paste a business access token:
              </span>
              <div style={{ display: "flex", gap: "8px" }}>
                <input
                  type="password"
                  placeholder="eyJhbGciOi..."
                  id="token-input-openai-main"
                  style={{
                    flex: 1,
                    background: "var(--bg-3)",
                    color: "var(--text-1)",
                    border: "1px solid var(--line-2)",
                    borderRadius: "4px",
                    padding: "6px 10px",
                    fontSize: "12px",
                    outline: "none"
                  }}
                />
                <button
                  type="button"
                  onClick={async () => {
                    const input = document.getElementById("token-input-openai-main") as HTMLInputElement;
                    const val = input?.value || "";
                    if (val.trim()) {
                      await onSaveOpenAiAccessToken(val, "openai");
                      onRefresh();
                    }
                  }}
                  style={{
                    background: "var(--accent-1, #9ca3af)",
                    color: "#fff",
                    border: "none",
                    borderRadius: "4px",
                    padding: "6px 12px",
                    fontSize: "12px",
                    fontWeight: 500,
                    cursor: "pointer"
                  }}
                >
                  Save
                </button>
              </div>
            </div>
          )}
        </ProviderCard>)}`
);

// 3. Cursor
code = code.replace(
  /<ProviderCard\s+name="Cursor"[\s\S]*?providerId="cursor"\s*\/>/,
  `{shouldShow("cursor") && (<ProviderCard
          name="Cursor"
          icon="local:cursor"
          description="Use Cursor composer auth to access specific models."
          status={cursorStatus}
          connectedMeta={["Cursor Auth"]}
          loading={loading}
          busy={busy}
          onConnect={onConnectCursor}
          onCancel={onCancelCursorComposer}
          onDisconnect={onDisconnectCursorComposer}
          providerId="cursor"
          onHide={() => hideProvider("cursor")}
        />)}`
);

// 4. Google
code = code.replace(
  /<ProviderCard\s+name="Google"[\s\S]*?providerId="google"\s*\/>/,
  `{shouldShow("google") && (<ProviderCard
          name="Google"
          icon="simple-icons:google"
          description="Use OAuth to connect your Google account for Gemini models."
          status={googleStatus}
          connectedMeta={[
            googleStatus?.email || "Signed in",
            googleStatus?.userTier ?? null,
            googleStatus?.projectId
              ? \`Project \${googleStatus.projectId}\`
              : null,
          ]}
          loading={loading}
          busy={busy}
          onConnect={onConnectGoogle}
          onCancel={onCancelGoogle}
          onDisconnect={onDisconnectGoogle}
          showPlus={true}
          onPlus={onAddGoogleAccount}
          providerId="google"
          onHide={() => hideProvider("google")}
        />)}`
);

// 5. Kimi
code = code.replace(
  /<ProviderCard\s+name="Kimi"[\s\S]*?providerId="kimi"\s*\/>/,
  `{shouldShow("kimi") && (<ProviderCard
          name="Kimi"
          icon="local:kimi"
          description="Use OAuth to connect your Kimi account for Kimi 2.6."
          status={kimiStatus}
          connectedMeta={["Kimi OAuth"]}
          loading={loading}
          busy={busy}
          onConnect={onConnectKimi}
          onCancel={onCancelKimi}
          onDisconnect={onDisconnectKimi}
          providerId="kimi"
          onHide={() => hideProvider("kimi")}
        />)}`
);

// 6. DeepSeek
code = code.replace(
  /<DeepSeekProviderCard[\s\S]*?onChanged=\{onDeepSeekChanged\}\s*\/>/,
  `{shouldShow("deepseek") && (<DeepSeekProviderCard
          status={deepSeekStatus}
          loading={loading}
          busy={busy}
          onDisconnect={onDisconnectDeepSeek}
          onStatusChange={onDeepSeekStatusChange}
          onChanged={onDeepSeekChanged}
          onHide={() => hideProvider("deepseek")}
        />)}`
);

// 7. OpenRouter
code = code.replace(
  /<OpenRouterProviderCard[\s\S]*?onChanged=\{onOpenRouterChanged\}\s*\/>/,
  `{shouldShow("openrouter") && (<OpenRouterProviderCard
          status={openRouterStatus}
          models={openRouterModels}
          loading={loading}
          busy={busy}
          onDisconnect={onDisconnectOpenRouter}
          onStatusChange={onOpenRouterStatusChange}
          onModelsChange={onOpenRouterModelsChange}
          onChanged={onOpenRouterChanged}
          onHide={() => hideProvider("openrouter")}
        />)}
        {hiddenProvidersList.length > 0 && (
          <div style={{ marginTop: "16px", padding: "16px", borderTop: "1px solid var(--line-1)", display: "flex", alignItems: "center", gap: "12px", flexWrap: "wrap" }}>
            <span style={{ fontSize: "12px", color: "var(--text-2)" }}>Ajouter un fournisseur :</span>
            {hiddenProvidersList.map((p) => (
              <button
                key={p.id}
                type="button"
                className="settings-pane__btn"
                onClick={() => showProvider(p.id)}
              >
                <Icon icon="lucide:plus" width={14} height={14} />
                {p.name}
              </button>
            ))}
          </div>
        )}`
);

fs.writeFileSync('C:\\dev\\sinew\\src\\components\\SettingsPane.tsx', code);
