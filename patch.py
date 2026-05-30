import re

with open(r'C:\dev\sinew\src\components\SettingsPane.tsx', 'r', encoding='utf-8') as f:
    code = f.read()

# 1. ProviderCardProps
code = code.replace(
    '  busyConnectLabel?: string;\n};',
    '  busyConnectLabel?: string;\n  onHide?: () => void;\n};'
)

# 2. OpenRouterProviderCardProps
code = code.replace(
    '  onChanged: () => void;\n};\n\nfunction OpenRouterProviderCard({',
    '  onChanged: () => void;\n  onHide?: () => void;\n};\n\nfunction OpenRouterProviderCard({'
)
code = code.replace(
    '  onChanged,\n}: OpenRouterProviderCardProps',
    '  onChanged,\n  onHide,\n}: OpenRouterProviderCardProps'
)

# 3. DeepSeekProviderCardProps
code = code.replace(
    '  onChanged: () => void;\n};\n\nfunction DeepSeekProviderCard({',
    '  onChanged: () => void;\n  onHide?: () => void;\n};\n\nfunction DeepSeekProviderCard({'
)
code = code.replace(
    '  onChanged,\n}: DeepSeekProviderCardProps',
    '  onChanged,\n  onHide,\n}: DeepSeekProviderCardProps'
)

# 4. Update ProviderCard component function signature
code = code.replace(
    '  busyConnectLabel,\n}: ProviderCardProps',
    '  busyConnectLabel,\n  onHide,\n}: ProviderCardProps'
)

# 5. Inject onHide button into ProviderCard
hideBtn1 = """              {statusLabel}
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
            )}"""
code = code.replace(
    '              {statusLabel}\n            </span>',
    hideBtn1,
    1
)

# 6. Inject onHide button into OpenRouterProviderCard
hideBtn2 = """              {statusLabel}
            </span>
            {onHide && !displayStatus.connected && !connecting && (
              <button
                type="button"
                onClick={(e) => { e.stopPropagation(); onHide(); }}
                className="settings-pane__btn"
                style={{ marginLeft: "auto", background: "transparent", border: "none", padding: "4px" }}
                title="Masquer ce fournisseur"
              >
                <Icon icon="lucide:eye-off" width={14} height={14} style={{ color: "var(--text-3)" }} />
              </button>
            )}"""
# Wait, OpenRouterProviderCard has its own statusLabel block. We must find it correctly.
# It's the next statusLabel block. Let's just find them by counting.
# But it's easier to replace all of them properly.
