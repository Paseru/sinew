# Changelog
## [Unreleased] - 2026-05-30 01:05:00

### Improved
- **Reglages adaptes a chaque PC (`crates/sinew-index/src/store.rs`)** : remplacement des tailles fixes de cache SQLite par un calcul base sur les coeurs disponibles. Sinew s'adapte ainsi automatiquement au PC fixe, au portable ou a une future machine sans viser une configuration precise.
- **Tracabilite (`CHANGELOG.md`)** : ajout de cette entree pour documenter l'ajustement general demande.

## [Unreleased] — 2026-05-30

### Added
- **Contrôle unique du projet (`scripts/check.ps1`, `package.json`)** : `npm run check` lance le build, `cargo check`, `clippy`, les tests, et les audits npm en une commande.
- **Synchronisation automatique des jetons IA via OneDrive (`src-tauri/src/lib.rs`)** : Copie automatique des fichiers de connexion OAuth/clés au démarrage, à la fermeture et en synchro forcée. Les comptes IA restent connectés sur tous les PC.
- **Identification universelle par URL Git (`src-tauri/src/workspace.rs`, `src-tauri/src/conversations.rs`)** : L'URL distante Git sert d'identifiant de projet unique. Migration automatique des anciennes conversations.
- **Synchronisation forcée à la demande (`src-tauri/src/lib.rs`, `src/components/SettingsPane.tsx`)** : Bouton "Synchroniser maintenant" dans les paramètres + commande Tauri `force_multi_pc_sync`.
- **Compaction automatique au changement de fournisseur IA (`crates/sinew-app/src/agent/compaction.rs`)** : Fiche de transmission structurée (Git status, tâches, diagnostics) lors du changement d'IA.

### Changed
- **Rapport d'analyse complet (`afaire.md`)** : Réécriture intégrale avec diagnostic architecture, qualité, sécurité, et plan d'action priorisé respectant la frontière upstream/fork.
- **Tests live Cursor séparés (`crates/sinew-cursor/src/tests.rs`)** : Ignorés par défaut dans les contrôles courants, lancés par scripts dédiés.
- **Option de mise à jour à 3 choix (`src/components/SettingsPane.tsx`)** : Bloquant / Notification uniquement / Désactivé.
- **Fermeture lightbox au clic extérieur (`src/components/chat/ChatPane.tsx`)**.
- **Maquettes visuelles assouplies (`src-tauri/src/turns.rs`)** : Plus obligatoires, seulement si demandé ou nécessaire.

### Fixed
- **Stabilisation du test terminal (`crates/sinew-app/src/bash.rs`)** : Le test interactif attend correctement la fin de session.
- **Ouverture de fichiers dans l'éditeur (`src/components/Workspace.tsx`)** : Correction du bug "Aucun fichier ouvert" via référence stable.
- **Normalisation des chemins Windows (`crates/sinew-app/src/workspace.rs`, `crates/sinew-app/src/read.rs`)** : Correction casse et préfixe UNC qui causaient de fausses erreurs.
- **Reprise de conversation avec Google Gemini (`crates/sinew-google/src/client.rs`)** : Résolution erreur INVALID_ARGUMENT sur les tool_call_id.
- **Reprise de conversation avec DeepSeek (`crates/sinew-deepseek/src/client.rs`)** : Correction du champ reasoning_content manquant.
- **Sensibilité à la casse sous Windows (`crates/sinew-app/src/workspace.rs`, `crates/sinew-app/src/store.rs`)** : Normalisation en minuscules du workspace_id.
- **Synchronisation Git Multi-PC (`src-tauri/src/lib.rs`)** : Suppression du préfixe UNC des chemins pour les commandes Git.
- **Statut de quota OpenAI (`src/lib/quotas.ts`)** : Pourcentage basé sur le minimum de toutes les fenêtres de limite.

### Removed
- **Badge "5" injustifié sur l'onglet Options (`src/components/SettingsPane.tsx`)**.

---

## [0.1.26] — 2026-05-29

### Added
- **Premier contrôle qualité** : Scripts `check.ps1`, `npm run check`, `cargo clippy` branché.
- **Premières corrections clippy** : `sinew-index`, `sinew-openai`, `sinew-google`.
- **Plan d'action (`afaire.md`)** : Première analyse structurée du projet.

### Changed
- **Agent Autonomie renforcée** : L'agent doit agir directement avec ses outils, sans donner d'instructions manuelles.
- **Nettoyage historique Git** : Retrait des binaires et node_modules trackés.

---

## Historique

Le fork premium `julienpiron.fr` apporte : interface FR/EN, 3 niveaux de réflexion, auto-save, synchro OneDrive, multi-comptes IA (OpenAI, Anthropic, Google, DeepSeek, Kimi, OpenRouter, Cursor), extension Chrome nouvelle génération, terminal intégré, et indexation sémantique locale.
