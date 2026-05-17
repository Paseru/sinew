import { useCallback, useEffect, useState } from "react";
import { Icon } from "@iconify/react";
import { api } from "../lib/ipc";
import { useLanguage } from "../lib/i18n";
import { markOnboardingDismissed } from "../lib/onboarding";

const PROVIDERS_CHANGED_EVENT = "sinew:providers-changed";

type Props = {
  onOpenProviders: () => void;
  onFinish: () => void;
};

export function WorkspaceOnboarding({ onOpenProviders, onFinish }: Props) {
  const language = useLanguage();
  const copy = workspaceOnboardingCopy[language];
  const [configuredProviders, setConfiguredProviders] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);

  const refreshProviders = useCallback(async () => {
    setLoading(true);
    try {
      setConfiguredProviders(await api.listConfiguredModelProviders());
    } catch {
      setConfiguredProviders([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refreshProviders();
    window.addEventListener(PROVIDERS_CHANGED_EVENT, refreshProviders);
    return () => {
      window.removeEventListener(PROVIDERS_CHANGED_EVENT, refreshProviders);
    };
  }, [refreshProviders]);

  const hasProvider = configuredProviders.length > 0;

  const finishOnboarding = () => {
    markOnboardingDismissed();
    onFinish();
  };

  return (
    <aside className="workspace-onboarding" data-ready={hasProvider ? "true" : "false"}>
      <div className="workspace-onboarding__head">
        <div>
          <span className="workspace-onboarding__eyebrow">{copy.eyebrow}</span>
          <h2>{copy.title}</h2>
        </div>
        <button
          type="button"
          className="workspace-onboarding__close"
          onClick={finishOnboarding}
          aria-label={copy.hideGuide}
        >
          <Icon icon="solar:close-circle-linear" width={18} height={18} />
        </button>
      </div>

      <div className="workspace-onboarding__steps">
        <div className="workspace-onboarding__step" data-state="done">
          <span className="workspace-onboarding__dot">
            <Icon icon="solar:check-circle-bold" width={16} height={16} />
          </span>
          <div>
            <h3>{copy.workspaceOpened}</h3>
            <p>{copy.workspaceOpenedText}</p>
          </div>
        </div>

        <div className="workspace-onboarding__step" data-state={hasProvider ? "done" : "active"}>
          <span className="workspace-onboarding__dot">
            <Icon
              icon={hasProvider ? "solar:check-circle-bold" : "solar:cloud-check-linear"}
              width={16}
              height={16}
            />
          </span>
          <div>
            <h3>{copy.connectProvider}</h3>
            <p>{copy.connectProviderText}</p>
          </div>
        </div>

        <div className="workspace-onboarding__step" data-state={hasProvider ? "active" : "idle"}>
          <span className="workspace-onboarding__dot">
            <Icon icon="solar:chat-round-dots-linear" width={16} height={16} />
          </span>
          <div>
            <h3>{copy.firstRun}</h3>
            <p>{copy.firstRunText}</p>
          </div>
        </div>
      </div>

      <div className="workspace-onboarding__actions">
        <button
          type="button"
          className="workspace-onboarding__primary"
          onClick={hasProvider ? finishOnboarding : onOpenProviders}
        >
          <Icon
            icon={hasProvider ? "solar:bolt-circle-linear" : "solar:settings-linear"}
            width={16}
            height={16}
          />
          <span>{hasProvider ? copy.startUsing : copy.openProviders}</span>
        </button>
        {!hasProvider && (
          <button
            type="button"
            className="workspace-onboarding__secondary"
            onClick={refreshProviders}
            disabled={loading}
          >
            {loading ? copy.checking : copy.connectedOne}
          </button>
        )}
      </div>
    </aside>
  );
}
const workspaceOnboardingCopy = {
  en: {
    eyebrow: "Setup guide",
    title: "Connect Sinew to a model.",
    hideGuide: "Hide setup guide",
    workspaceOpened: "Workspace opened",
    workspaceOpenedText: "Your project is loaded and ready.",
    connectProvider: "Connect a model provider",
    connectProviderText:
      "Open Settings, choose Providers, then connect Anthropic, OpenAI, Google, Kimi, or OpenRouter.",
    firstRun: "Start your first run",
    firstRunText: "Pick a mode in chat and ask Sinew to inspect or change the project.",
    startUsing: "Start using Sinew",
    openProviders: "Open Providers",
    checking: "Checking...",
    connectedOne: "I connected one",
  },
  fr: {
    eyebrow: "Guide de configuration",
    title: "Connectez Sinew \u00e0 un mod\u00e8le.",
    hideGuide: "Masquer le guide",
    workspaceOpened: "Espace de travail ouvert",
    workspaceOpenedText: "Votre projet est charg\u00e9 et pr\u00eat.",
    connectProvider: "Connecter un fournisseur de mod\u00e8les",
    connectProviderText:
      "Ouvrez les param\u00e8tres, choisissez Mod\u00e8les, puis connectez Anthropic, OpenAI, Google, Kimi ou OpenRouter.",
    firstRun: "Lancer la premi\u00e8re session",
    firstRunText: "Choisissez un mode dans le chat et demandez \u00e0 Sinew d'inspecter ou modifier le projet.",
    startUsing: "Commencer avec Sinew",
    openProviders: "Ouvrir Mod\u00e8les",
    checking: "V\u00e9rification...",
    connectedOne: "J'en ai connect\u00e9 un",
  },
} as const;
