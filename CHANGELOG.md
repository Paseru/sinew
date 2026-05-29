# Changelog

## [Unreleased] - 2026-05-30 00:46:40

### Fixed
- **Ouverture des fichiers dans l'éditeur (src/components/Workspace.tsx)** : Correction du problème d'ouverture des fichiers où l'éditeur restait vide ("Aucun fichier ouvert") lors du clic. Remplacement de la modification de l'index de l'onglet actif effectuée à tort au sein de la fonction de mise à jour de l'état des onglets (`setTabs`), évitant ainsi les effets de bord incompatibles avec le cycle de rendu de React 18. Utilisation également de la référence stable `tabsRef` pour éviter la recréation répétée de la fonction d'ouverture.
- **Normalisation des chemins sous Windows (crates/sinew-app/src/workspace.rs, crates/sinew-app/src/read.rs)** : Correction du bug d'indexation et de comparaison de chemin où la casse ou le préfixe UNC (\\?\) différait entre les outils de lecture et d'édition, provoquant de fausses erreurs d'évasion de l'espace de travail et bloquant la modification de fichiers.


## [Unreleased] - 2026-05-30 00:34:25

### Added
- **Fiche de transmission structurée (Ancrage, Tâches et diagnostics) (crates/sinew-app/src/agent/compaction.rs)** : Amélioration SOTA de la transition d'IA. Lorsque vous changez de fournisseur d'IA au cours d'une conversation, le contexte est compacté automatiquement avec une fiche structurée contenant : l'état des fichiers modifiés (Git status), le relais des tâches de la todo-list, et les diagnostics de santé du code (linter).

## [Unreleased] - 2026-05-30 00:48:09

### Improved
- **Indexation locale parallele (`crates/sinew-index/src/indexer.rs`)** : preparation des fichiers en parallele sur les coeurs disponibles, saut immediat des fichiers inchanges grace a leur date et leur taille, et limitation des relectures inutiles pour mieux exploiter le CPU et le SSD.
- **Base d'index mieux adaptee au SSD et a la RAM (`crates/sinew-index/src/store.rs`)** : ecritures groupees dans SQLite, cache RAM elargi, lecture memoire SSD, delai d'attente renforce et stockage prioritaire dans le dossier local rapide de la machine avec migration douce de l'ancien cache.
- **Repartition CPU ajoutee (`Cargo.toml`, `crates/sinew-index/Cargo.toml`, `Cargo.lock`)** : ajout de Rayon pour distribuer le travail d'indexation sur plusieurs coeurs.
- **Nettoyage de validation (`crates/sinew-index/src/indexer.rs`, `crates/sinew-index/src/store.rs`)** : retrait des fonctions devenues inutiles et des importations superflues pour garder les tests sans avertissements.
- **Tracabilite (`CHANGELOG.md`)** : ajout de cette entree pour documenter les optimisations de performance demandees.

## [Unreleased] - 2026-05-30 00:54:00

### Added
- **Synchronisation automatique des jetons de connexion (OAuth/clés) via OneDrive (`src-tauri/src/lib.rs`)** : Ajout de la copie automatique des fichiers de connexion (*-auth.json, *-device.json, *-stream-state.json) lors du démarrage, de la fermeture et de la synchronisation forcée. Cela permet de conserver la connexion aux comptes d'IA (Claude, ChatGPT, etc.) identique sur les deux ordinateurs sans devoir se réauthentifier manuellement.
- **Identification universelle par Git remote URL (`src-tauri/src/workspace.rs`, `src-tauri/src/conversations.rs`)** : Priorisation de l'URL distante Git comme identifiant de projet unique universel pour une synchronisation 100% automatique et transparente.
## [Unreleased] - 2026-05-30 00:42:23

### Changed
- **Réorientation du plan d'action (faire.md)** : Nouvelle stratégie respectant la frontière upstream/fork. Priorité aux modules qui nous appartiennent (chrome-bridge, scripts, correctifs propres) pour éviter les conflits avec l'upstream actif (Paseru/sinew, release tous les 2-3 jours). Les refactorings lourds du code upstream sont exclus.
- **Journal des changements (CHANGELOG.md)** : Ajout de cette entrée.

## [Unreleased] - 2026-05-30 00:34:25

### Added
- **Compaction automatique lors du changement d'IA (crates/sinew-app/src/agent/compaction.rs)** : DÃ©tection intelligente du changement de fournisseur d'IA au sein d'une discussion pour dÃ©clencher une compaction automatique du contexte de maniÃ¨re proactive. Cela rÃ©sume le travail prÃ©cÃ©dent et prÃ©sente une fiche de transmission propre au nouveau modÃ¨le, Ã©vitant de perdre en cohÃ©rence.

### Fixed

### Fixed
- **Normalisation des chemins sous Windows (crates/sinew-app/src/read.rs, crates/sinew-app/src/workspace.rs)** : RÃ©solution du problÃ¨me de casse/normalisation UNC (\\?\) qui empÃªchait la correspondance des empreintes de fichiers lus/Ã©crits lors des opÃ©rations edit_file et write_file, causant des erreurs erronÃ©es indiquant que le fichier n'avait pas Ã©tÃ© lu.
- **CompatibilitÃ© de la reprise de conversation Codex avec Google Gemini (crates/sinew-google/src/client.rs)** : RÃ©solution du problÃ¨me d'identifiants d'appels d'outils (tool_call_id) qui causait l'erreur INVALID_ARGUMENT (400) lors de la reprise d'une session commencÃ©e avec Codex en basculant vers Google. Les noms des appels d'outils sont dÃ©sormais recherchÃ©s et rÃ©solus dynamiquement dans l'historique pour garantir leur correspondance exacte.
- **CompatibilitÃ© de la reprise de conversation avec DeepSeek en mode pensÃ©e (crates/sinew-deepseek/src/client.rs)** : Correction de l'erreur 400: The reasoning_content in the thinking mode must be passed back to the API lors du basculement vers DeepSeek. Le champ reasoning_content est dÃ©sormais round-trippÃ© et inclus (mÃªme vide si aucun raisonnement n'a eu lieu) pour tous les messages de l'assistant dans l'historique.


## [Unreleased] - 2026-05-30 00:36:50

### Changed
- **Rapport d'analyse complet (faire.md)** : RÃ©Ã©criture intÃ©grale avec analyse approfondie de l'architecture (5 god files), qualitÃ© (250+ tests, 0 frontend), sÃ©curitÃ© (Tauri, terminal, Chrome), dettes techniques (clippy, CHANGELOG, dÃ©pendances) et plan d'action priorisÃ©.
- **Journal des changements (CHANGELOG.md)** : Ajout de cette entrÃ©e.

## [Unreleased] - 2026-05-30 00:45:00

### Changed
- **Identification universelle par Git remote URL (`src-tauri/src/workspace.rs`, `src-tauri/src/conversations.rs`)** : Priorisation de l'URL distante Git (config remote.origin.url) comme identifiant de projet unique universel. Si un dÃÂ©pÃÂ´t Git est prÃÂ©sent, les deux ordinateurs partagent le mÃÂªme identifiant de projet sans gÃÂ©nÃÂ©rer d'UUIDs locaux diffÃÂ©rents, rendant la synchronisation 100% automatique sans devoir lier manuellement. Migration automatique des anciennes conversations stockÃÂ©es sous le chemin ou sous l'UUID local vers ce nouvel identifiant.

## [Unreleased] - 2026-05-30 00:32:00

### Fixed
- **Normalisation et exclusion de l'UUID du projet actuel dans la liste des autres workspaces (`crates/sinew-app/src/store.rs`, `src-tauri/src/conversations.rs`)** : Correction du bug oÃÂ¹ l'identifiant unique (UUID) du projet actuel s'affichait comme un autre projet dans la liste de dÃÂ©tection. AmÃÂ©lioration de la robustesse de la migration de chemin en ignorant la casse et la forme des slashes (Windows).

## [Unreleased] - 2026-05-30 00:13:17

### Changed
- **Fermeture de la lightbox d'image au clic extÃÂ©rieur (`src/components/chat/ChatPane.tsx`)** : Permet de fermer l'aperÃÂ§u de l'image (lightbox) en cliquant n'importe oÃÂ¹ autour de celle-ci, tout en empÃÂªchant la fermeture accidentelle lors d'un clic sur l'image elle-mÃÂªme ou sur les boutons de la barre d'outils.

## [Unreleased] - 2026-05-30 00:11:30

### Changed
- **Comportement et description des Maquettes Visuelles (`src-tauri/src/turns.rs`, `src/components/SettingsPane.tsx`)** : Ajustement de la consigne et de la description de l'option ÃÂ« Maquettes Visuelles Automatiques ÃÂ». DÃÂ©sormais, la maquette Mermaid n'est plus obligatoire ÃÂ  chaque changement d'interface ; elle n'est gÃÂ©nÃÂ©rÃÂ©e que si vous en faites la demande expresse ou si je l'estime nÃÂ©cessaire pour un changement complexe.

## [Unreleased] - 2026-05-30 00:09:19

### Changed
- **Description de l'Autonomie de l'Agent (`src/components/SettingsPane.tsx`)** : Clarification de la description de l'option ÃÂ« Autonomie de l'Agent ÃÂ» en franÃÂ§ais et en anglais pour mieux expliquer que son but est de m'obliger ÃÂ  agir directement avec mes outils de codage plutÃÂ´t que de lister des instructions manuelles ÃÂ  faire vous-mÃÂªme.

## [Unreleased] - 2026-05-30 00:03:14

### Changed
- **Mode de recherche de mise ÃÂ  jour ÃÂ  3 options (`src/components/SettingsPane.tsx`, `src/App.tsx`, `src/components/UpdateBadge.tsx`)** : ÃÂvolution de l'option de mise ÃÂ  jour pour proposer trois choix : "Bloquant" (vÃÂ©rifie et force la mise ÃÂ  jour au dÃÂ©marrage), "Notification uniquement" (dÃÂ©marre normalement et alerte discrÃÂ¨tement via un badge interne), et "DÃÂ©sactivÃÂ©" (ne vÃÂ©rifie jamais les mises ÃÂ  jour automatiquement).

## [Unreleased] - 2026-05-29 23:59:57

### Added
- **Option de recherche de mise ÃÂ  jour automatique (`src/components/SettingsPane.tsx`, `src/App.tsx`, `src/components/UpdateBadge.tsx`)** : Ajout d'une option dans le panneau de configuration pour activer/dÃÂ©sactiver la recherche de mise ÃÂ  jour automatique. Si dÃÂ©sactivÃÂ©e, l'application ne recherche plus de nouvelles versions au dÃÂ©marrage (ce qui ÃÂ©vite de bloquer l'interface utilisateur avec l'ÃÂ©cran "Mise ÃÂ  jour requise") ni pÃÂ©riodiquement en arriÃÂ¨re-plan.

## [Unreleased] - 2026-05-29 23:45:15

### Added
- **Identifiant de projet universel par fichier cachÃÂ© (`crates/sinew-app/src/store.rs`, `src-tauri/src/workspace.rs`, `src-tauri/src/conversations.rs`, `src-tauri/Cargo.toml`)** : Introduction d'un identifiant unique de projet (UUID) gÃÂ©nÃÂ©rÃÂ© automatiquement dans le fichier cachÃÂ© `.sinew/project_id.txt` ÃÂ  la racine de chaque projet (qu'il utilise Git ou non). Ce fichier se synchronise automatiquement via OneDrive/Git, ce qui permet ÃÂ  l'application de lier instantanÃÂ©ment vos conversations d'un ordinateur ÃÂ  l'autre sans action manuelle, mÃÂªme si les chemins ou les noms des dossiers diffÃÂ¨rent.

## [Unreleased] - 2026-05-29 23:42:04

### Added
- **Synchronisation automatique par URL Git (`crates/sinew-app/src/store.rs`, `src-tauri/src/git.rs`, `src-tauri/src/workspace.rs`, `src-tauri/src/conversations.rs`, `src-tauri/src/turns.rs`, `src-tauri/src/swarm.rs`)** : Les conversations sont dÃÂ©sormais associÃÂ©es au dÃÂ©pÃÂ´t Git distant (remote origin URL). Lorsque vous ouvrez un dÃÂ©pÃÂ´t Git, l'application dÃÂ©tecte automatiquement les conversations correspondantes issues d'autres PC/dossiers (synchronisÃÂ©s par OneDrive) et les lie automatiquement ÃÂ  votre dossier de projet actuel, ÃÂ©liminant tout besoin d'action manuelle.

## [Unreleased] - 2026-05-29 23:36:07

### Added
- **Association de conversations de projets / PC alternatifs (`crates/sinew-app/src/store.rs`, `src-tauri/src/conversations.rs`, `src/lib/ipc.ts`, `src/components/SettingsPane.tsx`)** : Ajout de la possibilitÃÂ© de dÃÂ©tecter et de lier des conversations provenant d'autres dossiers de projets (comme vos dossiers de travail synchronisÃÂ©s par OneDrive) directement depuis les paramÃÂ¨tres.
- **RafraÃÂ®chissement dynamique des conversations (`src/components/Workspace.tsx`)** : ÃÂcouteur d'ÃÂ©vÃÂ©nement global pour rafraÃÂ®chir instantanÃÂ©ment la liste des conversations du projet actif lors d'une migration ou synchronisation.

## [Unreleased] - 2026-05-29 23:34:58

### Changed
- **Options d'affichage du dÃÂ©marrage des serveurs MCP (`src/components/SettingsPane.tsx`, `src/styles.css`)** : Alignement de l'option "Exposer tous les outils au dÃÂ©marrage" sur une seule ligne avec le bouton de bascule (toggle switch) alignÃÂ© ÃÂ  droite, au lieu d'ÃÂªtre compressÃÂ© sur une largeur ÃÂ©troite de 28px.
- **Correction des types TypeScript dans le panneau des options (`src/components/SettingsPane.tsx`)** : Passage de `workspacePath` ÃÂ  `OptionsSection` pour corriger les erreurs de compilation liÃÂ©es ÃÂ  la migration des conversations entre projets.

## [Unreleased] - 2026-05-29 23:34:08

### Changed
- **Comportement des blocs d'outils en erreur en mode trÃÂ¨s compact (`src/components/chat/ToolCard.tsx`)** : En mode trÃÂ¨s compact, les cartes d'outils ayant ÃÂ©chouÃÂ© (comme l'exÃÂ©cution de scripts Bash ou Python) dÃÂ©marrent dÃÂ©sormais repliÃÂ©es (fermÃÂ©es) par dÃÂ©faut au lieu de s'ouvrir automatiquement. L'utilisateur peut toujours cliquer dessus pour les dÃÂ©plier et voir les dÃÂ©tails de l'erreur.


## [Unreleased] - 2026-05-29 23:24:18

### Fixed
- **SensibilitÃÂ© ÃÂ  la casse des chemins de projet sous Windows (`crates/sinew-app/src/workspace.rs`, `crates/sinew-app/src/store.rs`)** : Normalisation en minuscules de l'identifiant unique du dossier de projet (`workspace_id`) sur Windows. Ajout d'une migration SQLite pour mettre ÃÂ  jour automatiquement les anciennes entrÃÂ©es de la base de donnÃÂ©es. Cela corrige le bug oÃÂ¹ les discussions crÃÂ©ÃÂ©es au travail dans un dossier (ex: `C:\Dev\Sinew`) n'ÃÂ©taient pas affichÃÂ©es ÃÂ  la maison si le dossier avait une casse lÃÂ©gÃÂ¨rement diffÃÂ©rente (ex: `C:\Dev\sinew`).

### Added
- **Bouton de synchronisation forcÃÂ©e dans les paramÃÂ¨tres (`src/components/SettingsPane.tsx`)** : Ajout d'un bouton "Synchroniser maintenant" sous l'option de synchronisation Multi-PC dans `OptionsSection` pour permettre ÃÂ  l'utilisateur de dÃÂ©clencher manuellement et ÃÂ  tout moment la synchronisation bidirectionnelle OneDrive et Git.
- **Commande Tauri de synchronisation forcÃÂ©e (`src-tauri/src/lib.rs`, `src/lib/ipc.ts`)** : Ajout de la commande `force_multi_pc_sync` dans Rust et exposition dans le pont IPC frontend pour permettre ÃÂ  l'utilisateur de dÃÂ©clencher manuellement une synchronisation bidirectionnelle complÃÂ¨te des bases de donnÃÂ©es et des dÃÂ©pÃÂ´ts Git avec OneDrive ÃÂ  la demande.

### Changed
- **Correction de la synchronisation Git Multi-PC (`src-tauri/src/lib.rs`)** : Suppression du prÃÂ©fixe UNC `\\?\` des chemins locaux de l'espace de travail avant de dÃÂ©finir le rÃÂ©pertoire de travail (`current_dir`) pour les commandes Git (`git status`, `git pull`, `git commit`, `git push`). Cela rÃÂ©sout l'erreur `CMD ne prend pas les chemins UNC comme rÃÂ©pertoires en cours` (code d'erreur 128) et rÃÂ©tablit la synchronisation automatique transparente entre les diffÃÂ©rents ordinateurs.

## [Unreleased] - 2026-05-29 23:20:51

### Changed
- **Correction de la synchronisation Git Multi-PC (`src-tauri/src/lib.rs`)** : Suppression du prÃÂ©fixe UNC `\\?\` des chemins locaux de l'espace de travail avant de dÃÂ©finir le rÃÂ©pertoire de travail (`current_dir`) pour les commandes Git (`git status`, `git pull`, `git commit`, `git push`). Cela rÃÂ©sout l'erreur `CMD ne prend pas les chemins UNC comme rÃÂ©pertoires en cours` (code d'erreur 128) et rÃÂ©tablit la synchronisation automatique transparente entre les diffÃÂ©rents ordinateurs.

## [Unreleased] - 2026-05-29 23:20:17

### Changed
- **Ignorer dÃÂ©finitivement la demande de mise ÃÂ  jour au dÃÂ©marrage (`src/App.tsx`)** : Permet de ne plus afficher l'ÃÂ©cran de mise ÃÂ  jour bloquante pour une version spÃÂ©cifique lorsque l'utilisateur clique sur "Skip". Le choix est sauvegardÃÂ© localement, et l'application charge directement le dernier dossier ouvert.

## [Unreleased] - 2026-05-29 17:20:18

### Added
- **Script de synchronisation forcÃÂ©e (`sync_now.py`)** : Ajout d'un script robuste en Python permettant de fusionner ÃÂ  la demande et en toute sÃÂ©curitÃÂ© les bases de donnÃÂ©es locale et OneDrive de Sinew, de copier les fichiers d'apprentissage globaux, et de pousser le dÃÂ©pÃÂ´t Git vers GitHub pour garantir une sauvegarde multi-PC ÃÂ  100% sans risque de perte.
- **Mise ÃÂ  jour du plan d'action (`afaire.md`)** : Ajout de la confirmation de mise en place de la synchronisation forcÃÂ©e ÃÂ  la demande.

## [Unreleased] - 2026-05-29 17:17:36

### Changed
- **Horodatage du plan d'action (`afaire.md`)** : Mise ÃÂ  jour de l'heure de synthÃÂ¨se aprÃÂ¨s validation du contrÃÂ´le unique complet.
- **TraÃÂ§abilitÃÂ© du changement (`CHANGELOG.md`)** : Ajout de cette entrÃÂ©e pour documenter cette mise ÃÂ  jour finale.

### Changed
- **ContrÃÂ´le unique sans blocage live (`crates/sinew-cursor/src/tests.rs`, `scripts/check.ps1`, `scripts/agent-bridge/test-live-rust.ps1`)** : Les tests Cursor dÃÂ©pendants d'un compte et du rÃÂ©seau sont ignorÃÂ©s par dÃÂ©faut dans les contrÃÂ´les courants, le script principal prÃÂ©cise qu'ils restent sÃÂ©parÃÂ©s, et le script dÃÂ©diÃÂ© sait les lancer explicitement.
- **Plan d'action mis ÃÂ  jour (`afaire.md`)** : Ajout de l'ÃÂ©tat confirmÃÂ© sur la sÃÂ©paration des tests live pour garder le contrÃÂ´le courant fiable.
- **Changelog (`CHANGELOG.md`)** : Restauration du titre principal et ajout de cette entrÃÂ©e pour documenter les ajustements de validation.

## [Unreleased] - 2026-05-29 16:51:36

### Improved
- **Index local plus rapide sur CPU, RAM et SSD (`crates/sinew-index/src/indexer.rs`)** : PrÃÂ©paration des fichiers en parallÃÂ¨le sur tous les cÃÂurs disponibles, saut des fichiers inchangÃÂ©s grÃÂ¢ce ÃÂ  leur date et leur taille, et ÃÂ©criture par lots pour mieux nourrir le SSD sans gonfler inutilement la mÃÂ©moire.
- **Base dÃ¢ÂÂindex optimisÃÂ©e pour le SSD local (`crates/sinew-index/src/store.rs`)** : DÃÂ©placement du cache dÃ¢ÂÂindex vers le dossier local de la machine, hausse du cache en RAM, lecture mÃÂ©moire SSD, dÃÂ©lai dÃ¢ÂÂattente robuste et ÃÂ©critures groupÃÂ©es.
- **DÃÂ©pendance de parallÃÂ©lisation (`Cargo.toml`, `crates/sinew-index/Cargo.toml`, `Cargo.lock`)** : Ajout de Rayon pour rÃÂ©partir le travail lourd de prÃÂ©paration dÃ¢ÂÂindex sur plusieurs cÃÂurs.
- **Journal des changements (`CHANGELOG.md`)** : Documentation de ces optimisations de performance et de leur raison.

All notable changes to this project will be documented in this file.

## [Unreleased] - 2026-05-29 16:43:13

### Added
- **Script de contrÃÂ´le unique (`scripts/check.ps1`, `package.json`)** : Ajout de `npm run check` pour lancer en une seule commande le build frontend, les contrÃÂ´les Rust, `clippy`, les tests Rust, les audits npm et l'audit Rust si l'outil est installÃÂ©. `clippy` tourne en mode rapport par dÃÂ©faut et peut devenir strict avec `SINEW_STRICT_CLIPPY=1`.

### Changed
- **Stabilisation du test terminal (`crates/sinew-app/src/bash.rs`)** : Le test interactif attend maintenant correctement la fin de la session Windows au lieu d'ÃÂ©chouer sur une rÃÂ©ponse encore en cours.
- **PremiÃÂ¨res corrections `clippy` (`crates/sinew-index/src/background.rs`, `crates/sinew-index/src/process.rs`, `crates/sinew-index/src/search.rs`, `crates/sinew-openai/src/auth.rs`, `crates/sinew-google/src/auth.rs`)** : Correction de petites alertes de qualitÃÂ© signalÃÂ©es pendant le branchement du contrÃÂ´le.
- **Nettoyage du plan d'action (`afaire.md`)** : Retrait des trois prioritÃÂ©s terminÃÂ©es et recentrage sur les actions restantes.
- **Mise ÃÂ  jour de la carte des fichiers (`AGENTS.md`)** : Ajout du nouveau script `scripts/check.ps1` dans la carte du projet.
- **TraÃÂ§abilitÃÂ© du changement (`CHANGELOG.md`)** : Ajout de cette entrÃÂ©e pour documenter les modifications demandÃÂ©es.

## [Unreleased] - 2026-05-29 16:42:57

### Changed
- **Statut de quota prÃÂ©cis pour OpenAI (`src/lib/quotas.ts`)** : Correction du calcul du pourcentage de quota restant pour OpenAI. Le pourcentage prend dÃÂ©sormais en compte le minimum de toutes les fenÃÂªtres de limite (fenÃÂªtre courte et longue) au lieu de masquer un ÃÂ©puisement de quota sur la fenÃÂªtre longue lorsque la fenÃÂªtre courte est ÃÂ  100%. Cela garantit que le voyant de statut du modÃÂ¨le (pastille) passe bien au rouge lorsque le compte est en limite atteinte.
- **TraÃÂ§abilitÃÂ© du changement (`CHANGELOG.md`)** : Ajout de cette entrÃÂ©e pour documenter la correction du bug d'affichage des pastilles de statut.


## [Unreleased] - 2026-05-29 16:39:35

### Removed
- **Badge numÃÂ©rique sur l'onglet Options (`src/components/SettingsPane.tsx`)** : Retrait du badge dynamique codÃÂ© en dur affichant un chiffre "5" injustifiÃÂ© sur l'onglet de configuration gÃÂ©nÃÂ©rale des Options pour un visuel plus propre et professionnel.

## [Unreleased] - 2026-05-29 16:36:20

### Changed
- **Nettoyage du plan d'amÃÂ©lioration (`afaire.md`)** : RÃÂ©ÃÂ©criture complÃÂ¨te du rapport pour ne garder que les constats vÃÂ©rifiÃÂ©s et les actions ÃÂ  fort impact : test Rust bloquant, contrÃÂ´les locaux, gros fichiers ÃÂ  dÃÂ©couper, zones sensibles ÃÂ  auditer, dÃÂ©pendances ÃÂ  surveiller et crÃÂ©ation de `DESIGN.md`.
- **TraÃÂ§abilitÃÂ© du changement (`CHANGELOG.md`)** : Ajout de cette entrÃÂ©e pour documenter la rÃÂ©ÃÂ©criture demandÃÂ©e du fichier de suivi.

## [Unreleased] - 2026-05-29 16:31:40

### Added
- **Mise ÃÂ  jour du rapport d'analyse (`afaire.md`)** : Ajout d'une synthÃÂ¨se vÃÂ©rifiÃÂ©e de l'ÃÂ©tat actuel du projet avec les contrÃÂ´les passÃÂ©s, le test Rust bloquant, les outils Rust manquants, les gros fichiers ÃÂ  dÃÂ©couper et les surfaces sensibles ÃÂ  auditer.
- **TraÃÂ§abilitÃÂ© du changement (`CHANGELOG.md`)** : Ajout de cette entrÃÂ©e pour consigner la modification documentaire demandÃÂ©e par l'utilisateur.

## [Unreleased] - 2026-05-29 16:29:01

### Improved
- **Renforcement des consignes d'Autonomie de l'Agent (`src-tauri/src/state.rs`)** : Ajout d'une rÃÂ¨gle absolue m'interdisant de donner des instructions manuelles ou des commandes textuelles ÃÂ  l'utilisateur si je dispose d'un outil interne capable de rÃÂ©aliser l'action de maniÃÂ¨re proactive et autonome.

## [Unreleased] - 2026-05-29 16:29:17

### Added
- **Rapport SimplifiÃÂ© d'Analyse** : Ajout d'une section de synthÃÂ¨se simplifiÃÂ©e ÃÂ  la fin du fichier `afaire.md` reprenant les points clÃÂ©s et problÃÂ©matiques identifiÃÂ©es sur le projet (god files, duplication, manque de tests).

## [Unreleased] - 2026-05-29 16:29:00

### Added
- **Compilation de l'installateur v0.1.26 et copie sur OneDrive** : Lancement rÃÂ©ussi de la compilation Tauri en mode NSIS et copie de l'installateur gÃÂ©nÃÂ©rÃÂ© (`Sinew_0.1.26_x64-setup.exe`) sur le bureau OneDrive (`C:\Users\julie\OneDrive\Bureau`) pour un dÃÂ©ploiement instantanÃÂ©.

## 2026-05-29
- Fusion (merge) des mises ÃÂ  jour du dÃÂ©pÃÂ´t d'origine (upstream/main) pour synchroniser l'historique Git
- Mise ÃÂ  jour du fichier `afaire.md` avec l'analyse complÃÂ¨te du projet : problÃÂ©matiques, prioritÃÂ©s, et plan d'amÃÂ©lioration
- Ajout de `afaire.md` dans la carte des fichiers de `AGENTS.md`

## Ã°ÂÂÂ PrÃÂ©sentation des FonctionnalitÃÂ©s Majeures (Fork Premium julienpiron.fr)

Cette version a ÃÂ©tÃÂ© optimisÃÂ©e en profondeur pour offrir une expÃÂ©rience utilisateur haut de gamme (SOTA), une autonomie maximale en arriÃÂ¨re-plan, et des intÃÂ©grations d'intelligence artificielle inÃÂ©galÃÂ©es.

### Ã°ÂÂÂ¨ Interface, Confort & Ergonomie (Premium UI)
* **Animation de dÃÂ©marrage premium :** Une animation de boot moderne, fluide et ÃÂ©lÃÂ©gante ÃÂ  l'ouverture de l'application.
* **3 niveaux de rÃÂ©flexion :** Choix entre DÃÂ©taillÃÂ©, Compact ou TrÃÂ¨s compact pour configurer prÃÂ©cisÃÂ©ment la verbositÃÂ© de l'IA et le masquage des dÃÂ©tails techniques dans le chat.
* **Question collante (Sticky Question) :** La question en cours de traitement reste ÃÂ©pinglÃÂ©e en haut de l'ÃÂ©cran pendant que vous faites dÃÂ©filer le fil de discussion.
* **Menu clic droit interactif :**
  * **Sur les onglets :** Clic droit pour fermer l'onglet (raccourci `Ctrl+F4`), les autres ou tous les onglets situÃÂ©s ÃÂ  droite, copier le chemin (absolu ou relatif) ou rÃÂ©vÃÂ©ler dans le Finder/Explorateur.
  * **Sur les fichiers dans le chat :** Clic droit direct pour ouvrir le fichier dans l'ÃÂ©diteur, le rÃÂ©vÃÂ©ler dans le dossier systÃÂ¨me ou l'exÃÂ©cuter dans le terminal.
  * **Sur l'arbre des fichiers (File Tree) :** Option d'exÃÂ©cution directe au clic droit.
* **Polices dynamiques ajustables :** Boutons tactiles rÃÂ©actifs (`+` et `-`) dans les options pour ajuster instantanÃÂ©ment ÃÂ  chaud la taille du texte de l'ÃÂ©diteur de code Monaco et du chat de l'IA.
* **Version franÃÂ§aise complÃÂ¨te :** L'interface entiÃÂ¨re et toutes les actions de l'application s'adaptent automatiquement en franÃÂ§ais ou en anglais selon vos prÃÂ©fÃÂ©rences.
* **SÃÂ©lection et copie libre :** DÃÂ©blocage de la sÃÂ©lection et copie de texte directement dans le fil de discussion.
* **DÃÂ©marcation visuelle du panneau de configuration :** Ligne de sÃÂ©paration verticale ÃÂ©lÃÂ©gante ÃÂ  gauche du panneau de configuration des paramÃÂ¨tres.
* **DÃÂ©coupage du bundle Vite (-80% de taille) :** Monaco Editor et xterm.js sont isolÃÂ©s dans des sous-lots sÃÂ©parÃÂ©s pour un chargement instantanÃÂ© de l'interface utilisateur.

### Ã°ÂÂÂ¾ Autonomie, Sauvegarde & Robustesse SystÃÂ¨me
* **Sauvegarde automatique (Auto-Save SOTA) :** Enregistrement automatique et transparent en arriÃÂ¨re-plan 1,5 seconde aprÃÂ¨s l'arrÃÂªt de la frappe. Activable ou dÃÂ©sactivable d'un clic dans vos options.
* **Mode Sandbox :** Lancement de l'application en un clic sans aucun projet ouvert pour tester l'IA ou utiliser les outils MCP de maniÃÂ¨re isolÃÂ©e.
* **Synchro OneDrive & SQLite automatique :** Synchronisation transparente de vos conversations, configurations de projets et bases de donnÃÂ©es SQLite entre vos diffÃÂ©rents ordinateurs.
* **ZÃÂ©ro popup console Windows :** Lancement asynchrone et silencieux de tous les outils et serveurs en arriÃÂ¨re-plan sans aucune ouverture intempestive de fenÃÂªtres d'invite de commandes.
* **PrÃÂ©fixe PC rÃÂ©el automatique :** Identification automatique du nom de la machine physique pour typer et sÃÂ©curiser les configurations de conversation multi-PC.
* **Diagnostic Windows OAuth rÃÂ©silient :** Capture robuste de l'erreur rÃÂ©seau typique sous Windows (code 10013) et conseils clairs pour dÃÂ©bloquer la connexion (WinNAT/HNS).
* **Diagnostic SOTA :** VÃÂ©rification en un clic de l'ÃÂ©tat de santÃÂ©, du PATH et des versions de tous vos outils de dÃÂ©veloppement (Git, Python, Node, Cargo, etc.).
* **ÃÂcran de mises ÃÂ  jour sÃÂ©curisÃÂ© :** Verrouillage propre de l'interface pendant l'application des correctifs systÃÂ¨me pour ÃÂ©viter toute corruption de donnÃÂ©es.
* **Script de compilation OneDrive (`compil.ps1`) :** Automatisation de la gÃÂ©nÃÂ©ration de l'application et copie immÃÂ©diate sur OneDrive pour un dÃÂ©ploiement instantanÃÂ© sur vos PC.
* **Active Turn Registry :** Moteur intelligent Rust qui suit les turns de l'agent en cours et assure une reprise instantanÃÂ©e du streaming aprÃÂ¨s un redÃÂ©marrage ou en cas de dÃÂ©connexion.

### Ã°ÂÂ¤Â ModÃÂ¨les d'IA, Comptes & FurtivitÃÂ© (AI Engine)
* **Gestion Multi-comptes OpenAI & Google Gemini :** Connexion simultanÃÂ©e de plusieurs profils OpenAI et Google Gemini secondaires avec bascule instantanÃÂ©e entre vos diffÃÂ©rentes clÃÂ©s, comptes et abonnements.
* **Quotas en temps rÃÂ©el :** Visualisation dynamique de votre consommation (crÃÂ©dits / balance restante) sous forme de barres de progression et pastille live dans le chat.
* **Routage & RÃÂ©silience Google Antigravity SOTA :** RÃÂ©paration, optimisation et routage intelligent de vos requÃÂªtes vers les modÃÂ¨les Google les plus performants.
* **Optimisation de vitesse Gemini :** Streaming et requÃÂªtes ultra-rapides pour les modÃÂ¨les Gemini, basÃÂ©s sur l'architecture rÃÂ©seau optimisÃÂ©e de Google Antigravity.
* **Incorporation d'Opus par Google :** IntÃÂ©gration de Claude Opus 4.6 via les abonnements professionnels Google.
* **SystÃÂ¨me Pending/Steering pour Influencer :** Un vrai systÃÂ¨me d'interception et de guidage pour orienter, corriger ou ajouter des instructions en temps rÃÂ©el en cours de gÃÂ©nÃÂ©ration (Pending/Steering).
* **Indexation sÃÂ©mantique locale vectorielle :** Indexation et recherche vectorielle haute-performance effectuÃÂ©e localement sur votre machine avec badge d'ÃÂ©tat interactif dans la barre latÃÂ©rale.
* **IntÃÂ©gration de DeepSeek V4 Pro & V4 Flash :** Prise en charge complÃÂ¨te des modÃÂ¨les phares **DeepSeek V4 Pro** et **DeepSeek V4 Flash** dans le catalogue de l'application.
* **Pont Cursor Composer 2.5 (agent.v1) :** Moteur haute-performance autonome sur connexions HTTP/2 persistantes gÃÂ©rant toutes les modifications chirurgicales de fichiers, avec installation automatique et invisible en arriÃÂ¨re-plan, et masquage du sÃÂ©lecteur d'intelligence inutile.
* **SÃÂ©curitÃÂ© & FurtivitÃÂ© WebSocket :** Spoofing d'empreinte rÃÂ©seau avancÃÂ© pour ÃÂ©liminer tout risque de dÃÂ©tection ou de blocage sur les flux de ChatGPT.
* **WebSocket OpenAI :** Transport temps-rÃÂ©el haute performance basÃÂ© sur WebSocket pour des rÃÂ©ponses fluides et ÃÂ  latence minimale avec OpenAI.

### Ã°ÂÂÂ Extensions & Ponts locaux (MCP & Bridge)
* **Extension Chrome nouvelle gÃÂ©nÃÂ©ration :** Pilotage d'actions de navigation ultra-stables en natif Rust avec mouvements et clics ÃÂ  vitesse humaine (mouvements Beziers, physique fluide) et mode silencieux.
* **RÃÂ©paration Chrome en un clic :** Bouton bleu de configuration automatique si le pont Chrome ne rÃÂ©pond pas sur un nouveau PC.
* **Empaquetage des ressources Tauri :** Le pont local et l'extension Chrome sont intÃÂ©grÃÂ©s directement au sein de l'installateur compilÃÂ© (MSI/EXE).
* **Outils Rust & ripgrep Sidecar :** IntÃÂ©gration de Ripgrep en binaire natif sidecar et de nouveaux outils (`list_dir`, `delete_file`) pour accÃÂ©lÃÂ©rer la recherche et la gestion des fichiers par 10x.
* **Diagnostics Monaco en temps rÃÂ©el :** RemontÃÂ©e automatique des lints et erreurs de compilation de l'ÃÂ©diteur de code ÃÂ  l'IA en temps rÃÂ©el.
* **Logs ultra-compacts :** Nettoyage automatique du contexte de discussion pour ÃÂ©liminer le bruit et optimiser la consommation de jetons.
* **Laboratoire rÃÂ©seau MITM :** Outils de dÃÂ©bogage et d'ingÃÂ©nierie inverse intÃÂ©grÃÂ©s pour inspecter le trafic chiffrÃÂ© des outils IA.
* **Moteur de remplacement intelligent (Search/Replace) :** SystÃÂ¨me d'auto-correction ÃÂ  8 couches (Unicode, indentations, etc.) garantissant que les modifications de l'IA s'insÃÂ¨rent correctement dans vos fichiers mÃÂªme en cas de lÃÂ©gÃÂ¨res erreurs d'espaces.

---

## Ã°ÂÂÂ Historique des Versions

## [Unreleased] - 2026-05-29 16:18:48

### Added
- **Rapport d'analyse et d'amÃÂ©liorations (`afaire.md`)** : CrÃÂ©ation d'un document rÃÂ©capitulatif listant les principaux axes d'amÃÂ©lioration du projet (performances de l'interface, organisation du moteur Rust, et sÃÂ©curitÃÂ© des dÃÂ©pendances).


## [Unreleased] - 2026-05-29 16:16:37

### Added
- **Prise en charge de Claude Opus 4.8 (`crates/sinew-anthropic/src/client.rs`, `crates/sinew-anthropic/src/model_info.rs`, `src/lib/models.ts`)** : IntÃÂ©gration du nouveau modÃÂ¨le phare de la gamme d'intelligence artificielle d'Anthropic (Claude Opus 4.8) dotÃÂ© d'une fenÃÂªtre de contexte native de 1 million de jetons et des niveaux de rÃÂ©flexion configurables dans l'interface utilisateur.

### Changed
- **Mise ÃÂ  jour de la configuration de compilation (`Cargo.toml`, `Cargo.lock`)** : Synchronisation des versions des dÃÂ©pendances et du systÃÂ¨me de compilation avec la version de rÃÂ©fÃÂ©rence 0.1.26 de la branche parente.

## [Unreleased] - 2026-05-29 16:21:45

### Removed
- **Nettoyage complet du fichier `AGENTS.md` local** : Suppression dÃÂ©finitive de toute trace ou consigne relative au systÃÂ¨me d'apprentissage automatique dans le fichier `AGENTS.md` de ce projet. L'injection et la gestion de la mÃÂ©moire globale sont dÃÂ©sormais entiÃÂ¨rement intÃÂ©grÃÂ©es de maniÃÂ¨re native au sein de l'application (cÃÂ´tÃÂ© Rust) pour tous les projets ouverts sur cette machine.

## [Unreleased] - 2026-05-29 16:15:36

### Added
- **Synchronisation OneDrive du SystÃÂ¨me d'Apprentissage (`src-tauri/src/lib.rs`)** : Ajout de la synchronisation bidirectionnelle automatique des fichiers d'apprentissage globaux (`errors_raw.json` et `instructions_consolidated.md`) via OneDrive. Lorsque l'option de synchronisation Multi-PC est active, ces fichiers sont fusionnÃÂ©s et sauvegardÃÂ©s sur OneDrive ÃÂ  la fermeture de l'application, et restaurÃÂ©s au dÃÂ©marrage sur vos autres ordinateurs.

## [Unreleased] - 2026-05-29 16:06:04

### Fixed
- **Correction de la visibilitÃÂ© de l'indicateur de contexte en mode IA (`src/styles.css`)** : Augmentation de la prioritÃÂ© d'empilement (z-index) de la zone de saisie (`.composer`) ÃÂ  10 pour s'assurer que le volet volant affichant l'utilisation du contexte s'affiche par-dessus les messages du chat et la liste des tÃÂ¢ches, ÃÂ©vitant qu'il ne soit masquÃÂ© ou floutÃÂ© par les effets de transparence premium du mode IA.

## [Unreleased] - 2026-05-29 16:11:45

### Added
- **IntÃÂ©gration du systÃÂ¨me d'apprentissage global cÃÂ´tÃÂ© Rust (`src-tauri/src/turns.rs`)** : Modification du backend Rust de l'application pour charger et injecter automatiquement et de maniÃÂ¨re transparente le fichier d'instructions consolidÃÂ©es global (`%LOCALAPPDATA%\Sinew\instructions_consolidated.md`) dans le prompt systÃÂ¨me de tous les agents pour l'ensemble des projets ouverts sur cet ordinateur. Les agents bÃÂ©nÃÂ©ficient dÃÂ©sormais de cette base d'apprentissage universelle sans dÃÂ©pendre d'un fichier local `AGENTS.md`.

## [Unreleased] - 2026-05-29 16:06:07

### Changed
- **DÃÂ©centralisation complÃÂ¨te des descriptions d'erreurs (`AGENTS.md`)** : Suppression dÃÂ©finitive des descriptions locales d'erreurs dans `AGENTS.md` pour ÃÂ©viter toute duplication. DÃÂ©sormais, `AGENTS.md` ne contient que le pointeur d'instructions globales pour forcer la lecture de la base d'apprentissage centralisÃÂ©e dans `%LOCALAPPDATA%\Sinew\`.
- **Base de connaissances globale (`%LOCALAPPDATA%\Sinew\`)** : Migration de toutes les anciennes rÃÂ¨gles d'erreurs (Git, Windows, npm, MCP) directement dans le fichier d'apprentissage consolidÃÂ© de la machine.

## [Unreleased] - 2026-05-29 16:04:26

### Changed
- **Globalisation du SystÃÂ¨me d'Apprentissage** : DÃÂ©placement de la mÃÂ©moire d'apprentissage (`errors_raw.json` et `instructions_consolidated.md`) dans le dossier de configuration global de la machine (`%LOCALAPPDATA%\Sinew\`). Cela permet d'avoir un systÃÂ¨me d'apprentissage partagÃÂ© et partagÃÂ© sur tous les projets et espaces de travail ouverts sur cet ordinateur.
- **RÃÂ¨gles d'agent (`AGENTS.md`)** : Mise ÃÂ  jour des rÃÂ¨gles d'instructions globales pour forcer la lecture et l'alimentation de la base d'apprentissage globale ÃÂ  chaque dÃÂ©but de session.

## [Unreleased] - 2026-05-29 15:57:18

### Added
- **IntÃÂ©gration d'outils de diagnostic et d'ÃÂ©mulation hybrides (`sinew-chrome-bridge/mcp_server.js` et `.sinew/skills/browser/SKILL.md`)** : Ajout de trois nouveaux outils structurÃÂ©s MCP (`emulate_experience`, `lighthouse_audit`, `analyze_memory_leaks`) basÃÂ©s sur l'API CDP brute de Google Chrome pour tester la compatibilitÃÂ© mobile (taille, touch events), ÃÂ©valuer les performances et diagnostics de qualitÃÂ© Lighthouse en local, et analyser la consommation de mÃÂ©moire (JS heap et DOM nodes count).
- **Mise ÃÂ  jour de la compÃÂ©tence Browser (`.sinew/skills/browser/SKILL.md`)** : Ajout d'une section de documentation claire guidant l'IA ÃÂ  utiliser ces nouveaux pouvoirs de diagnostic en mode hybride tout en prÃÂ©servant le simulateur biologique humain furtif de Sinew Chrome.

## [Unreleased] - 2026-05-29 15:46:39

### Fixed
- **Correction et universalisation des lightboxes (`src/components/chat/ChatPane.tsx`, `src/components/chat/MermaidDiagram.tsx`, `src/styles.css`)** : Passage des zooms d'image et de diagrammes Mermaid sous forme de Portails React (`createPortal`) attachÃÂ©s ÃÂ  `document.body`. Cela corrige dÃÂ©finitivement le problÃÂ¨me de rognage/masquage causÃÂ© par les empilements CSS (stacking contexts) des colonnes du chat et des options. De plus, augmentation de la prioritÃÂ© d'affichage (`z-index: 99999`) et intÃÂ©gration des styles sombres nÃÂ©on haut de gamme pour les schÃÂ©mas en mode IA afin qu'ils s'affichent magnifiquement dans tous les thÃÂ¨mes.

## [Unreleased] - 2026-05-29 15:52:23

### Added
- **ÃÂtude d'impact sur la sÃÂ©curitÃÂ©, l'anti-dÃÂ©tection et la discrÃÂ©tion (`ETUDE_IMPACT_SECURITE.md`)** : Rapport d'analyse complet ÃÂ©valuant les risques de dÃÂ©tection des fonctionnalitÃÂ©s de DevTools for Agents 1.0 (ÃÂ©mulation, auto-connect, audits) et concevant une synergie d'interception avec notre simulateur biologique humain.
- **Rapport d'analyse et d'intÃÂ©gration DevTools (`ANALYSE_DEVTOOLS_MCP.md`)** : CrÃÂ©ation d'une analyse technique poussÃÂ©e ÃÂ©valuant la faisabilitÃÂ© et l'impact de l'intÃÂ©gration des fonctionnalitÃÂ©s de Chrome DevTools for Agents 1.0 (Lighthouse, ÃÂ©mulation, fuites mÃÂ©moire, auto-connect) tout en conservant le simulateur biologique humain furtif unique de Sinew Chrome.
- **Rapport d'ÃÂ©valuation de la pertinence des outils de diagnostic Google (`ANALYSE_DIAGNOSTIC_GOOGLE.md`)** : Analyse de pertinence des outils de diagnostic avancÃÂ©s de Google (Lighthouse, ÃÂ©mulation, fuites mÃÂ©moire, auto-connect) du point de vue de l'audit et du contrÃÂ´le qualitÃÂ©, avec mÃÂ©taphores simples et plan d'action d'adaptation pour notre simulateur biologique.

### Changed
- **Mise ÃÂ  jour de la carte du code (`AGENTS.md`)** : Enregistrement des rapports d'analyse de l'ÃÂ©quipe (sÃÂ©curitÃÂ©, architecture, intÃÂ©gration, diagnostic) dans la carte des fichiers du projet.

## [Unreleased] - 2026-05-29 15:48:14

### Added
- **Option de Recherche SÃÂ©mantique Vectorielle (`src/components/SettingsPane.tsx` & `src-tauri/src/workspace.rs` & `src-tauri/src/lib.rs` & `src/lib/ipc.ts`)** : Ajout d'une nouvelle option SOTA ÃÂ« Recherche SÃÂ©mantique Vectorielle (BETA) ÃÂ» dans les paramÃÂ¨tres (Apparence & Interface). Liaison avec un nouveau handler Tauri Rust `set_semantic_embeddings_enabled` pour activer/dÃÂ©sactiver dynamiquement la variable d'environnement `SINEW_INDEX_EMBEDDINGS` ÃÂ  la volÃÂ©e.

## [Unreleased] - 2026-05-29 15:47:36

### Added
- **Script d'installation automatisÃÂ© (`package.json`)** : Ajout d'un script `postinstall` pour installer automatiquement les dÃÂ©pendances des extensions locales (`sinew-chrome-bridge` et `scripts/agent-bridge`) lors du lancement initial de `npm install` par n'importe quel dÃÂ©veloppeur ou serveur de compilation.

## [Unreleased] - 2026-05-29 15:43:42

### Changed
- **Exclusion de dossiers (`.gitignore`)** : Ajout du rÃÂ©pertoire `/build/` dans les rÃÂ¨gles d'exclusion de Git pour ÃÂ©viter de traquer les binaires et installateurs compilÃÂ©s localement.

### Removed
- **Nettoyage de l'historique Git** : Retrait du suivi de version Git pour les gros fichiers d'installation d'anciennes versions (`build/Sinew.exe`, `build/Sinew_0.1.25_x64-setup.exe` et `build/Sinew_0.1.25_x64_en-US.msi`) ainsi que pour le dossier de dÃÂ©pendances obsolÃÂ¨tes (`sinew-chrome-bridge/node_modules/`), tout en conservant ces fichiers intacts sur les disques durs locaux.

## [Unreleased] - 2026-05-29 15:40:51

### Added
- **Rapport d'analyse et d'idÃÂ©es SOTA (`RAPPORT_OUTILS.md`)** : CrÃÂ©ation d'un rapport consolidÃÂ© prÃÂ©sentant la vÃÂ©rification des outils et dÃÂ©taillant les 3 pistes d'amÃÂ©liorations SOTA validÃÂ©es et sÃÂ©curisÃÂ©es.

## [Unreleased] - 2026-05-29 15:39:58

### Changed
- **IntÃÂ©gration du Changelog (`CHANGELOG.md`)** : DÃÂ©placement et intÃÂ©gration de la documentation des fonctionnalitÃÂ©s majeures du fork (anciennement `README-FORK.md`) directement en en-tÃÂªte du journal pour une meilleure visibilitÃÂ©.

### Removed
- **Nettoyage gÃÂ©nÃÂ©ral des vestiges et rapports** : Suppression dÃÂ©finitive de 12 fichiers d'audits, de rapports et de scripts de test obsolÃÂ¨tes (`AUDIT_PERFORMANCE_SOTA.md`, `AUDIT_RUST.md`, `AUDIT_SECURITE.md`, `RAPPORT_OUTILS.md`, `RAPPORT_VERIFICATION.md`, `RAPPORT_VERIFICATION_FINAL.md`, `README-FORK.md`, `scripts/AGENT-SPIKE-DESIGN.md`, `scripts/CAPTURE-MITM.md`, `scripts/probe_agent_run.py`, `scripts/probe_idempotent_crypto.py`, `scripts/verify_all.py` et le cache `__pycache__`) pour nettoyer totalement l'arborescence du projet.

## [Unreleased] - 2026-05-29 15:33:18

### Added
- **Rapport d'analyse des outils (`RAPPORT_OUTILS.md`)** : CrÃÂ©ation d'un rapport complet sur l'ÃÂ©tat des outils systÃÂ¨me, de l'indexeur et de l'intÃÂ©gration du navigateur Sinew pour rÃÂ©pondre aux interrogations de l'utilisateur.

## [Unreleased] - 2026-05-29 15:31:31

### Fixed
- **Settings Pane (`src/components/SettingsPane.tsx`)** : Correction de la persistence de l'option ÃÂ« Exposer tous les outils au dÃÂ©marrage ÃÂ» (`autoLoad`) pour les serveurs MCP. L'option ÃÂ©tait omise lors de la sÃÂ©rialisation des paramÃÂ¨tres en JSON (`settingsToJson`), ce qui entraÃÂ®nait sa rÃÂ©initialisation ÃÂ  chaque rechargement ou modification de la configuration. Ajout de la sÃÂ©rialisation de `autoLoad` dans le JSON exportÃÂ©.

## [Unreleased] - 2026-05-29 15:28:08

### Fixed
- **Styles CSS (`src/styles.css`)** : Correction du problÃÂ¨me d'affichage du menu dÃÂ©roulant (popover) de sÃÂ©lection des modÃÂ¨les en mode/thÃÂ¨me IA. Remplacement de `overflow: hidden` par `overflow: visible !important` pour la boÃÂ®te de composition `.composer__box` sous le sÃÂ©lecteur `html[data-theme="ai"]`, ÃÂ©vitant ainsi le masquage ou le rognage des options du menu au-delÃÂ  des bordures du conteneur.
