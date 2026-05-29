# Changelog

## [Unreleased] - 2026-05-30 01:00:52

### Added
- **Script d'auto-consolidation de la mémoire (consolidate_rules.py)** : Ajout d'un script d'automatisation pour analyser les erreurs répétitives du fichier errors_raw.json et les nettoyer si une règle globale correspondante est présente dans instructions_consolidated.md.
## [Unreleased] - 2026-05-30 01:05:00

### Improved
- **Reglages adaptes a chaque PC (`crates/sinew-index/src/store.rs`)** : remplacement des tailles fixes de cache SQLite par un calcul base sur les coeurs disponibles. Sinew s'adapte ainsi automatiquement au PC fixe, au portable ou a une future machine sans viser une configuration precise.
- **Tracabilite (`CHANGELOG.md`)** : ajout de cette entree pour documenter l'ajustement general demande.

## [Unreleased] â€” 2026-05-30

### Added
- **ContrÃ´le unique du projet (`scripts/check.ps1`, `package.json`)** : `npm run check` lance le build, `cargo check`, `clippy`, les tests, et les audits npm en une commande.
- **Synchronisation automatique des jetons IA via OneDrive (`src-tauri/src/lib.rs`)** : Copie automatique des fichiers de connexion OAuth/clÃ©s au dÃ©marrage, Ã  la fermeture et en synchro forcÃ©e. Les comptes IA restent connectÃ©s sur tous les PC.
- **Identification universelle par URL Git (`src-tauri/src/workspace.rs`, `src-tauri/src/conversations.rs`)** : L'URL distante Git sert d'identifiant de projet unique. Migration automatique des anciennes conversations.
- **Synchronisation forcÃ©e Ã  la demande (`src-tauri/src/lib.rs`, `src/components/SettingsPane.tsx`)** : Bouton "Synchroniser maintenant" dans les paramÃ¨tres + commande Tauri `force_multi_pc_sync`.
- **Compaction automatique au changement de fournisseur IA (`crates/sinew-app/src/agent/compaction.rs`)** : Fiche de transmission structurÃ©e (Git status, tÃ¢ches, diagnostics) lors du changement d'IA.

### Changed
- **Rapport d'analyse complet (`afaire.md`)** : RÃ©Ã©criture intÃ©grale avec diagnostic architecture, qualitÃ©, sÃ©curitÃ©, et plan d'action priorisÃ© respectant la frontiÃ¨re upstream/fork.
- **Tests live Cursor sÃ©parÃ©s (`crates/sinew-cursor/src/tests.rs`)** : IgnorÃ©s par dÃ©faut dans les contrÃ´les courants, lancÃ©s par scripts dÃ©diÃ©s.
- **Option de mise Ã  jour Ã  3 choix (`src/components/SettingsPane.tsx`)** : Bloquant / Notification uniquement / DÃ©sactivÃ©.
- **Fermeture lightbox au clic extÃ©rieur (`src/components/chat/ChatPane.tsx`)**.
- **Maquettes visuelles assouplies (`src-tauri/src/turns.rs`)** : Plus obligatoires, seulement si demandÃ© ou nÃ©cessaire.

### Fixed
- **Stabilisation du test terminal (`crates/sinew-app/src/bash.rs`)** : Le test interactif attend correctement la fin de session.
- **Ouverture de fichiers dans l'Ã©diteur (`src/components/Workspace.tsx`)** : Correction du bug "Aucun fichier ouvert" via rÃ©fÃ©rence stable.
- **Normalisation des chemins Windows (`crates/sinew-app/src/workspace.rs`, `crates/sinew-app/src/read.rs`)** : Correction casse et prÃ©fixe UNC qui causaient de fausses erreurs.
- **Reprise de conversation avec Google Gemini (`crates/sinew-google/src/client.rs`)** : RÃ©solution erreur INVALID_ARGUMENT sur les tool_call_id.
- **Reprise de conversation avec DeepSeek (`crates/sinew-deepseek/src/client.rs`)** : Correction du champ reasoning_content manquant.
- **SensibilitÃ© Ã  la casse sous Windows (`crates/sinew-app/src/workspace.rs`, `crates/sinew-app/src/store.rs`)** : Normalisation en minuscules du workspace_id.
- **Synchronisation Git Multi-PC (`src-tauri/src/lib.rs`)** : Suppression du prÃ©fixe UNC des chemins pour les commandes Git.
- **Statut de quota OpenAI (`src/lib/quotas.ts`)** : Pourcentage basÃ© sur le minimum de toutes les fenÃªtres de limite.

### Removed
- **Badge "5" injustifiÃ© sur l'onglet Options (`src/components/SettingsPane.tsx`)**.

---

## [0.1.26] â€” 2026-05-29

### Added
- **Premier contrÃ´le qualitÃ©** : Scripts `check.ps1`, `npm run check`, `cargo clippy` branchÃ©.
- **PremiÃ¨res corrections clippy** : `sinew-index`, `sinew-openai`, `sinew-google`.
- **Plan d'action (`afaire.md`)** : PremiÃ¨re analyse structurÃ©e du projet.

### Changed
- **Agent Autonomie renforcÃ©e** : L'agent doit agir directement avec ses outils, sans donner d'instructions manuelles.
- **Nettoyage historique Git** : Retrait des binaires et node_modules trackÃ©s.

---

## Historique

Le fork premium `julienpiron.fr` apporte : interface FR/EN, 3 niveaux de rÃ©flexion, auto-save, synchro OneDrive, multi-comptes IA (OpenAI, Anthropic, Google, DeepSeek, Kimi, OpenRouter, Cursor), extension Chrome nouvelle gÃ©nÃ©ration, terminal intÃ©grÃ©, et indexation sÃ©mantique locale.

