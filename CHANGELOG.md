# Changelog


## [Unreleased] - 2026-05-30 00:36:50

### Changed
- **Rapport d'analyse complet (faire.md)** : R��criture int�grale avec analyse approfondie de l'architecture (5 god files), qualit� (250+ tests, 0 frontend), s�curit� (Tauri, terminal, Chrome), dettes techniques (clippy, CHANGELOG, d�pendances) et plan d'action prioris�.
- **Journal des changements (CHANGELOG.md)** : Ajout de cette entr�e.

## [Unreleased] - 2026-05-30 00:45:00

### Changed
- **Identification universelle par Git remote URL (`src-tauri/src/workspace.rs`, `src-tauri/src/conversations.rs`)** : Priorisation de l'URL distante Git (config remote.origin.url) comme identifiant de projet unique universel. Si un dépôt Git est présent, les deux ordinateurs partagent le même identifiant de projet sans générer d'UUIDs locaux différents, rendant la synchronisation 100% automatique sans devoir lier manuellement. Migration automatique des anciennes conversations stockées sous le chemin ou sous l'UUID local vers ce nouvel identifiant.

## [Unreleased] - 2026-05-30 00:32:00

### Fixed
- **Normalisation et exclusion de l'UUID du projet actuel dans la liste des autres workspaces (`crates/sinew-app/src/store.rs`, `src-tauri/src/conversations.rs`)** : Correction du bug où l'identifiant unique (UUID) du projet actuel s'affichait comme un autre projet dans la liste de détection. Amélioration de la robustesse de la migration de chemin en ignorant la casse et la forme des slashes (Windows).

## [Unreleased] - 2026-05-30 00:13:17

### Changed
- **Fermeture de la lightbox d'image au clic extérieur (`src/components/chat/ChatPane.tsx`)** : Permet de fermer l'aperçu de l'image (lightbox) en cliquant n'importe où autour de celle-ci, tout en empêchant la fermeture accidentelle lors d'un clic sur l'image elle-même ou sur les boutons de la barre d'outils.

## [Unreleased] - 2026-05-30 00:11:30

### Changed
- **Comportement et description des Maquettes Visuelles (`src-tauri/src/turns.rs`, `src/components/SettingsPane.tsx`)** : Ajustement de la consigne et de la description de l'option « Maquettes Visuelles Automatiques ». Désormais, la maquette Mermaid n'est plus obligatoire à chaque changement d'interface ; elle n'est générée que si vous en faites la demande expresse ou si je l'estime nécessaire pour un changement complexe.

## [Unreleased] - 2026-05-30 00:09:19

### Changed
- **Description de l'Autonomie de l'Agent (`src/components/SettingsPane.tsx`)** : Clarification de la description de l'option « Autonomie de l'Agent » en français et en anglais pour mieux expliquer que son but est de m'obliger à agir directement avec mes outils de codage plutôt que de lister des instructions manuelles à faire vous-même.

## [Unreleased] - 2026-05-30 00:03:14

### Changed
- **Mode de recherche de mise à jour à 3 options (`src/components/SettingsPane.tsx`, `src/App.tsx`, `src/components/UpdateBadge.tsx`)** : Évolution de l'option de mise à jour pour proposer trois choix : "Bloquant" (vérifie et force la mise à jour au démarrage), "Notification uniquement" (démarre normalement et alerte discrètement via un badge interne), et "Désactivé" (ne vérifie jamais les mises à jour automatiquement).

## [Unreleased] - 2026-05-29 23:59:57

### Added
- **Option de recherche de mise à jour automatique (`src/components/SettingsPane.tsx`, `src/App.tsx`, `src/components/UpdateBadge.tsx`)** : Ajout d'une option dans le panneau de configuration pour activer/désactiver la recherche de mise à jour automatique. Si désactivée, l'application ne recherche plus de nouvelles versions au démarrage (ce qui évite de bloquer l'interface utilisateur avec l'écran "Mise à jour requise") ni périodiquement en arrière-plan.

## [Unreleased] - 2026-05-29 23:45:15

### Added
- **Identifiant de projet universel par fichier caché (`crates/sinew-app/src/store.rs`, `src-tauri/src/workspace.rs`, `src-tauri/src/conversations.rs`, `src-tauri/Cargo.toml`)** : Introduction d'un identifiant unique de projet (UUID) généré automatiquement dans le fichier caché `.sinew/project_id.txt` à la racine de chaque projet (qu'il utilise Git ou non). Ce fichier se synchronise automatiquement via OneDrive/Git, ce qui permet à l'application de lier instantanément vos conversations d'un ordinateur à l'autre sans action manuelle, même si les chemins ou les noms des dossiers diffèrent.

## [Unreleased] - 2026-05-29 23:42:04

### Added
- **Synchronisation automatique par URL Git (`crates/sinew-app/src/store.rs`, `src-tauri/src/git.rs`, `src-tauri/src/workspace.rs`, `src-tauri/src/conversations.rs`, `src-tauri/src/turns.rs`, `src-tauri/src/swarm.rs`)** : Les conversations sont désormais associées au dépôt Git distant (remote origin URL). Lorsque vous ouvrez un dépôt Git, l'application détecte automatiquement les conversations correspondantes issues d'autres PC/dossiers (synchronisés par OneDrive) et les lie automatiquement à votre dossier de projet actuel, éliminant tout besoin d'action manuelle.

## [Unreleased] - 2026-05-29 23:36:07

### Added
- **Association de conversations de projets / PC alternatifs (`crates/sinew-app/src/store.rs`, `src-tauri/src/conversations.rs`, `src/lib/ipc.ts`, `src/components/SettingsPane.tsx`)** : Ajout de la possibilité de détecter et de lier des conversations provenant d'autres dossiers de projets (comme vos dossiers de travail synchronisés par OneDrive) directement depuis les paramètres.
- **Rafraîchissement dynamique des conversations (`src/components/Workspace.tsx`)** : Écouteur d'événement global pour rafraîchir instantanément la liste des conversations du projet actif lors d'une migration ou synchronisation.

## [Unreleased] - 2026-05-29 23:34:58

### Changed
- **Options d'affichage du démarrage des serveurs MCP (`src/components/SettingsPane.tsx`, `src/styles.css`)** : Alignement de l'option "Exposer tous les outils au démarrage" sur une seule ligne avec le bouton de bascule (toggle switch) aligné à droite, au lieu d'être compressé sur une largeur étroite de 28px.
- **Correction des types TypeScript dans le panneau des options (`src/components/SettingsPane.tsx`)** : Passage de `workspacePath` à `OptionsSection` pour corriger les erreurs de compilation liées à la migration des conversations entre projets.

## [Unreleased] - 2026-05-29 23:34:08

### Changed
- **Comportement des blocs d'outils en erreur en mode très compact (`src/components/chat/ToolCard.tsx`)** : En mode très compact, les cartes d'outils ayant échoué (comme l'exécution de scripts Bash ou Python) démarrent désormais repliées (fermées) par défaut au lieu de s'ouvrir automatiquement. L'utilisateur peut toujours cliquer dessus pour les déplier et voir les détails de l'erreur.


## [Unreleased] - 2026-05-29 23:24:18

### Fixed
- **Sensibilité à la casse des chemins de projet sous Windows (`crates/sinew-app/src/workspace.rs`, `crates/sinew-app/src/store.rs`)** : Normalisation en minuscules de l'identifiant unique du dossier de projet (`workspace_id`) sur Windows. Ajout d'une migration SQLite pour mettre à jour automatiquement les anciennes entrées de la base de données. Cela corrige le bug où les discussions créées au travail dans un dossier (ex: `C:\Dev\Sinew`) n'étaient pas affichées à la maison si le dossier avait une casse légèrement différente (ex: `C:\Dev\sinew`).

### Added
- **Bouton de synchronisation forcée dans les paramètres (`src/components/SettingsPane.tsx`)** : Ajout d'un bouton "Synchroniser maintenant" sous l'option de synchronisation Multi-PC dans `OptionsSection` pour permettre à l'utilisateur de déclencher manuellement et à tout moment la synchronisation bidirectionnelle OneDrive et Git.
- **Commande Tauri de synchronisation forcée (`src-tauri/src/lib.rs`, `src/lib/ipc.ts`)** : Ajout de la commande `force_multi_pc_sync` dans Rust et exposition dans le pont IPC frontend pour permettre à l'utilisateur de déclencher manuellement une synchronisation bidirectionnelle complète des bases de données et des dépôts Git avec OneDrive à la demande.

### Changed
- **Correction de la synchronisation Git Multi-PC (`src-tauri/src/lib.rs`)** : Suppression du préfixe UNC `\\?\` des chemins locaux de l'espace de travail avant de définir le répertoire de travail (`current_dir`) pour les commandes Git (`git status`, `git pull`, `git commit`, `git push`). Cela résout l'erreur `CMD ne prend pas les chemins UNC comme répertoires en cours` (code d'erreur 128) et rétablit la synchronisation automatique transparente entre les différents ordinateurs.

## [Unreleased] - 2026-05-29 23:20:51

### Changed
- **Correction de la synchronisation Git Multi-PC (`src-tauri/src/lib.rs`)** : Suppression du préfixe UNC `\\?\` des chemins locaux de l'espace de travail avant de définir le répertoire de travail (`current_dir`) pour les commandes Git (`git status`, `git pull`, `git commit`, `git push`). Cela résout l'erreur `CMD ne prend pas les chemins UNC comme répertoires en cours` (code d'erreur 128) et rétablit la synchronisation automatique transparente entre les différents ordinateurs.

## [Unreleased] - 2026-05-29 23:20:17

### Changed
- **Ignorer définitivement la demande de mise à jour au démarrage (`src/App.tsx`)** : Permet de ne plus afficher l'écran de mise à jour bloquante pour une version spécifique lorsque l'utilisateur clique sur "Skip". Le choix est sauvegardé localement, et l'application charge directement le dernier dossier ouvert.

## [Unreleased] - 2026-05-29 17:20:18

### Added
- **Script de synchronisation forcée (`sync_now.py`)** : Ajout d'un script robuste en Python permettant de fusionner à la demande et en toute sécurité les bases de données locale et OneDrive de Sinew, de copier les fichiers d'apprentissage globaux, et de pousser le dépôt Git vers GitHub pour garantir une sauvegarde multi-PC à 100% sans risque de perte.
- **Mise à jour du plan d'action (`afaire.md`)** : Ajout de la confirmation de mise en place de la synchronisation forcée à la demande.

## [Unreleased] - 2026-05-29 17:17:36

### Changed
- **Horodatage du plan d'action (`afaire.md`)** : Mise à jour de l'heure de synthèse après validation du contrôle unique complet.
- **Traçabilité du changement (`CHANGELOG.md`)** : Ajout de cette entrée pour documenter cette mise à jour finale.

### Changed
- **Contrôle unique sans blocage live (`crates/sinew-cursor/src/tests.rs`, `scripts/check.ps1`, `scripts/agent-bridge/test-live-rust.ps1`)** : Les tests Cursor dépendants d'un compte et du réseau sont ignorés par défaut dans les contrôles courants, le script principal précise qu'ils restent séparés, et le script dédié sait les lancer explicitement.
- **Plan d'action mis à jour (`afaire.md`)** : Ajout de l'état confirmé sur la séparation des tests live pour garder le contrôle courant fiable.
- **Changelog (`CHANGELOG.md`)** : Restauration du titre principal et ajout de cette entrée pour documenter les ajustements de validation.

## [Unreleased] - 2026-05-29 16:51:36

### Improved
- **Index local plus rapide sur CPU, RAM et SSD (`crates/sinew-index/src/indexer.rs`)** : Préparation des fichiers en parallèle sur tous les cœurs disponibles, saut des fichiers inchangés grâce à leur date et leur taille, et écriture par lots pour mieux nourrir le SSD sans gonfler inutilement la mémoire.
- **Base d’index optimisée pour le SSD local (`crates/sinew-index/src/store.rs`)** : Déplacement du cache d’index vers le dossier local de la machine, hausse du cache en RAM, lecture mémoire SSD, délai d’attente robuste et écritures groupées.
- **Dépendance de parallélisation (`Cargo.toml`, `crates/sinew-index/Cargo.toml`, `Cargo.lock`)** : Ajout de Rayon pour répartir le travail lourd de préparation d’index sur plusieurs cœurs.
- **Journal des changements (`CHANGELOG.md`)** : Documentation de ces optimisations de performance et de leur raison.

All notable changes to this project will be documented in this file.

## [Unreleased] - 2026-05-29 16:43:13

### Added
- **Script de contrôle unique (`scripts/check.ps1`, `package.json`)** : Ajout de `npm run check` pour lancer en une seule commande le build frontend, les contrôles Rust, `clippy`, les tests Rust, les audits npm et l'audit Rust si l'outil est installé. `clippy` tourne en mode rapport par défaut et peut devenir strict avec `SINEW_STRICT_CLIPPY=1`.

### Changed
- **Stabilisation du test terminal (`crates/sinew-app/src/bash.rs`)** : Le test interactif attend maintenant correctement la fin de la session Windows au lieu d'échouer sur une réponse encore en cours.
- **Premières corrections `clippy` (`crates/sinew-index/src/background.rs`, `crates/sinew-index/src/process.rs`, `crates/sinew-index/src/search.rs`, `crates/sinew-openai/src/auth.rs`, `crates/sinew-google/src/auth.rs`)** : Correction de petites alertes de qualité signalées pendant le branchement du contrôle.
- **Nettoyage du plan d'action (`afaire.md`)** : Retrait des trois priorités terminées et recentrage sur les actions restantes.
- **Mise à jour de la carte des fichiers (`AGENTS.md`)** : Ajout du nouveau script `scripts/check.ps1` dans la carte du projet.
- **Traçabilité du changement (`CHANGELOG.md`)** : Ajout de cette entrée pour documenter les modifications demandées.

## [Unreleased] - 2026-05-29 16:42:57

### Changed
- **Statut de quota précis pour OpenAI (`src/lib/quotas.ts`)** : Correction du calcul du pourcentage de quota restant pour OpenAI. Le pourcentage prend désormais en compte le minimum de toutes les fenêtres de limite (fenêtre courte et longue) au lieu de masquer un épuisement de quota sur la fenêtre longue lorsque la fenêtre courte est à 100%. Cela garantit que le voyant de statut du modèle (pastille) passe bien au rouge lorsque le compte est en limite atteinte.
- **Traçabilité du changement (`CHANGELOG.md`)** : Ajout de cette entrée pour documenter la correction du bug d'affichage des pastilles de statut.


## [Unreleased] - 2026-05-29 16:39:35

### Removed
- **Badge numérique sur l'onglet Options (`src/components/SettingsPane.tsx`)** : Retrait du badge dynamique codé en dur affichant un chiffre "5" injustifié sur l'onglet de configuration générale des Options pour un visuel plus propre et professionnel.

## [Unreleased] - 2026-05-29 16:36:20

### Changed
- **Nettoyage du plan d'amélioration (`afaire.md`)** : Réécriture complète du rapport pour ne garder que les constats vérifiés et les actions à fort impact : test Rust bloquant, contrôles locaux, gros fichiers à découper, zones sensibles à auditer, dépendances à surveiller et création de `DESIGN.md`.
- **Traçabilité du changement (`CHANGELOG.md`)** : Ajout de cette entrée pour documenter la réécriture demandée du fichier de suivi.

## [Unreleased] - 2026-05-29 16:31:40

### Added
- **Mise à jour du rapport d'analyse (`afaire.md`)** : Ajout d'une synthèse vérifiée de l'état actuel du projet avec les contrôles passés, le test Rust bloquant, les outils Rust manquants, les gros fichiers à découper et les surfaces sensibles à auditer.
- **Traçabilité du changement (`CHANGELOG.md`)** : Ajout de cette entrée pour consigner la modification documentaire demandée par l'utilisateur.

## [Unreleased] - 2026-05-29 16:29:01

### Improved
- **Renforcement des consignes d'Autonomie de l'Agent (`src-tauri/src/state.rs`)** : Ajout d'une règle absolue m'interdisant de donner des instructions manuelles ou des commandes textuelles à l'utilisateur si je dispose d'un outil interne capable de réaliser l'action de manière proactive et autonome.

## [Unreleased] - 2026-05-29 16:29:17

### Added
- **Rapport Simplifié d'Analyse** : Ajout d'une section de synthèse simplifiée à la fin du fichier `afaire.md` reprenant les points clés et problématiques identifiées sur le projet (god files, duplication, manque de tests).

## [Unreleased] - 2026-05-29 16:29:00

### Added
- **Compilation de l'installateur v0.1.26 et copie sur OneDrive** : Lancement réussi de la compilation Tauri en mode NSIS et copie de l'installateur généré (`Sinew_0.1.26_x64-setup.exe`) sur le bureau OneDrive (`C:\Users\julie\OneDrive\Bureau`) pour un déploiement instantané.

## 2026-05-29
- Fusion (merge) des mises à jour du dépôt d'origine (upstream/main) pour synchroniser l'historique Git
- Mise à jour du fichier `afaire.md` avec l'analyse complète du projet : problématiques, priorités, et plan d'amélioration
- Ajout de `afaire.md` dans la carte des fichiers de `AGENTS.md`

## 🚀 Présentation des Fonctionnalités Majeures (Fork Premium julienpiron.fr)

Cette version a été optimisée en profondeur pour offrir une expérience utilisateur haut de gamme (SOTA), une autonomie maximale en arrière-plan, et des intégrations d'intelligence artificielle inégalées.

### 🎨 Interface, Confort & Ergonomie (Premium UI)
* **Animation de démarrage premium :** Une animation de boot moderne, fluide et élégante à l'ouverture de l'application.
* **3 niveaux de réflexion :** Choix entre Détaillé, Compact ou Très compact pour configurer précisément la verbosité de l'IA et le masquage des détails techniques dans le chat.
* **Question collante (Sticky Question) :** La question en cours de traitement reste épinglée en haut de l'écran pendant que vous faites défiler le fil de discussion.
* **Menu clic droit interactif :**
  * **Sur les onglets :** Clic droit pour fermer l'onglet (raccourci `Ctrl+F4`), les autres ou tous les onglets situés à droite, copier le chemin (absolu ou relatif) ou révéler dans le Finder/Explorateur.
  * **Sur les fichiers dans le chat :** Clic droit direct pour ouvrir le fichier dans l'éditeur, le révéler dans le dossier système ou l'exécuter dans le terminal.
  * **Sur l'arbre des fichiers (File Tree) :** Option d'exécution directe au clic droit.
* **Polices dynamiques ajustables :** Boutons tactiles réactifs (`+` et `-`) dans les options pour ajuster instantanément à chaud la taille du texte de l'éditeur de code Monaco et du chat de l'IA.
* **Version française complète :** L'interface entière et toutes les actions de l'application s'adaptent automatiquement en français ou en anglais selon vos préférences.
* **Sélection et copie libre :** Déblocage de la sélection et copie de texte directement dans le fil de discussion.
* **Démarcation visuelle du panneau de configuration :** Ligne de séparation verticale élégante à gauche du panneau de configuration des paramètres.
* **Découpage du bundle Vite (-80% de taille) :** Monaco Editor et xterm.js sont isolés dans des sous-lots séparés pour un chargement instantané de l'interface utilisateur.

### 💾 Autonomie, Sauvegarde & Robustesse Système
* **Sauvegarde automatique (Auto-Save SOTA) :** Enregistrement automatique et transparent en arrière-plan 1,5 seconde après l'arrêt de la frappe. Activable ou désactivable d'un clic dans vos options.
* **Mode Sandbox :** Lancement de l'application en un clic sans aucun projet ouvert pour tester l'IA ou utiliser les outils MCP de manière isolée.
* **Synchro OneDrive & SQLite automatique :** Synchronisation transparente de vos conversations, configurations de projets et bases de données SQLite entre vos différents ordinateurs.
* **Zéro popup console Windows :** Lancement asynchrone et silencieux de tous les outils et serveurs en arrière-plan sans aucune ouverture intempestive de fenêtres d'invite de commandes.
* **Préfixe PC réel automatique :** Identification automatique du nom de la machine physique pour typer et sécuriser les configurations de conversation multi-PC.
* **Diagnostic Windows OAuth résilient :** Capture robuste de l'erreur réseau typique sous Windows (code 10013) et conseils clairs pour débloquer la connexion (WinNAT/HNS).
* **Diagnostic SOTA :** Vérification en un clic de l'état de santé, du PATH et des versions de tous vos outils de développement (Git, Python, Node, Cargo, etc.).
* **Écran de mises à jour sécurisé :** Verrouillage propre de l'interface pendant l'application des correctifs système pour éviter toute corruption de données.
* **Script de compilation OneDrive (`compil.ps1`) :** Automatisation de la génération de l'application et copie immédiate sur OneDrive pour un déploiement instantané sur vos PC.
* **Active Turn Registry :** Moteur intelligent Rust qui suit les turns de l'agent en cours et assure une reprise instantanée du streaming après un redémarrage ou en cas de déconnexion.

### 🤖 Modèles d'IA, Comptes & Furtivité (AI Engine)
* **Gestion Multi-comptes OpenAI & Google Gemini :** Connexion simultanée de plusieurs profils OpenAI et Google Gemini secondaires avec bascule instantanée entre vos différentes clés, comptes et abonnements.
* **Quotas en temps réel :** Visualisation dynamique de votre consommation (crédits / balance restante) sous forme de barres de progression et pastille live dans le chat.
* **Routage & Résilience Google Antigravity SOTA :** Réparation, optimisation et routage intelligent de vos requêtes vers les modèles Google les plus performants.
* **Optimisation de vitesse Gemini :** Streaming et requêtes ultra-rapides pour les modèles Gemini, basés sur l'architecture réseau optimisée de Google Antigravity.
* **Incorporation d'Opus par Google :** Intégration de Claude Opus 4.6 via les abonnements professionnels Google.
* **Système Pending/Steering pour Influencer :** Un vrai système d'interception et de guidage pour orienter, corriger ou ajouter des instructions en temps réel en cours de génération (Pending/Steering).
* **Indexation sémantique locale vectorielle :** Indexation et recherche vectorielle haute-performance effectuée localement sur votre machine avec badge d'état interactif dans la barre latérale.
* **Intégration de DeepSeek V4 Pro & V4 Flash :** Prise en charge complète des modèles phares **DeepSeek V4 Pro** et **DeepSeek V4 Flash** dans le catalogue de l'application.
* **Pont Cursor Composer 2.5 (agent.v1) :** Moteur haute-performance autonome sur connexions HTTP/2 persistantes gérant toutes les modifications chirurgicales de fichiers, avec installation automatique et invisible en arrière-plan, et masquage du sélecteur d'intelligence inutile.
* **Sécurité & Furtivité WebSocket :** Spoofing d'empreinte réseau avancé pour éliminer tout risque de détection ou de blocage sur les flux de ChatGPT.
* **WebSocket OpenAI :** Transport temps-réel haute performance basé sur WebSocket pour des réponses fluides et à latence minimale avec OpenAI.

### 🔌 Extensions & Ponts locaux (MCP & Bridge)
* **Extension Chrome nouvelle génération :** Pilotage d'actions de navigation ultra-stables en natif Rust avec mouvements et clics à vitesse humaine (mouvements Beziers, physique fluide) et mode silencieux.
* **Réparation Chrome en un clic :** Bouton bleu de configuration automatique si le pont Chrome ne répond pas sur un nouveau PC.
* **Empaquetage des ressources Tauri :** Le pont local et l'extension Chrome sont intégrés directement au sein de l'installateur compilé (MSI/EXE).
* **Outils Rust & ripgrep Sidecar :** Intégration de Ripgrep en binaire natif sidecar et de nouveaux outils (`list_dir`, `delete_file`) pour accélérer la recherche et la gestion des fichiers par 10x.
* **Diagnostics Monaco en temps réel :** Remontée automatique des lints et erreurs de compilation de l'éditeur de code à l'IA en temps réel.
* **Logs ultra-compacts :** Nettoyage automatique du contexte de discussion pour éliminer le bruit et optimiser la consommation de jetons.
* **Laboratoire réseau MITM :** Outils de débogage et d'ingénierie inverse intégrés pour inspecter le trafic chiffré des outils IA.
* **Moteur de remplacement intelligent (Search/Replace) :** Système d'auto-correction à 8 couches (Unicode, indentations, etc.) garantissant que les modifications de l'IA s'insèrent correctement dans vos fichiers même en cas de légères erreurs d'espaces.

---

## 📜 Historique des Versions

## [Unreleased] - 2026-05-29 16:18:48

### Added
- **Rapport d'analyse et d'améliorations (`afaire.md`)** : Création d'un document récapitulatif listant les principaux axes d'amélioration du projet (performances de l'interface, organisation du moteur Rust, et sécurité des dépendances).


## [Unreleased] - 2026-05-29 16:16:37

### Added
- **Prise en charge de Claude Opus 4.8 (`crates/sinew-anthropic/src/client.rs`, `crates/sinew-anthropic/src/model_info.rs`, `src/lib/models.ts`)** : Intégration du nouveau modèle phare de la gamme d'intelligence artificielle d'Anthropic (Claude Opus 4.8) doté d'une fenêtre de contexte native de 1 million de jetons et des niveaux de réflexion configurables dans l'interface utilisateur.

### Changed
- **Mise à jour de la configuration de compilation (`Cargo.toml`, `Cargo.lock`)** : Synchronisation des versions des dépendances et du système de compilation avec la version de référence 0.1.26 de la branche parente.

## [Unreleased] - 2026-05-29 16:21:45

### Removed
- **Nettoyage complet du fichier `AGENTS.md` local** : Suppression définitive de toute trace ou consigne relative au système d'apprentissage automatique dans le fichier `AGENTS.md` de ce projet. L'injection et la gestion de la mémoire globale sont désormais entièrement intégrées de manière native au sein de l'application (côté Rust) pour tous les projets ouverts sur cette machine.

## [Unreleased] - 2026-05-29 16:15:36

### Added
- **Synchronisation OneDrive du Système d'Apprentissage (`src-tauri/src/lib.rs`)** : Ajout de la synchronisation bidirectionnelle automatique des fichiers d'apprentissage globaux (`errors_raw.json` et `instructions_consolidated.md`) via OneDrive. Lorsque l'option de synchronisation Multi-PC est active, ces fichiers sont fusionnés et sauvegardés sur OneDrive à la fermeture de l'application, et restaurés au démarrage sur vos autres ordinateurs.

## [Unreleased] - 2026-05-29 16:06:04

### Fixed
- **Correction de la visibilité de l'indicateur de contexte en mode IA (`src/styles.css`)** : Augmentation de la priorité d'empilement (z-index) de la zone de saisie (`.composer`) à 10 pour s'assurer que le volet volant affichant l'utilisation du contexte s'affiche par-dessus les messages du chat et la liste des tâches, évitant qu'il ne soit masqué ou flouté par les effets de transparence premium du mode IA.

## [Unreleased] - 2026-05-29 16:11:45

### Added
- **Intégration du système d'apprentissage global côté Rust (`src-tauri/src/turns.rs`)** : Modification du backend Rust de l'application pour charger et injecter automatiquement et de manière transparente le fichier d'instructions consolidées global (`%LOCALAPPDATA%\Sinew\instructions_consolidated.md`) dans le prompt système de tous les agents pour l'ensemble des projets ouverts sur cet ordinateur. Les agents bénéficient désormais de cette base d'apprentissage universelle sans dépendre d'un fichier local `AGENTS.md`.

## [Unreleased] - 2026-05-29 16:06:07

### Changed
- **Décentralisation complète des descriptions d'erreurs (`AGENTS.md`)** : Suppression définitive des descriptions locales d'erreurs dans `AGENTS.md` pour éviter toute duplication. Désormais, `AGENTS.md` ne contient que le pointeur d'instructions globales pour forcer la lecture de la base d'apprentissage centralisée dans `%LOCALAPPDATA%\Sinew\`.
- **Base de connaissances globale (`%LOCALAPPDATA%\Sinew\`)** : Migration de toutes les anciennes règles d'erreurs (Git, Windows, npm, MCP) directement dans le fichier d'apprentissage consolidé de la machine.

## [Unreleased] - 2026-05-29 16:04:26

### Changed
- **Globalisation du Système d'Apprentissage** : Déplacement de la mémoire d'apprentissage (`errors_raw.json` et `instructions_consolidated.md`) dans le dossier de configuration global de la machine (`%LOCALAPPDATA%\Sinew\`). Cela permet d'avoir un système d'apprentissage partagé et partagé sur tous les projets et espaces de travail ouverts sur cet ordinateur.
- **Règles d'agent (`AGENTS.md`)** : Mise à jour des règles d'instructions globales pour forcer la lecture et l'alimentation de la base d'apprentissage globale à chaque début de session.

## [Unreleased] - 2026-05-29 15:57:18

### Added
- **Intégration d'outils de diagnostic et d'émulation hybrides (`sinew-chrome-bridge/mcp_server.js` et `.sinew/skills/browser/SKILL.md`)** : Ajout de trois nouveaux outils structurés MCP (`emulate_experience`, `lighthouse_audit`, `analyze_memory_leaks`) basés sur l'API CDP brute de Google Chrome pour tester la compatibilité mobile (taille, touch events), évaluer les performances et diagnostics de qualité Lighthouse en local, et analyser la consommation de mémoire (JS heap et DOM nodes count).
- **Mise à jour de la compétence Browser (`.sinew/skills/browser/SKILL.md`)** : Ajout d'une section de documentation claire guidant l'IA à utiliser ces nouveaux pouvoirs de diagnostic en mode hybride tout en préservant le simulateur biologique humain furtif de Sinew Chrome.

## [Unreleased] - 2026-05-29 15:46:39

### Fixed
- **Correction et universalisation des lightboxes (`src/components/chat/ChatPane.tsx`, `src/components/chat/MermaidDiagram.tsx`, `src/styles.css`)** : Passage des zooms d'image et de diagrammes Mermaid sous forme de Portails React (`createPortal`) attachés à `document.body`. Cela corrige définitivement le problème de rognage/masquage causé par les empilements CSS (stacking contexts) des colonnes du chat et des options. De plus, augmentation de la priorité d'affichage (`z-index: 99999`) et intégration des styles sombres néon haut de gamme pour les schémas en mode IA afin qu'ils s'affichent magnifiquement dans tous les thèmes.

## [Unreleased] - 2026-05-29 15:52:23

### Added
- **Étude d'impact sur la sécurité, l'anti-détection et la discrétion (`ETUDE_IMPACT_SECURITE.md`)** : Rapport d'analyse complet évaluant les risques de détection des fonctionnalités de DevTools for Agents 1.0 (émulation, auto-connect, audits) et concevant une synergie d'interception avec notre simulateur biologique humain.
- **Rapport d'analyse et d'intégration DevTools (`ANALYSE_DEVTOOLS_MCP.md`)** : Création d'une analyse technique poussée évaluant la faisabilité et l'impact de l'intégration des fonctionnalités de Chrome DevTools for Agents 1.0 (Lighthouse, émulation, fuites mémoire, auto-connect) tout en conservant le simulateur biologique humain furtif unique de Sinew Chrome.
- **Rapport d'évaluation de la pertinence des outils de diagnostic Google (`ANALYSE_DIAGNOSTIC_GOOGLE.md`)** : Analyse de pertinence des outils de diagnostic avancés de Google (Lighthouse, émulation, fuites mémoire, auto-connect) du point de vue de l'audit et du contrôle qualité, avec métaphores simples et plan d'action d'adaptation pour notre simulateur biologique.

### Changed
- **Mise à jour de la carte du code (`AGENTS.md`)** : Enregistrement des rapports d'analyse de l'équipe (sécurité, architecture, intégration, diagnostic) dans la carte des fichiers du projet.

## [Unreleased] - 2026-05-29 15:48:14

### Added
- **Option de Recherche Sémantique Vectorielle (`src/components/SettingsPane.tsx` & `src-tauri/src/workspace.rs` & `src-tauri/src/lib.rs` & `src/lib/ipc.ts`)** : Ajout d'une nouvelle option SOTA « Recherche Sémantique Vectorielle (BETA) » dans les paramètres (Apparence & Interface). Liaison avec un nouveau handler Tauri Rust `set_semantic_embeddings_enabled` pour activer/désactiver dynamiquement la variable d'environnement `SINEW_INDEX_EMBEDDINGS` à la volée.

## [Unreleased] - 2026-05-29 15:47:36

### Added
- **Script d'installation automatisé (`package.json`)** : Ajout d'un script `postinstall` pour installer automatiquement les dépendances des extensions locales (`sinew-chrome-bridge` et `scripts/agent-bridge`) lors du lancement initial de `npm install` par n'importe quel développeur ou serveur de compilation.

## [Unreleased] - 2026-05-29 15:43:42

### Changed
- **Exclusion de dossiers (`.gitignore`)** : Ajout du répertoire `/build/` dans les règles d'exclusion de Git pour éviter de traquer les binaires et installateurs compilés localement.

### Removed
- **Nettoyage de l'historique Git** : Retrait du suivi de version Git pour les gros fichiers d'installation d'anciennes versions (`build/Sinew.exe`, `build/Sinew_0.1.25_x64-setup.exe` et `build/Sinew_0.1.25_x64_en-US.msi`) ainsi que pour le dossier de dépendances obsolètes (`sinew-chrome-bridge/node_modules/`), tout en conservant ces fichiers intacts sur les disques durs locaux.

## [Unreleased] - 2026-05-29 15:40:51

### Added
- **Rapport d'analyse et d'idées SOTA (`RAPPORT_OUTILS.md`)** : Création d'un rapport consolidé présentant la vérification des outils et détaillant les 3 pistes d'améliorations SOTA validées et sécurisées.

## [Unreleased] - 2026-05-29 15:39:58

### Changed
- **Intégration du Changelog (`CHANGELOG.md`)** : Déplacement et intégration de la documentation des fonctionnalités majeures du fork (anciennement `README-FORK.md`) directement en en-tête du journal pour une meilleure visibilité.

### Removed
- **Nettoyage général des vestiges et rapports** : Suppression définitive de 12 fichiers d'audits, de rapports et de scripts de test obsolètes (`AUDIT_PERFORMANCE_SOTA.md`, `AUDIT_RUST.md`, `AUDIT_SECURITE.md`, `RAPPORT_OUTILS.md`, `RAPPORT_VERIFICATION.md`, `RAPPORT_VERIFICATION_FINAL.md`, `README-FORK.md`, `scripts/AGENT-SPIKE-DESIGN.md`, `scripts/CAPTURE-MITM.md`, `scripts/probe_agent_run.py`, `scripts/probe_idempotent_crypto.py`, `scripts/verify_all.py` et le cache `__pycache__`) pour nettoyer totalement l'arborescence du projet.

## [Unreleased] - 2026-05-29 15:33:18

### Added
- **Rapport d'analyse des outils (`RAPPORT_OUTILS.md`)** : Création d'un rapport complet sur l'état des outils système, de l'indexeur et de l'intégration du navigateur Sinew pour répondre aux interrogations de l'utilisateur.

## [Unreleased] - 2026-05-29 15:31:31

### Fixed
- **Settings Pane (`src/components/SettingsPane.tsx`)** : Correction de la persistence de l'option « Exposer tous les outils au démarrage » (`autoLoad`) pour les serveurs MCP. L'option était omise lors de la sérialisation des paramètres en JSON (`settingsToJson`), ce qui entraînait sa réinitialisation à chaque rechargement ou modification de la configuration. Ajout de la sérialisation de `autoLoad` dans le JSON exporté.

## [Unreleased] - 2026-05-29 15:28:08

### Fixed
- **Styles CSS (`src/styles.css`)** : Correction du problème d'affichage du menu déroulant (popover) de sélection des modèles en mode/thème IA. Remplacement de `overflow: hidden` par `overflow: visible !important` pour la boîte de composition `.composer__box` sous le sélecteur `html[data-theme="ai"]`, évitant ainsi le masquage ou le rognage des options du menu au-delà des bordures du conteneur.
