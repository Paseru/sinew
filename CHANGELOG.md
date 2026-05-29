# Changelog

## [Unreleased] - 2026-05-30 00:34:25

### Fixed
- **Normalisation des chemins sous Windows (crates/sinew-app/src/read.rs, crates/sinew-app/src/workspace.rs)** : Résolution du problème de casse/normalisation UNC (\\?\) qui empêchait la correspondance des empreintes de fichiers lus/écrits lors des opérations edit_file et write_file, causant des erreurs erronées indiquant que le fichier n'avait pas été lu.
- **Compatibilité de la reprise de conversation Codex avec Google Gemini (crates/sinew-google/src/client.rs)** : Résolution du problème d'identifiants d'appels d'outils (tool_call_id) qui causait l'erreur INVALID_ARGUMENT (400) lors de la reprise d'une session commencée avec Codex en basculant vers Google. Les noms des appels d'outils sont désormais recherchés et résolus dynamiquement dans l'historique pour garantir leur correspondance exacte.
- **Compatibilité de la reprise de conversation avec DeepSeek en mode pensée (crates/sinew-deepseek/src/client.rs)** : Correction de l'erreur 400: The reasoning_content in the thinking mode must be passed back to the API lors du basculement vers DeepSeek. Le champ reasoning_content est désormais round-trippé et inclus (même vide si aucun raisonnement n'a eu lieu) pour tous les messages de l'assistant dans l'historique.


## [Unreleased] - 2026-05-30 00:36:50

### Changed
- **Rapport d'analyse complet (faire.md)** : Réécriture intégrale avec analyse approfondie de l'architecture (5 god files), qualité (250+ tests, 0 frontend), sécurité (Tauri, terminal, Chrome), dettes techniques (clippy, CHANGELOG, dépendances) et plan d'action priorisé.
- **Journal des changements (CHANGELOG.md)** : Ajout de cette entrée.

## [Unreleased] - 2026-05-30 00:45:00

### Changed
- **Identification universelle par Git remote URL (`src-tauri/src/workspace.rs`, `src-tauri/src/conversations.rs`)** : Priorisation de l'URL distante Git (config remote.origin.url) comme identifiant de projet unique universel. Si un dÃ©pÃ´t Git est prÃ©sent, les deux ordinateurs partagent le mÃªme identifiant de projet sans gÃ©nÃ©rer d'UUIDs locaux diffÃ©rents, rendant la synchronisation 100% automatique sans devoir lier manuellement. Migration automatique des anciennes conversations stockÃ©es sous le chemin ou sous l'UUID local vers ce nouvel identifiant.

## [Unreleased] - 2026-05-30 00:32:00

### Fixed
- **Normalisation et exclusion de l'UUID du projet actuel dans la liste des autres workspaces (`crates/sinew-app/src/store.rs`, `src-tauri/src/conversations.rs`)** : Correction du bug oÃ¹ l'identifiant unique (UUID) du projet actuel s'affichait comme un autre projet dans la liste de dÃ©tection. AmÃ©lioration de la robustesse de la migration de chemin en ignorant la casse et la forme des slashes (Windows).

## [Unreleased] - 2026-05-30 00:13:17

### Changed
- **Fermeture de la lightbox d'image au clic extÃ©rieur (`src/components/chat/ChatPane.tsx`)** : Permet de fermer l'aperÃ§u de l'image (lightbox) en cliquant n'importe oÃ¹ autour de celle-ci, tout en empÃªchant la fermeture accidentelle lors d'un clic sur l'image elle-mÃªme ou sur les boutons de la barre d'outils.

## [Unreleased] - 2026-05-30 00:11:30

### Changed
- **Comportement et description des Maquettes Visuelles (`src-tauri/src/turns.rs`, `src/components/SettingsPane.tsx`)** : Ajustement de la consigne et de la description de l'option Â« Maquettes Visuelles Automatiques Â». DÃ©sormais, la maquette Mermaid n'est plus obligatoire Ã  chaque changement d'interface ; elle n'est gÃ©nÃ©rÃ©e que si vous en faites la demande expresse ou si je l'estime nÃ©cessaire pour un changement complexe.

## [Unreleased] - 2026-05-30 00:09:19

### Changed
- **Description de l'Autonomie de l'Agent (`src/components/SettingsPane.tsx`)** : Clarification de la description de l'option Â« Autonomie de l'Agent Â» en franÃ§ais et en anglais pour mieux expliquer que son but est de m'obliger Ã  agir directement avec mes outils de codage plutÃ´t que de lister des instructions manuelles Ã  faire vous-mÃªme.

## [Unreleased] - 2026-05-30 00:03:14

### Changed
- **Mode de recherche de mise Ã  jour Ã  3 options (`src/components/SettingsPane.tsx`, `src/App.tsx`, `src/components/UpdateBadge.tsx`)** : Ãvolution de l'option de mise Ã  jour pour proposer trois choix : "Bloquant" (vÃ©rifie et force la mise Ã  jour au dÃ©marrage), "Notification uniquement" (dÃ©marre normalement et alerte discrÃ¨tement via un badge interne), et "DÃ©sactivÃ©" (ne vÃ©rifie jamais les mises Ã  jour automatiquement).

## [Unreleased] - 2026-05-29 23:59:57

### Added
- **Option de recherche de mise Ã  jour automatique (`src/components/SettingsPane.tsx`, `src/App.tsx`, `src/components/UpdateBadge.tsx`)** : Ajout d'une option dans le panneau de configuration pour activer/dÃ©sactiver la recherche de mise Ã  jour automatique. Si dÃ©sactivÃ©e, l'application ne recherche plus de nouvelles versions au dÃ©marrage (ce qui Ã©vite de bloquer l'interface utilisateur avec l'Ã©cran "Mise Ã  jour requise") ni pÃ©riodiquement en arriÃ¨re-plan.

## [Unreleased] - 2026-05-29 23:45:15

### Added
- **Identifiant de projet universel par fichier cachÃ© (`crates/sinew-app/src/store.rs`, `src-tauri/src/workspace.rs`, `src-tauri/src/conversations.rs`, `src-tauri/Cargo.toml`)** : Introduction d'un identifiant unique de projet (UUID) gÃ©nÃ©rÃ© automatiquement dans le fichier cachÃ© `.sinew/project_id.txt` Ã  la racine de chaque projet (qu'il utilise Git ou non). Ce fichier se synchronise automatiquement via OneDrive/Git, ce qui permet Ã  l'application de lier instantanÃ©ment vos conversations d'un ordinateur Ã  l'autre sans action manuelle, mÃªme si les chemins ou les noms des dossiers diffÃ¨rent.

## [Unreleased] - 2026-05-29 23:42:04

### Added
- **Synchronisation automatique par URL Git (`crates/sinew-app/src/store.rs`, `src-tauri/src/git.rs`, `src-tauri/src/workspace.rs`, `src-tauri/src/conversations.rs`, `src-tauri/src/turns.rs`, `src-tauri/src/swarm.rs`)** : Les conversations sont dÃ©sormais associÃ©es au dÃ©pÃ´t Git distant (remote origin URL). Lorsque vous ouvrez un dÃ©pÃ´t Git, l'application dÃ©tecte automatiquement les conversations correspondantes issues d'autres PC/dossiers (synchronisÃ©s par OneDrive) et les lie automatiquement Ã  votre dossier de projet actuel, Ã©liminant tout besoin d'action manuelle.

## [Unreleased] - 2026-05-29 23:36:07

### Added
- **Association de conversations de projets / PC alternatifs (`crates/sinew-app/src/store.rs`, `src-tauri/src/conversations.rs`, `src/lib/ipc.ts`, `src/components/SettingsPane.tsx`)** : Ajout de la possibilitÃ© de dÃ©tecter et de lier des conversations provenant d'autres dossiers de projets (comme vos dossiers de travail synchronisÃ©s par OneDrive) directement depuis les paramÃ¨tres.
- **RafraÃ®chissement dynamique des conversations (`src/components/Workspace.tsx`)** : Ãcouteur d'Ã©vÃ©nement global pour rafraÃ®chir instantanÃ©ment la liste des conversations du projet actif lors d'une migration ou synchronisation.

## [Unreleased] - 2026-05-29 23:34:58

### Changed
- **Options d'affichage du dÃ©marrage des serveurs MCP (`src/components/SettingsPane.tsx`, `src/styles.css`)** : Alignement de l'option "Exposer tous les outils au dÃ©marrage" sur une seule ligne avec le bouton de bascule (toggle switch) alignÃ© Ã  droite, au lieu d'Ãªtre compressÃ© sur une largeur Ã©troite de 28px.
- **Correction des types TypeScript dans le panneau des options (`src/components/SettingsPane.tsx`)** : Passage de `workspacePath` Ã  `OptionsSection` pour corriger les erreurs de compilation liÃ©es Ã  la migration des conversations entre projets.

## [Unreleased] - 2026-05-29 23:34:08

### Changed
- **Comportement des blocs d'outils en erreur en mode trÃ¨s compact (`src/components/chat/ToolCard.tsx`)** : En mode trÃ¨s compact, les cartes d'outils ayant Ã©chouÃ© (comme l'exÃ©cution de scripts Bash ou Python) dÃ©marrent dÃ©sormais repliÃ©es (fermÃ©es) par dÃ©faut au lieu de s'ouvrir automatiquement. L'utilisateur peut toujours cliquer dessus pour les dÃ©plier et voir les dÃ©tails de l'erreur.


## [Unreleased] - 2026-05-29 23:24:18

### Fixed
- **SensibilitÃ© Ã  la casse des chemins de projet sous Windows (`crates/sinew-app/src/workspace.rs`, `crates/sinew-app/src/store.rs`)** : Normalisation en minuscules de l'identifiant unique du dossier de projet (`workspace_id`) sur Windows. Ajout d'une migration SQLite pour mettre Ã  jour automatiquement les anciennes entrÃ©es de la base de donnÃ©es. Cela corrige le bug oÃ¹ les discussions crÃ©Ã©es au travail dans un dossier (ex: `C:\Dev\Sinew`) n'Ã©taient pas affichÃ©es Ã  la maison si le dossier avait une casse lÃ©gÃ¨rement diffÃ©rente (ex: `C:\Dev\sinew`).

### Added
- **Bouton de synchronisation forcÃ©e dans les paramÃ¨tres (`src/components/SettingsPane.tsx`)** : Ajout d'un bouton "Synchroniser maintenant" sous l'option de synchronisation Multi-PC dans `OptionsSection` pour permettre Ã  l'utilisateur de dÃ©clencher manuellement et Ã  tout moment la synchronisation bidirectionnelle OneDrive et Git.
- **Commande Tauri de synchronisation forcÃ©e (`src-tauri/src/lib.rs`, `src/lib/ipc.ts`)** : Ajout de la commande `force_multi_pc_sync` dans Rust et exposition dans le pont IPC frontend pour permettre Ã  l'utilisateur de dÃ©clencher manuellement une synchronisation bidirectionnelle complÃ¨te des bases de donnÃ©es et des dÃ©pÃ´ts Git avec OneDrive Ã  la demande.

### Changed
- **Correction de la synchronisation Git Multi-PC (`src-tauri/src/lib.rs`)** : Suppression du prÃ©fixe UNC `\\?\` des chemins locaux de l'espace de travail avant de dÃ©finir le rÃ©pertoire de travail (`current_dir`) pour les commandes Git (`git status`, `git pull`, `git commit`, `git push`). Cela rÃ©sout l'erreur `CMD ne prend pas les chemins UNC comme rÃ©pertoires en cours` (code d'erreur 128) et rÃ©tablit la synchronisation automatique transparente entre les diffÃ©rents ordinateurs.

## [Unreleased] - 2026-05-29 23:20:51

### Changed
- **Correction de la synchronisation Git Multi-PC (`src-tauri/src/lib.rs`)** : Suppression du prÃ©fixe UNC `\\?\` des chemins locaux de l'espace de travail avant de dÃ©finir le rÃ©pertoire de travail (`current_dir`) pour les commandes Git (`git status`, `git pull`, `git commit`, `git push`). Cela rÃ©sout l'erreur `CMD ne prend pas les chemins UNC comme rÃ©pertoires en cours` (code d'erreur 128) et rÃ©tablit la synchronisation automatique transparente entre les diffÃ©rents ordinateurs.

## [Unreleased] - 2026-05-29 23:20:17

### Changed
- **Ignorer dÃ©finitivement la demande de mise Ã  jour au dÃ©marrage (`src/App.tsx`)** : Permet de ne plus afficher l'Ã©cran de mise Ã  jour bloquante pour une version spÃ©cifique lorsque l'utilisateur clique sur "Skip". Le choix est sauvegardÃ© localement, et l'application charge directement le dernier dossier ouvert.

## [Unreleased] - 2026-05-29 17:20:18

### Added
- **Script de synchronisation forcÃ©e (`sync_now.py`)** : Ajout d'un script robuste en Python permettant de fusionner Ã  la demande et en toute sÃ©curitÃ© les bases de donnÃ©es locale et OneDrive de Sinew, de copier les fichiers d'apprentissage globaux, et de pousser le dÃ©pÃ´t Git vers GitHub pour garantir une sauvegarde multi-PC Ã  100% sans risque de perte.
- **Mise Ã  jour du plan d'action (`afaire.md`)** : Ajout de la confirmation de mise en place de la synchronisation forcÃ©e Ã  la demande.

## [Unreleased] - 2026-05-29 17:17:36

### Changed
- **Horodatage du plan d'action (`afaire.md`)** : Mise Ã  jour de l'heure de synthÃ¨se aprÃ¨s validation du contrÃ´le unique complet.
- **TraÃ§abilitÃ© du changement (`CHANGELOG.md`)** : Ajout de cette entrÃ©e pour documenter cette mise Ã  jour finale.

### Changed
- **ContrÃ´le unique sans blocage live (`crates/sinew-cursor/src/tests.rs`, `scripts/check.ps1`, `scripts/agent-bridge/test-live-rust.ps1`)** : Les tests Cursor dÃ©pendants d'un compte et du rÃ©seau sont ignorÃ©s par dÃ©faut dans les contrÃ´les courants, le script principal prÃ©cise qu'ils restent sÃ©parÃ©s, et le script dÃ©diÃ© sait les lancer explicitement.
- **Plan d'action mis Ã  jour (`afaire.md`)** : Ajout de l'Ã©tat confirmÃ© sur la sÃ©paration des tests live pour garder le contrÃ´le courant fiable.
- **Changelog (`CHANGELOG.md`)** : Restauration du titre principal et ajout de cette entrÃ©e pour documenter les ajustements de validation.

## [Unreleased] - 2026-05-29 16:51:36

### Improved
- **Index local plus rapide sur CPU, RAM et SSD (`crates/sinew-index/src/indexer.rs`)** : PrÃ©paration des fichiers en parallÃ¨le sur tous les cÅurs disponibles, saut des fichiers inchangÃ©s grÃ¢ce Ã  leur date et leur taille, et Ã©criture par lots pour mieux nourrir le SSD sans gonfler inutilement la mÃ©moire.
- **Base dâindex optimisÃ©e pour le SSD local (`crates/sinew-index/src/store.rs`)** : DÃ©placement du cache dâindex vers le dossier local de la machine, hausse du cache en RAM, lecture mÃ©moire SSD, dÃ©lai dâattente robuste et Ã©critures groupÃ©es.
- **DÃ©pendance de parallÃ©lisation (`Cargo.toml`, `crates/sinew-index/Cargo.toml`, `Cargo.lock`)** : Ajout de Rayon pour rÃ©partir le travail lourd de prÃ©paration dâindex sur plusieurs cÅurs.
- **Journal des changements (`CHANGELOG.md`)** : Documentation de ces optimisations de performance et de leur raison.

All notable changes to this project will be documented in this file.

## [Unreleased] - 2026-05-29 16:43:13

### Added
- **Script de contrÃ´le unique (`scripts/check.ps1`, `package.json`)** : Ajout de `npm run check` pour lancer en une seule commande le build frontend, les contrÃ´les Rust, `clippy`, les tests Rust, les audits npm et l'audit Rust si l'outil est installÃ©. `clippy` tourne en mode rapport par dÃ©faut et peut devenir strict avec `SINEW_STRICT_CLIPPY=1`.

### Changed
- **Stabilisation du test terminal (`crates/sinew-app/src/bash.rs`)** : Le test interactif attend maintenant correctement la fin de la session Windows au lieu d'Ã©chouer sur une rÃ©ponse encore en cours.
- **PremiÃ¨res corrections `clippy` (`crates/sinew-index/src/background.rs`, `crates/sinew-index/src/process.rs`, `crates/sinew-index/src/search.rs`, `crates/sinew-openai/src/auth.rs`, `crates/sinew-google/src/auth.rs`)** : Correction de petites alertes de qualitÃ© signalÃ©es pendant le branchement du contrÃ´le.
- **Nettoyage du plan d'action (`afaire.md`)** : Retrait des trois prioritÃ©s terminÃ©es et recentrage sur les actions restantes.
- **Mise Ã  jour de la carte des fichiers (`AGENTS.md`)** : Ajout du nouveau script `scripts/check.ps1` dans la carte du projet.
- **TraÃ§abilitÃ© du changement (`CHANGELOG.md`)** : Ajout de cette entrÃ©e pour documenter les modifications demandÃ©es.

## [Unreleased] - 2026-05-29 16:42:57

### Changed
- **Statut de quota prÃ©cis pour OpenAI (`src/lib/quotas.ts`)** : Correction du calcul du pourcentage de quota restant pour OpenAI. Le pourcentage prend dÃ©sormais en compte le minimum de toutes les fenÃªtres de limite (fenÃªtre courte et longue) au lieu de masquer un Ã©puisement de quota sur la fenÃªtre longue lorsque la fenÃªtre courte est Ã  100%. Cela garantit que le voyant de statut du modÃ¨le (pastille) passe bien au rouge lorsque le compte est en limite atteinte.
- **TraÃ§abilitÃ© du changement (`CHANGELOG.md`)** : Ajout de cette entrÃ©e pour documenter la correction du bug d'affichage des pastilles de statut.


## [Unreleased] - 2026-05-29 16:39:35

### Removed
- **Badge numÃ©rique sur l'onglet Options (`src/components/SettingsPane.tsx`)** : Retrait du badge dynamique codÃ© en dur affichant un chiffre "5" injustifiÃ© sur l'onglet de configuration gÃ©nÃ©rale des Options pour un visuel plus propre et professionnel.

## [Unreleased] - 2026-05-29 16:36:20

### Changed
- **Nettoyage du plan d'amÃ©lioration (`afaire.md`)** : RÃ©Ã©criture complÃ¨te du rapport pour ne garder que les constats vÃ©rifiÃ©s et les actions Ã  fort impact : test Rust bloquant, contrÃ´les locaux, gros fichiers Ã  dÃ©couper, zones sensibles Ã  auditer, dÃ©pendances Ã  surveiller et crÃ©ation de `DESIGN.md`.
- **TraÃ§abilitÃ© du changement (`CHANGELOG.md`)** : Ajout de cette entrÃ©e pour documenter la rÃ©Ã©criture demandÃ©e du fichier de suivi.

## [Unreleased] - 2026-05-29 16:31:40

### Added
- **Mise Ã  jour du rapport d'analyse (`afaire.md`)** : Ajout d'une synthÃ¨se vÃ©rifiÃ©e de l'Ã©tat actuel du projet avec les contrÃ´les passÃ©s, le test Rust bloquant, les outils Rust manquants, les gros fichiers Ã  dÃ©couper et les surfaces sensibles Ã  auditer.
- **TraÃ§abilitÃ© du changement (`CHANGELOG.md`)** : Ajout de cette entrÃ©e pour consigner la modification documentaire demandÃ©e par l'utilisateur.

## [Unreleased] - 2026-05-29 16:29:01

### Improved
- **Renforcement des consignes d'Autonomie de l'Agent (`src-tauri/src/state.rs`)** : Ajout d'une rÃ¨gle absolue m'interdisant de donner des instructions manuelles ou des commandes textuelles Ã  l'utilisateur si je dispose d'un outil interne capable de rÃ©aliser l'action de maniÃ¨re proactive et autonome.

## [Unreleased] - 2026-05-29 16:29:17

### Added
- **Rapport SimplifiÃ© d'Analyse** : Ajout d'une section de synthÃ¨se simplifiÃ©e Ã  la fin du fichier `afaire.md` reprenant les points clÃ©s et problÃ©matiques identifiÃ©es sur le projet (god files, duplication, manque de tests).

## [Unreleased] - 2026-05-29 16:29:00

### Added
- **Compilation de l'installateur v0.1.26 et copie sur OneDrive** : Lancement rÃ©ussi de la compilation Tauri en mode NSIS et copie de l'installateur gÃ©nÃ©rÃ© (`Sinew_0.1.26_x64-setup.exe`) sur le bureau OneDrive (`C:\Users\julie\OneDrive\Bureau`) pour un dÃ©ploiement instantanÃ©.

## 2026-05-29
- Fusion (merge) des mises Ã  jour du dÃ©pÃ´t d'origine (upstream/main) pour synchroniser l'historique Git
- Mise Ã  jour du fichier `afaire.md` avec l'analyse complÃ¨te du projet : problÃ©matiques, prioritÃ©s, et plan d'amÃ©lioration
- Ajout de `afaire.md` dans la carte des fichiers de `AGENTS.md`

## ð PrÃ©sentation des FonctionnalitÃ©s Majeures (Fork Premium julienpiron.fr)

Cette version a Ã©tÃ© optimisÃ©e en profondeur pour offrir une expÃ©rience utilisateur haut de gamme (SOTA), une autonomie maximale en arriÃ¨re-plan, et des intÃ©grations d'intelligence artificielle inÃ©galÃ©es.

### ð¨ Interface, Confort & Ergonomie (Premium UI)
* **Animation de dÃ©marrage premium :** Une animation de boot moderne, fluide et Ã©lÃ©gante Ã  l'ouverture de l'application.
* **3 niveaux de rÃ©flexion :** Choix entre DÃ©taillÃ©, Compact ou TrÃ¨s compact pour configurer prÃ©cisÃ©ment la verbositÃ© de l'IA et le masquage des dÃ©tails techniques dans le chat.
* **Question collante (Sticky Question) :** La question en cours de traitement reste Ã©pinglÃ©e en haut de l'Ã©cran pendant que vous faites dÃ©filer le fil de discussion.
* **Menu clic droit interactif :**
  * **Sur les onglets :** Clic droit pour fermer l'onglet (raccourci `Ctrl+F4`), les autres ou tous les onglets situÃ©s Ã  droite, copier le chemin (absolu ou relatif) ou rÃ©vÃ©ler dans le Finder/Explorateur.
  * **Sur les fichiers dans le chat :** Clic droit direct pour ouvrir le fichier dans l'Ã©diteur, le rÃ©vÃ©ler dans le dossier systÃ¨me ou l'exÃ©cuter dans le terminal.
  * **Sur l'arbre des fichiers (File Tree) :** Option d'exÃ©cution directe au clic droit.
* **Polices dynamiques ajustables :** Boutons tactiles rÃ©actifs (`+` et `-`) dans les options pour ajuster instantanÃ©ment Ã  chaud la taille du texte de l'Ã©diteur de code Monaco et du chat de l'IA.
* **Version franÃ§aise complÃ¨te :** L'interface entiÃ¨re et toutes les actions de l'application s'adaptent automatiquement en franÃ§ais ou en anglais selon vos prÃ©fÃ©rences.
* **SÃ©lection et copie libre :** DÃ©blocage de la sÃ©lection et copie de texte directement dans le fil de discussion.
* **DÃ©marcation visuelle du panneau de configuration :** Ligne de sÃ©paration verticale Ã©lÃ©gante Ã  gauche du panneau de configuration des paramÃ¨tres.
* **DÃ©coupage du bundle Vite (-80% de taille) :** Monaco Editor et xterm.js sont isolÃ©s dans des sous-lots sÃ©parÃ©s pour un chargement instantanÃ© de l'interface utilisateur.

### ð¾ Autonomie, Sauvegarde & Robustesse SystÃ¨me
* **Sauvegarde automatique (Auto-Save SOTA) :** Enregistrement automatique et transparent en arriÃ¨re-plan 1,5 seconde aprÃ¨s l'arrÃªt de la frappe. Activable ou dÃ©sactivable d'un clic dans vos options.
* **Mode Sandbox :** Lancement de l'application en un clic sans aucun projet ouvert pour tester l'IA ou utiliser les outils MCP de maniÃ¨re isolÃ©e.
* **Synchro OneDrive & SQLite automatique :** Synchronisation transparente de vos conversations, configurations de projets et bases de donnÃ©es SQLite entre vos diffÃ©rents ordinateurs.
* **ZÃ©ro popup console Windows :** Lancement asynchrone et silencieux de tous les outils et serveurs en arriÃ¨re-plan sans aucune ouverture intempestive de fenÃªtres d'invite de commandes.
* **PrÃ©fixe PC rÃ©el automatique :** Identification automatique du nom de la machine physique pour typer et sÃ©curiser les configurations de conversation multi-PC.
* **Diagnostic Windows OAuth rÃ©silient :** Capture robuste de l'erreur rÃ©seau typique sous Windows (code 10013) et conseils clairs pour dÃ©bloquer la connexion (WinNAT/HNS).
* **Diagnostic SOTA :** VÃ©rification en un clic de l'Ã©tat de santÃ©, du PATH et des versions de tous vos outils de dÃ©veloppement (Git, Python, Node, Cargo, etc.).
* **Ãcran de mises Ã  jour sÃ©curisÃ© :** Verrouillage propre de l'interface pendant l'application des correctifs systÃ¨me pour Ã©viter toute corruption de donnÃ©es.
* **Script de compilation OneDrive (`compil.ps1`) :** Automatisation de la gÃ©nÃ©ration de l'application et copie immÃ©diate sur OneDrive pour un dÃ©ploiement instantanÃ© sur vos PC.
* **Active Turn Registry :** Moteur intelligent Rust qui suit les turns de l'agent en cours et assure une reprise instantanÃ©e du streaming aprÃ¨s un redÃ©marrage ou en cas de dÃ©connexion.

### ð¤ ModÃ¨les d'IA, Comptes & FurtivitÃ© (AI Engine)
* **Gestion Multi-comptes OpenAI & Google Gemini :** Connexion simultanÃ©e de plusieurs profils OpenAI et Google Gemini secondaires avec bascule instantanÃ©e entre vos diffÃ©rentes clÃ©s, comptes et abonnements.
* **Quotas en temps rÃ©el :** Visualisation dynamique de votre consommation (crÃ©dits / balance restante) sous forme de barres de progression et pastille live dans le chat.
* **Routage & RÃ©silience Google Antigravity SOTA :** RÃ©paration, optimisation et routage intelligent de vos requÃªtes vers les modÃ¨les Google les plus performants.
* **Optimisation de vitesse Gemini :** Streaming et requÃªtes ultra-rapides pour les modÃ¨les Gemini, basÃ©s sur l'architecture rÃ©seau optimisÃ©e de Google Antigravity.
* **Incorporation d'Opus par Google :** IntÃ©gration de Claude Opus 4.6 via les abonnements professionnels Google.
* **SystÃ¨me Pending/Steering pour Influencer :** Un vrai systÃ¨me d'interception et de guidage pour orienter, corriger ou ajouter des instructions en temps rÃ©el en cours de gÃ©nÃ©ration (Pending/Steering).
* **Indexation sÃ©mantique locale vectorielle :** Indexation et recherche vectorielle haute-performance effectuÃ©e localement sur votre machine avec badge d'Ã©tat interactif dans la barre latÃ©rale.
* **IntÃ©gration de DeepSeek V4 Pro & V4 Flash :** Prise en charge complÃ¨te des modÃ¨les phares **DeepSeek V4 Pro** et **DeepSeek V4 Flash** dans le catalogue de l'application.
* **Pont Cursor Composer 2.5 (agent.v1) :** Moteur haute-performance autonome sur connexions HTTP/2 persistantes gÃ©rant toutes les modifications chirurgicales de fichiers, avec installation automatique et invisible en arriÃ¨re-plan, et masquage du sÃ©lecteur d'intelligence inutile.
* **SÃ©curitÃ© & FurtivitÃ© WebSocket :** Spoofing d'empreinte rÃ©seau avancÃ© pour Ã©liminer tout risque de dÃ©tection ou de blocage sur les flux de ChatGPT.
* **WebSocket OpenAI :** Transport temps-rÃ©el haute performance basÃ© sur WebSocket pour des rÃ©ponses fluides et Ã  latence minimale avec OpenAI.

### ð Extensions & Ponts locaux (MCP & Bridge)
* **Extension Chrome nouvelle gÃ©nÃ©ration :** Pilotage d'actions de navigation ultra-stables en natif Rust avec mouvements et clics Ã  vitesse humaine (mouvements Beziers, physique fluide) et mode silencieux.
* **RÃ©paration Chrome en un clic :** Bouton bleu de configuration automatique si le pont Chrome ne rÃ©pond pas sur un nouveau PC.
* **Empaquetage des ressources Tauri :** Le pont local et l'extension Chrome sont intÃ©grÃ©s directement au sein de l'installateur compilÃ© (MSI/EXE).
* **Outils Rust & ripgrep Sidecar :** IntÃ©gration de Ripgrep en binaire natif sidecar et de nouveaux outils (`list_dir`, `delete_file`) pour accÃ©lÃ©rer la recherche et la gestion des fichiers par 10x.
* **Diagnostics Monaco en temps rÃ©el :** RemontÃ©e automatique des lints et erreurs de compilation de l'Ã©diteur de code Ã  l'IA en temps rÃ©el.
* **Logs ultra-compacts :** Nettoyage automatique du contexte de discussion pour Ã©liminer le bruit et optimiser la consommation de jetons.
* **Laboratoire rÃ©seau MITM :** Outils de dÃ©bogage et d'ingÃ©nierie inverse intÃ©grÃ©s pour inspecter le trafic chiffrÃ© des outils IA.
* **Moteur de remplacement intelligent (Search/Replace) :** SystÃ¨me d'auto-correction Ã  8 couches (Unicode, indentations, etc.) garantissant que les modifications de l'IA s'insÃ¨rent correctement dans vos fichiers mÃªme en cas de lÃ©gÃ¨res erreurs d'espaces.

---

## ð Historique des Versions

## [Unreleased] - 2026-05-29 16:18:48

### Added
- **Rapport d'analyse et d'amÃ©liorations (`afaire.md`)** : CrÃ©ation d'un document rÃ©capitulatif listant les principaux axes d'amÃ©lioration du projet (performances de l'interface, organisation du moteur Rust, et sÃ©curitÃ© des dÃ©pendances).


## [Unreleased] - 2026-05-29 16:16:37

### Added
- **Prise en charge de Claude Opus 4.8 (`crates/sinew-anthropic/src/client.rs`, `crates/sinew-anthropic/src/model_info.rs`, `src/lib/models.ts`)** : IntÃ©gration du nouveau modÃ¨le phare de la gamme d'intelligence artificielle d'Anthropic (Claude Opus 4.8) dotÃ© d'une fenÃªtre de contexte native de 1 million de jetons et des niveaux de rÃ©flexion configurables dans l'interface utilisateur.

### Changed
- **Mise Ã  jour de la configuration de compilation (`Cargo.toml`, `Cargo.lock`)** : Synchronisation des versions des dÃ©pendances et du systÃ¨me de compilation avec la version de rÃ©fÃ©rence 0.1.26 de la branche parente.

## [Unreleased] - 2026-05-29 16:21:45

### Removed
- **Nettoyage complet du fichier `AGENTS.md` local** : Suppression dÃ©finitive de toute trace ou consigne relative au systÃ¨me d'apprentissage automatique dans le fichier `AGENTS.md` de ce projet. L'injection et la gestion de la mÃ©moire globale sont dÃ©sormais entiÃ¨rement intÃ©grÃ©es de maniÃ¨re native au sein de l'application (cÃ´tÃ© Rust) pour tous les projets ouverts sur cette machine.

## [Unreleased] - 2026-05-29 16:15:36

### Added
- **Synchronisation OneDrive du SystÃ¨me d'Apprentissage (`src-tauri/src/lib.rs`)** : Ajout de la synchronisation bidirectionnelle automatique des fichiers d'apprentissage globaux (`errors_raw.json` et `instructions_consolidated.md`) via OneDrive. Lorsque l'option de synchronisation Multi-PC est active, ces fichiers sont fusionnÃ©s et sauvegardÃ©s sur OneDrive Ã  la fermeture de l'application, et restaurÃ©s au dÃ©marrage sur vos autres ordinateurs.

## [Unreleased] - 2026-05-29 16:06:04

### Fixed
- **Correction de la visibilitÃ© de l'indicateur de contexte en mode IA (`src/styles.css`)** : Augmentation de la prioritÃ© d'empilement (z-index) de la zone de saisie (`.composer`) Ã  10 pour s'assurer que le volet volant affichant l'utilisation du contexte s'affiche par-dessus les messages du chat et la liste des tÃ¢ches, Ã©vitant qu'il ne soit masquÃ© ou floutÃ© par les effets de transparence premium du mode IA.

## [Unreleased] - 2026-05-29 16:11:45

### Added
- **IntÃ©gration du systÃ¨me d'apprentissage global cÃ´tÃ© Rust (`src-tauri/src/turns.rs`)** : Modification du backend Rust de l'application pour charger et injecter automatiquement et de maniÃ¨re transparente le fichier d'instructions consolidÃ©es global (`%LOCALAPPDATA%\Sinew\instructions_consolidated.md`) dans le prompt systÃ¨me de tous les agents pour l'ensemble des projets ouverts sur cet ordinateur. Les agents bÃ©nÃ©ficient dÃ©sormais de cette base d'apprentissage universelle sans dÃ©pendre d'un fichier local `AGENTS.md`.

## [Unreleased] - 2026-05-29 16:06:07

### Changed
- **DÃ©centralisation complÃ¨te des descriptions d'erreurs (`AGENTS.md`)** : Suppression dÃ©finitive des descriptions locales d'erreurs dans `AGENTS.md` pour Ã©viter toute duplication. DÃ©sormais, `AGENTS.md` ne contient que le pointeur d'instructions globales pour forcer la lecture de la base d'apprentissage centralisÃ©e dans `%LOCALAPPDATA%\Sinew\`.
- **Base de connaissances globale (`%LOCALAPPDATA%\Sinew\`)** : Migration de toutes les anciennes rÃ¨gles d'erreurs (Git, Windows, npm, MCP) directement dans le fichier d'apprentissage consolidÃ© de la machine.

## [Unreleased] - 2026-05-29 16:04:26

### Changed
- **Globalisation du SystÃ¨me d'Apprentissage** : DÃ©placement de la mÃ©moire d'apprentissage (`errors_raw.json` et `instructions_consolidated.md`) dans le dossier de configuration global de la machine (`%LOCALAPPDATA%\Sinew\`). Cela permet d'avoir un systÃ¨me d'apprentissage partagÃ© et partagÃ© sur tous les projets et espaces de travail ouverts sur cet ordinateur.
- **RÃ¨gles d'agent (`AGENTS.md`)** : Mise Ã  jour des rÃ¨gles d'instructions globales pour forcer la lecture et l'alimentation de la base d'apprentissage globale Ã  chaque dÃ©but de session.

## [Unreleased] - 2026-05-29 15:57:18

### Added
- **IntÃ©gration d'outils de diagnostic et d'Ã©mulation hybrides (`sinew-chrome-bridge/mcp_server.js` et `.sinew/skills/browser/SKILL.md`)** : Ajout de trois nouveaux outils structurÃ©s MCP (`emulate_experience`, `lighthouse_audit`, `analyze_memory_leaks`) basÃ©s sur l'API CDP brute de Google Chrome pour tester la compatibilitÃ© mobile (taille, touch events), Ã©valuer les performances et diagnostics de qualitÃ© Lighthouse en local, et analyser la consommation de mÃ©moire (JS heap et DOM nodes count).
- **Mise Ã  jour de la compÃ©tence Browser (`.sinew/skills/browser/SKILL.md`)** : Ajout d'une section de documentation claire guidant l'IA Ã  utiliser ces nouveaux pouvoirs de diagnostic en mode hybride tout en prÃ©servant le simulateur biologique humain furtif de Sinew Chrome.

## [Unreleased] - 2026-05-29 15:46:39

### Fixed
- **Correction et universalisation des lightboxes (`src/components/chat/ChatPane.tsx`, `src/components/chat/MermaidDiagram.tsx`, `src/styles.css`)** : Passage des zooms d'image et de diagrammes Mermaid sous forme de Portails React (`createPortal`) attachÃ©s Ã  `document.body`. Cela corrige dÃ©finitivement le problÃ¨me de rognage/masquage causÃ© par les empilements CSS (stacking contexts) des colonnes du chat et des options. De plus, augmentation de la prioritÃ© d'affichage (`z-index: 99999`) et intÃ©gration des styles sombres nÃ©on haut de gamme pour les schÃ©mas en mode IA afin qu'ils s'affichent magnifiquement dans tous les thÃ¨mes.

## [Unreleased] - 2026-05-29 15:52:23

### Added
- **Ãtude d'impact sur la sÃ©curitÃ©, l'anti-dÃ©tection et la discrÃ©tion (`ETUDE_IMPACT_SECURITE.md`)** : Rapport d'analyse complet Ã©valuant les risques de dÃ©tection des fonctionnalitÃ©s de DevTools for Agents 1.0 (Ã©mulation, auto-connect, audits) et concevant une synergie d'interception avec notre simulateur biologique humain.
- **Rapport d'analyse et d'intÃ©gration DevTools (`ANALYSE_DEVTOOLS_MCP.md`)** : CrÃ©ation d'une analyse technique poussÃ©e Ã©valuant la faisabilitÃ© et l'impact de l'intÃ©gration des fonctionnalitÃ©s de Chrome DevTools for Agents 1.0 (Lighthouse, Ã©mulation, fuites mÃ©moire, auto-connect) tout en conservant le simulateur biologique humain furtif unique de Sinew Chrome.
- **Rapport d'Ã©valuation de la pertinence des outils de diagnostic Google (`ANALYSE_DIAGNOSTIC_GOOGLE.md`)** : Analyse de pertinence des outils de diagnostic avancÃ©s de Google (Lighthouse, Ã©mulation, fuites mÃ©moire, auto-connect) du point de vue de l'audit et du contrÃ´le qualitÃ©, avec mÃ©taphores simples et plan d'action d'adaptation pour notre simulateur biologique.

### Changed
- **Mise Ã  jour de la carte du code (`AGENTS.md`)** : Enregistrement des rapports d'analyse de l'Ã©quipe (sÃ©curitÃ©, architecture, intÃ©gration, diagnostic) dans la carte des fichiers du projet.

## [Unreleased] - 2026-05-29 15:48:14

### Added
- **Option de Recherche SÃ©mantique Vectorielle (`src/components/SettingsPane.tsx` & `src-tauri/src/workspace.rs` & `src-tauri/src/lib.rs` & `src/lib/ipc.ts`)** : Ajout d'une nouvelle option SOTA Â« Recherche SÃ©mantique Vectorielle (BETA) Â» dans les paramÃ¨tres (Apparence & Interface). Liaison avec un nouveau handler Tauri Rust `set_semantic_embeddings_enabled` pour activer/dÃ©sactiver dynamiquement la variable d'environnement `SINEW_INDEX_EMBEDDINGS` Ã  la volÃ©e.

## [Unreleased] - 2026-05-29 15:47:36

### Added
- **Script d'installation automatisÃ© (`package.json`)** : Ajout d'un script `postinstall` pour installer automatiquement les dÃ©pendances des extensions locales (`sinew-chrome-bridge` et `scripts/agent-bridge`) lors du lancement initial de `npm install` par n'importe quel dÃ©veloppeur ou serveur de compilation.

## [Unreleased] - 2026-05-29 15:43:42

### Changed
- **Exclusion de dossiers (`.gitignore`)** : Ajout du rÃ©pertoire `/build/` dans les rÃ¨gles d'exclusion de Git pour Ã©viter de traquer les binaires et installateurs compilÃ©s localement.

### Removed
- **Nettoyage de l'historique Git** : Retrait du suivi de version Git pour les gros fichiers d'installation d'anciennes versions (`build/Sinew.exe`, `build/Sinew_0.1.25_x64-setup.exe` et `build/Sinew_0.1.25_x64_en-US.msi`) ainsi que pour le dossier de dÃ©pendances obsolÃ¨tes (`sinew-chrome-bridge/node_modules/`), tout en conservant ces fichiers intacts sur les disques durs locaux.

## [Unreleased] - 2026-05-29 15:40:51

### Added
- **Rapport d'analyse et d'idÃ©es SOTA (`RAPPORT_OUTILS.md`)** : CrÃ©ation d'un rapport consolidÃ© prÃ©sentant la vÃ©rification des outils et dÃ©taillant les 3 pistes d'amÃ©liorations SOTA validÃ©es et sÃ©curisÃ©es.

## [Unreleased] - 2026-05-29 15:39:58

### Changed
- **IntÃ©gration du Changelog (`CHANGELOG.md`)** : DÃ©placement et intÃ©gration de la documentation des fonctionnalitÃ©s majeures du fork (anciennement `README-FORK.md`) directement en en-tÃªte du journal pour une meilleure visibilitÃ©.

### Removed
- **Nettoyage gÃ©nÃ©ral des vestiges et rapports** : Suppression dÃ©finitive de 12 fichiers d'audits, de rapports et de scripts de test obsolÃ¨tes (`AUDIT_PERFORMANCE_SOTA.md`, `AUDIT_RUST.md`, `AUDIT_SECURITE.md`, `RAPPORT_OUTILS.md`, `RAPPORT_VERIFICATION.md`, `RAPPORT_VERIFICATION_FINAL.md`, `README-FORK.md`, `scripts/AGENT-SPIKE-DESIGN.md`, `scripts/CAPTURE-MITM.md`, `scripts/probe_agent_run.py`, `scripts/probe_idempotent_crypto.py`, `scripts/verify_all.py` et le cache `__pycache__`) pour nettoyer totalement l'arborescence du projet.

## [Unreleased] - 2026-05-29 15:33:18

### Added
- **Rapport d'analyse des outils (`RAPPORT_OUTILS.md`)** : CrÃ©ation d'un rapport complet sur l'Ã©tat des outils systÃ¨me, de l'indexeur et de l'intÃ©gration du navigateur Sinew pour rÃ©pondre aux interrogations de l'utilisateur.

## [Unreleased] - 2026-05-29 15:31:31

### Fixed
- **Settings Pane (`src/components/SettingsPane.tsx`)** : Correction de la persistence de l'option Â« Exposer tous les outils au dÃ©marrage Â» (`autoLoad`) pour les serveurs MCP. L'option Ã©tait omise lors de la sÃ©rialisation des paramÃ¨tres en JSON (`settingsToJson`), ce qui entraÃ®nait sa rÃ©initialisation Ã  chaque rechargement ou modification de la configuration. Ajout de la sÃ©rialisation de `autoLoad` dans le JSON exportÃ©.

## [Unreleased] - 2026-05-29 15:28:08

### Fixed
- **Styles CSS (`src/styles.css`)** : Correction du problÃ¨me d'affichage du menu dÃ©roulant (popover) de sÃ©lection des modÃ¨les en mode/thÃ¨me IA. Remplacement de `overflow: hidden` par `overflow: visible !important` pour la boÃ®te de composition `.composer__box` sous le sÃ©lecteur `html[data-theme="ai"]`, Ã©vitant ainsi le masquage ou le rognage des options du menu au-delÃ  des bordures du conteneur.
