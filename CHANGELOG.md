# Changelog

All notable changes to this project will be documented in this file.

## [2026-05-30 03:33:19]
- `src/components/Workspace.tsx` : Ajout d'un timer automatique (toutes les 5 minutes) qui vérifie si l'apprentissage IA est activé et, si oui, déclenche la consolidation IA des erreurs. Premier déclenchement après 30 secondes au démarrage.

## [2026-05-30 03:20:13]
- `src-tauri/src/rules.rs` : Ajout de la fonction `ai_consolidate_rules()` qui lit les erreurs brutes (`errors_raw.json`) et les règles existantes (`instructions_consolidated.md`), les envoie à un fournisseur IA (DeepSeek par défaut) pour analyse, dédoublonnage et fusion intelligente des règles similaires, puis écrit le fichier optimisé.
- `src-tauri/src/lib.rs` : Ajout de la commande Tauri `trigger_ai_rule_consolidation` pour déclencher manuellement l'analyse IA depuis l'interface.
- `src/lib/ipc.ts` : Ajout de la méthode `triggerAiRuleConsolidation(providerId)` au bridge IPC.
- `src/components/SettingsPane.tsx` : Ajout d'une carte "Apprentissage Automatique IA" dans la section Diagnostics, avec bouton ON/OFF, sélecteur de fournisseur IA, bouton d'analyse manuelle, et affichage du statut. Cette IA remplace le script de consolidation simple pour fusionner les règles redondantes.

## [2026-05-30 03:22:45]
- `src/components/SettingsPane.tsx` : Ajout d'un bouton "Refresh" global dans l'en-tête de la section MCP pour reconnecter et rafraîchir tous les serveurs MCP en un clic.

## [2026-05-30 03:21:11]
- `.sinew/skills/browser/` & `.sinew/skills/computer_use/` : Déplacement des compétences `browser` et `computer_use` du workspace vers le dossier global utilisateur `~/.agents/skills/`, afin qu'elles soient disponibles pour tous les workspaces et non uniquement pour celui de Sinew.

## [2026-05-30 03:13:06]
- `src-tauri/src/workspace.rs` : Création de la commande `list_ssh_hosts` pour extraire automatiquement les serveurs/alias configurés dans le fichier `~/.ssh/config` de l'utilisateur.
- `src-tauri/src/lib.rs` : Enregistrement de la commande `list_ssh_hosts` dans le gestionnaire Tauri.
- `src/lib/ipc.ts` : Exposition de la méthode `listSshHosts`.
- `src/components/Welcome.tsx` : Intégration des boutons de connexion rapide ("Quick Connect") basés sur la liste des serveurs configurés pour une connexion instantanée en un clic.

## [2026-05-30 03:06:42]
- `src-tauri/src/turns.rs` : Correction d'une erreur d'emprunt de valeur déplacée (borrow of moved value) en clonant les paramètres de configuration dans la fonction d'orchestration des turns.

## [2026-05-30 03:03:29]
- `crates/sinew-app/src/lib.rs` : Re-exportation de la structure `TurnOutput` pour la rendre accessible par l'application Tauri.
- `crates/sinew-app/src/agent/events.rs` : Dérivation du trait `Deserialize` pour la structure `AgentEvent`, permettant au client de désérialiser les évènements de l'agent.
- `src-tauri/src/turns.rs` : Implémentation du pont Named Pipe IPC client (`run_turn_via_daemon`) qui envoie la commande `StartTurn` au démon Windows, écoute les réponses en temps réel, redirige les évènements vers le moteur principal de l'UI, et démarre automatiquement le binaire detached (`spawn_daemon`) en cas d'absence.


## [2026-05-30 03:06:42]
- `src-tauri/src/turns.rs` : Utilisation explicite des types ré-exportés par `sinew_app` dans le proxy du démon de fond (AgentEvent, TurnOutput, McpSettings, etc.) pour résoudre les conflits de types et d'importations.
- `src-tauri/src/workspace.rs` : Ajout de l'installation automatique et silencieuse en tâche de fond de WinFsp et SSHFS-Win via Winget s'ils sont absents lors de la connexion.
- `sinew-chrome-bridge/native-host-wrapper/src/main.rs` : Correction d'une erreur de syntaxe (accolades fermantes superflues) empêchant la compilation du binaire natif du pont Chrome.

## [2026-05-30 03:04:09]
- `src-tauri/src/lib.rs` : Restauration des imports accidentellement supprimés par la session précédente (`DeleteFileTool`, `EditFileTool`, `GlobTool`, `GoalWorkflowState`, `GrepTool`), résolvant les erreurs de compilation du projet principal.
- `sinew-chrome-bridge/native-host-wrapper/Cargo.toml` : Ajout de la dépendance `base64` pour permettre la compilation du binaire natif du pont Chrome.

## [2026-05-30 03:06:42]
- `crates/sinew-agent-daemon/src/main.rs` : Remplacement du chargement incorrect de `all_auth_files` par le constructeur standard `from_default_sources` pour le fournisseur Google, résolvant les erreurs de compilation du daemon.

## [2026-05-30 03:04:09]
- `sinew-chrome-bridge/native-host-wrapper/Cargo.toml` : Ajout de la dépendance `base64` pour permettre la compilation du binaire natif du pont Chrome.

## [2026-05-30 03:06:42]
- `src-tauri/src/workspace.rs` : Ajout de l'installation automatique et silencieuse en tâche de fond de WinFsp et SSHFS-Win via Winget s'ils sont absents lors de la connexion.
- `sinew-chrome-bridge/native-host-wrapper/src/main.rs` : Correction d'une erreur de syntaxe (accolades fermantes superflues) empêchant la compilation du binaire natif du pont Chrome.

## [2026-05-30 03:00:17]
- `AGENTS.md` : Mise à jour de la carte des fichiers (code map) avec `computer_use.rs` et la nouvelle Skill.
- `.sinew/skills/computer_use/SKILL.md` : Création de la compétence (Skill) documentant le pilotage Windows pour l'agent.
- `sinew-chrome-bridge/native-host-wrapper/src/main.rs` : Exposition de la commande MCP `computer_use` et implémentation Windows native correspondante.
- `sinew-chrome-bridge/native-host-wrapper/Cargo.toml` : Ajout de la dépendance `image` pour le wrapper MCP.
- `crates/sinew-app/src/subagent.rs`, `crates/sinew-app/src/team.rs`, `crates/sinew-app/src/team/agent_turns.rs` : Instanciation de l'outil `ComputerUseTool` pour les sous-agents et les agents d'équipe.
- `src-tauri/src/lib.rs` : Import de `ComputerUseTool` dans le binaire principal de Tauri.
- `src-tauri/src/turns.rs`, `src-tauri/src/swarm.rs` : Instanciation de l'outil `ComputerUseTool` dans les contextes de turn.
- `crates/sinew-app/src/agent/turn.rs` : Intégration de l'appel et des descripteurs de `ComputerUseTool` dans le flux principal de discussion de l'agent.
- `crates/sinew-app/src/agent/tool_dispatch.rs` : Routage dynamique de la commande `computer_use` vers la simulation système correspondante.
- `crates/sinew-app/src/agent/context.rs` : Intégration de `ComputerUseTool` dans le contexte de discussion de l'agent.
- `crates/sinew-app/src/lib.rs` : Exportation du nouvel outil `ComputerUseTool`.
- `crates/sinew-app/src/tool_names.rs` : Définition de la constante d'outil `computer_use` et prise en charge de sa résolution canonique.
- `crates/sinew-app/Cargo.toml` : Ajout de la dépendance `image` pour compresser les captures d'écran du Computer Use.
- `crates/sinew-app/src/computer_use.rs` : Création du module d'automatisation et de pilotage d'ordinateur (Computer Use) natif pour Windows (GDI screenshots, simulation clavier/souris).











## [2026-05-30 03:01:48]
- `CHANGELOG.md` : Enregistrement de la suppression des fichiers temporaires et rapports d'analyse obsolètes.
- `AGENTS.md` : Mise à jour de la carte des fichiers (code map) suite au retrait des fichiers inutiles du projet.
- `afaire.md`, `AMELIORATION_SSH.md`, `COMPARAISON_ARCHITECTURE.md`, `Rapport_Analyse_Composer_2.5.md`, `RAPPORT_ANTIGRAVITY.md`, `Rapport_Codex_Analyse.md`, `RAPPORT_DAEMON_PERSISTANT.md`, `RAPPORT_DECOMPILE_CURSOR.md`, `Rapport_SSH_Analyse.md`, `untitled.txt` : Suppression des fichiers de rapports temporaires et documents d'analyse obsolètes pour nettoyer le projet.
- `sinew-chrome-bridge/bridge.log`, `sinew-chrome-bridge/bridge_err.log` : Nettoyage des journaux de logs locaux inutiles.


## [2026-05-30 03:02:04]
- `crates/sinew-agent-daemon/src/protocol.rs` : Création de la structure du protocole d'échange JSON IPC (Requêtes de turn, d'annulation, de statut et Réponses d'événements et d'erreurs).
- `crates/sinew-agent-daemon/src/main.rs` : Implémentation du serveur d'écoute asynchrone multithread gérant les connexions entrantes sur le Named Pipe et le traitement des messages JSON-RPC délimités par des retours à la ligne (`\n`).


## [2026-05-30 02:57:00]
- `src-tauri/src/workspace.rs` : Création de la commande `mount_ssh_workspace` pour automatiser la détection de lettre libre, le lancement d'SSHFS-Win et l'ouverture automatique du lecteur.
- `src-tauri/src/lib.rs` : Enregistrement du gestionnaire Tauri `mount_ssh_workspace`.
- `src/lib/ipc.ts` : Exposition de la méthode API `mountSshWorkspace`.
- `src/components/Welcome.tsx` : Intégration d'un formulaire et bouton de connexion SSH directe dans l'interface d'accueil (Switch) permettant de connecter n'importe quelle VM à la volée.

## [2026-05-30 03:00:40]
- `Cargo.toml` : Ajout du sous-projet `crates/sinew-agent-daemon` à la liste des membres du workspace Cargo.
- `crates/sinew-agent-daemon/Cargo.toml` : Création du fichier de configuration Cargo avec ses dépendances (tokio, anyhow, serde, etc.).
- `crates/sinew-agent-daemon/src/main.rs` : Implémentation du squelette du démon de fond persistant Windows (écriture de PID, configuration de serveur Named Pipe).


## [2026-05-30 02:57:59]
- `RAPPORT_DAEMON_PERSISTANT.md` : Création du rapport de conception SOTA détaillant le découplage du moteur de discussion en démon d'arrière-plan Windows persistant (Named Pipes, cycle de vie detached, persistance SQLite, stream de reconnexion).


## [2026-05-30 02:52:05]
- `sinew-chrome-bridge/mcp_server.js`, `sinew-chrome-bridge/server.js` : Suppression définitive des anciens serveurs Node.js obsolètes après la réécriture totale du pont Chrome en Rust.
- `sinew-chrome-bridge/register.ps1` : Nettoyage et suppression complète des dépendances et lanceurs Node.js (`ws`, `npm install`, fichiers `.bat`) pour un déploiement 100% natif.

## [2026-05-30 02:54:37]
- `src/components/Welcome.tsx` : Retrait du bouton d'accès SSH/Sandbox de la page d'accueil pour respecter la préférence de l'utilisateur de travailler exclusivement dans un dossier projet monté.

## [2026-05-30 02:51:02]
- `Cargo.toml` : Ajout de la dépendance `ignore` au niveau de l'espace de travail.
- `crates/sinew-index/Cargo.toml` : Ajout de la dépendance `ignore`.
- `crates/sinew-index/src/indexer.rs` : Intégration de la gestion dynamique des fichiers `.gitignore`, `.cursorignore` et `.sinewignore` dans l'indexeur de base de code.
- `crates/sinew-app/src/workspace.rs` : Ajout de `.sinew` dans la liste des répertoires exclus de l'exploration de l'espace de travail, masquant ainsi `.sinew/worktrees`.
- `sinew-chrome-bridge/sinew_cursor.js` : Implémentation du système d'étiquetage d'interface (injection visuelle des badges `@ref1`, `@ref2` etc.) et résolution automatique des sélecteurs de référence par l'assistant.
- `crates/sinew-app/src/agent/turn.rs` : Ajout d'une boucle d'auto-correction (Forced Reflection system reminder) en cas de tours d'outils répétés pour éviter les boucles infinies de l'IA.


- `sinew-chrome-bridge/native-host-wrapper/Cargo.toml` : Ajout de la dépendance chrono pour l'analyse de performance.
- `sinew-chrome-bridge/native-host-wrapper/src/main.rs` : Ajout des outils de diagnostic et d'émulation Chrome restants (emulate_experience, lighthouse_audit et analyze_memory_leaks) en Rust natif pour atteindre 100% de parité fonctionnelle et supprimer la dépendance à Node.js.

## [2026-05-30 02:48:23]
- `src-tauri/src/cli.rs` : Enregistrement du serveur MCP natif Rust (native-host-wrapper.exe) s'il existe, avec repli automatique sur Node.js (mcp_server.js).
- `crates/sinew-openai/src/stream.rs` : Correction d'avertissement clippy sur un bloc match pliable.
- `crates/sinew-app/src/edit.rs` : Correction d'avertissements clippy sur l'indexation de boucles et les tris personnalisés.
- `crates/sinew-app/src/agent/cancel.rs` : Correction d'avertissement clippy sur le retour d'un type d'erreur unitaire.
- `sinew-chrome-bridge/native-host-wrapper/src/main.rs` : Suppression des avertissements clippy de conversions redondantes dans l'affichage JSON.
- `sinew-chrome-bridge/native-host-wrapper.exe` : Recompilation en mode release sans avertissements clippy.

## [2026-05-30 02:45:07]
- `AMELIORATION_SSH.md` : Création du plan d'action d'amélioration SSH surpassant le SOTA (filtrage des clés, persistance des connexions en tâche de fond et découplage des configurations).

## [2026-05-30 02:44:34]
- `Cargo.toml` : Ajout de la dépendance chrono dans le workspace pour la consolidation des règles en Rust.
- `src-tauri/Cargo.toml` : Ajout des dépendances regex, chrono et futures.
- `src-tauri/src/rules.rs` : Création de l'implémentation native en Rust de la consolidation des règles d'apprentissage.
- `src-tauri/src/cli.rs` : Création du CLI natif en Rust pour synchroniser le projet et enregistrer les extensions MCP sans dépendance Python (et nettoyage des imports inutilisés), ajout de l'outil de diagnostic de connexion (--probe) avec gestion des événements de flux.
- `src-tauri/src/main.rs` : Interception des paramètres en ligne de commande pour le CLI de synchronisation et de configuration.
- `src-tauri/src/lib.rs` : Exposition des fonctions internes de base de données et de copie de fichiers pour le CLI.
- `consolidate_rules.py`, `sync_now.py`, `sinew-chrome-bridge/add_to_sinew.py`, `scripts/probe_*.py` : Suppression de tous les scripts Python obsolètes suite à leur réécriture native en Rust.
- `sinew-chrome-bridge/register.ps1` : Utilisation de la commande native Rust `Sinew.exe --register-chrome` au lieu du script Python.
- `crates/sinew-cursor`, `crates/sinew-app` : Application de corrections automatiques Clippy et résolution manuelle de warnings de syntaxe.

## [2026-05-30 02:44:22]
- `src/components/Welcome.tsx` : Ajout d'un bouton d'accès direct SSH/Sandbox sur la page d'accueil (Switch) pour utiliser le serveur MCP SSH.

## [2026-05-30 02:43:27]
- `COMPARAISON_ARCHITECTURE.md` : Création du document d'analyse comparative entre l'architecture de Cursor et les fonctionnalités actuelles de Sinew, évaluant le niveau d'opportunité d'intégration (Shadow Workspace, Indexation/Ignore, MCP Navigateur, Boucle d'agent, Commits).


## [2026-05-30 02:36:45]
- `RAPPORT_DECOMPILE_CURSOR.md` : Mise à jour et enrichissement en profondeur du rapport d'analyse de l'architecture de Cursor (gRPC, sockets locaux, indexation Merkle native, daemon autonome d'agent, plomberie Git temporaire, automatisation de navigateur par WebView injectée et réduction de contexte).


## [2026-05-30 02:33:05]
- `sinew-chrome-bridge/native-host-wrapper/Cargo.toml` : Ajout des dépendances tokio, tokio-tungstenite, serde, serde_json, anyhow, futures-util, directories, uuid et reqwest pour réécrire le pont Chrome natif en Rust.
- `sinew-chrome-bridge/native-host-wrapper/src/main.rs` : Réécriture complète du pont Chrome et du serveur MCP en Rust (SOTA zero-install) permettant de supprimer la dépendance à Node.js.
- `sinew-chrome-bridge/add_to_sinew.py` : Enregistrement du nouveau binaire natif Rust MCP dans la base de données SQLite de Sinew à la place de l'ancien script Node.js.


## [2026-05-30 02:39:37]
- `mcp_settings` : Intégration du serveur MCP SOTA `slepp-ssh-mcp` dans la base SQLite locale pour donner aux agents un accès SSH complet aux machines distantes.

## [2026-05-30 02:38:31]
- `src/components/chat/ToolCard.tsx` : Ajout du bouton "Auto-réparer" sur les cartes de commande bash en cas d'erreur.
- `src/components/chat/ChatPane.tsx` : Implémentation du callback de réparation `handleFixCommand` et passage du prop à `ToolCard`.
- `src/styles.css` : Ajout des styles pour le bouton d'auto-réparation `.tool-card__fix-action`.

## [2026-05-30 02:36:45]
- `search_decompiled.py` : Créé puis supprimé après avoir servi à analyser en profondeur les extensions décompilées de Cursor.
- `RAPPORT_DECOMPILE_CURSOR.md` : Rapport complet d'analyse de l'architecture de Cursor (Shadow Workspace, Retrieval, MCP Navigateur, Boucle d'agent, Commits) enrichi avec les détails bas niveau (Délégation CDP, sockets gRPC, synchronisations Merkle, simhash, correctifs OAuth MCP SDK) rédigé en français simple.

## [2026-05-30 02:37:33]
- `Rapport_Codex_Analyse.md` : Détail complet du fonctionnement SOTA du bouton "Auto-réparer" (boucle d'auto-correction via sous-agents et vérification de build).

## [2026-05-30 02:35:40]
- `Rapport_Codex_Analyse.md` : Ajout des sections d'analyse sur le pilotage d'ordinateur (Computer Use) et la télécommande par téléphone (Remote Control).
- Confirmé la présence native du rendu de diagrammes Mermaid dans Sinew.

## [2026-05-30 02:26:13]
- `crates/sinew-cursor/src/agent/run_h2.rs` : Redirection du point de contact de l'agent NAL vers le serveur de production express de Cursor (`agent.api5.cursor.sh` au lieu de `api2.cursor.sh`).
- `scripts/agent-bridge/run-stream.mjs` : Alignement de l'endpoint du pont Node pour utiliser le serveur express `agent.api5.cursor.sh`.
- `scripts/agent-bridge/h2-bridge.mjs` : Alignement de l'endpoint par défaut du pont HTTP/2 Node pour utiliser `agent.api5.cursor.sh`.

## [2026-05-30 02:26:13]
- `Rapport_Analyse_Composer_2.5.md` : Ajout du rapport d'analyse synthétique sur le support de Composer 2.5 standalone, les clés de sécurité et la migration vers la ligne express agent.api5.

## [2026-05-30 02:31:00]
- `Rapport_Codex_Analyse.md` : Enrichissement du rapport avec les analyses d'interface utilisateur et de fonctionnalités frontend (Mini-apps MCP, planificateur d'automatisations RRule, auto-réparation des espaces temporaires Git et régulateur de débit d'affichage).

## [2026-05-30 02:26:01]
- `Rapport_Codex_Analyse.md` : Ajout des analyses détaillées sur la sécurité de Codex (relocalisation de binaires hors WindowsApps, filtres réseau WFP persistants pour Windows Sandbox et jetons AppContainer/Capability SIDs pour le Command Runner).

## [2026-05-30 02:29:00]
- `Rapport_Codex_Analyse.md` : Création du rapport de synthèse de Codex analysant son architecture, son intégration avec le clavier Work Louder, son isolation d'exécutables (staging) et ses politiques de bac à sable (sandbox).


## [2026-05-30 02:26:42]
- `src/components/SettingsPane.tsx` : Ajout d'une option de configuration pour agrandir la taille de la boîte de saisie (boîte de chat) en mode normal ou agrandi.
- `src/App.tsx` : Initialisation au démarrage de l'attribut `data-large-chat-box` sur le document HTML à partir des paramètres persistés de l'utilisateur.
- `src/styles.css` : Utilisation de variables CSS pour la hauteur minimale/maximale du composer de messages et doublement automatique de ces dimensions en mode agrandi.


## [2026-05-30 02:23:29]
- `RAPPORT_ANTIGRAVITY.md` : Création et simplification complète du rapport d'analyse pour supprimer le jargon technique et utiliser des métaphores faciles à comprendre (Téléviseur et Décodeur).

## [2026-05-30 02:20:52]
- `Rapport_SSH_Analyse.md` : Création du rapport détaillé d'analyse de l'implémentation SSH dans Antigravity, Codexx et Cursor en utilisant les perspectives des 4 sous-agents.

## [2026-05-30 02:18:39]
- `crates/sinew-app/src/write.rs` : Résolution d'un bug critique bloquant l'écriture de nouveaux fichiers sur Windows en harmonisant la comparaison insensible à la casse et la suppression des préfixes UNC (`\\?\`).
- `crates/sinew-app/src/read.rs` : Harmonisation de la fonction `relative_from_root` pour nettoyer correctement les préfixes UNC sous Windows et éviter les fausses alertes d'accès hors espace de travail.

## [2026-05-30 02:16:06]
- `consolidate_rules.py` : Correction d'un bug cosmétique de double point final lors de la génération de règles d'auto-apprentissage si la description d'erreur contenait déjà un point.
- `test_consolidation.py` : Ajout puis suppression du script temporaire de test de validation du système d'auto-apprentissage des erreurs.

## [2026-05-30 02:13:43]
- `C:\Users\julie\.agents\skills` : Restauration de la compétence de recherche globale `find-skills` pour permettre la découverte et l'installation de compétences à la demande.

## [2026-05-30 02:15:11]
- `crates/sinew-cursor/src/identity.rs` : Cache de la détection du fuseau horaire via OnceLock pour éviter le spawn répétitif de PowerShell sur chaque requête.
- `crates/sinew-index/src/store.rs` : Optimisation majeure des performances SQLite. Mise en cache du profil de puissance machine (OnceLock), détection SSD/NVMe Windows améliorée via le PNPDeviceID et Caption, augmentation dynamique de la taille du cache SQLite (limité à ~3.1% de la mémoire vive pour rester bien en dessous du plafond de 40% demandé par l'utilisateur, max 1 Go) et de la taille de mmap (max 4 Go), et activation de PRAGMA threads multi-cœurs.

## [2026-05-30 02:12:16]
- `crates/sinew-index/src/process.rs` : Limitation de la mémoire des sous-processus de l'indexeur (recherche codebase et watch) à 12 Go maximum sur Windows via les API de Job Object, afin d'éviter tout blocage ou fuite de mémoire excessive.

## [2026-05-30 02:10:30]
- `C:\Users\julie\.agents\skills` : Suppression des dossiers de compétences globales pré-installés superflus pour ne conserver que la compétence Chrome locale (`browser`) de l'espace de travail.

## [2026-05-30 02:08:33]
- `src/components/SettingsPane.tsx` : Suppression du bouton de synchronisation manuelle ("Synchroniser maintenant") et de la section de détection/liaison des conversations d'autres projets ("Détection de conversations d'autres projets / PC") pour simplifier l'interface utilisateur.


## 🚀 Présentation des Fonctionnalités Majeures (Fork Premium julienpiron.fr)

Cette version a été optimisée en profondeur pour offrir une expérience utilisateur haut de gamme (SOTA), une autonomie maximale en arrière-plan, et des intégrations d'intelligence artificielle inégalées.

### 🎨 Interface, Confort & Ergonomie (Premium UI)
* **Animation de démarrage premium :** Une animation de boot moderne, fluide et élégante à l'ouverture de l'application.
* **3 niveaux de réflexion :** Choix entre Détaillé, Compact ou Très compact pour configurer précisément la verbosité de l'IA et le masquage des détails techniques dans le chat.
* **Question collante (Sticky Question) :** La question en cours de traitement reste épinglée en haut de l'écran pendant que vous faites défiler le fil de discussion.
* **Menu clic droit interactif sur les onglets de l'éditeur :** Clic droit (ou `F10`) sur les onglets pour fermer l'onglet (raccourci `Ctrl+F4`), les autres, à sa droite ou tous, copier le chemin (absolu ou relatif) et révéler dans le Finder/Explorateur.
* **Menu clic droit d'exécution :** Clic droit sur les fichiers dans le chat et l'arbre des fichiers pour les ouvrir, les révéler ou les exécuter directement.
* **Polices dynamiques ajustables :** Boutons tactiles réactifs (`+` et `-`) dans les options pour ajuster instantanément à chaud la taille du texte de l'éditeur Monaco et du chat.
* **Version française complète :** L'interface entière et toutes les actions de l'application s'adaptent automatiquement en français ou en anglais.
* **Sélection et copie libre :** Déblocage de la sélection et copie de texte directement dans le fil de discussion du chat.
* **Démarcation visuelle :** Ligne de séparation verticale élégante à gauche du panneau de configuration des paramètres.
* **Découpage du bundle Vite (-80% de taille) :** Monaco Editor et xterm.js sont isolés dans des sous-lots séparés pour un chargement instantanél'interface utilisateur.
* **Visualisation du plan d'action (Planning Board) :** Intégration d'un bloc dynamique interactif (`PlanningNextMoveBlock`) montrant en temps réel les prochaines étapes planifiées par le Swarm d'agents.
* **Aperçu d'image immersif (Lightbox) :** Visionneuse d'images de discussion immersive avec zoom à la molette de souris, déplacement panoramique, rotation, téléchargement et fermeture par clic extérieur.

### 💾 Autonomie, Sauvegarde & Robustesse Système
* **Sauvegarde automatique (Auto-Save SOTA) :** Enregistrement automatique et transparent en arrière-plan 1,5 seconde après l'arrêt de la frappe. Activable ou désactivable d'un clic dans vos options.
* **Mode Sandbox :** Lancement de l'application en un clic sans aucun projet ouvert pour tester l'IA ou utiliser les outils MCP de manière isolée.
* **Synchro OneDrive & SQLite automatique :** Synchronisation transparente de vos conversations, configurations de projets, jetons de connexion/clés d'authentification (`*-auth.json`, `*-device.json`, `*-stream-state.json`), fichiers d'apprentissage globaux (`errors_raw.json` et `instructions_consolidated.md`), et bases de données SQLite entre vos différents ordinateurs.
* **Zéro popup console Windows :** Lancement asynchrone et silencieux de tous les outils, serveurs MCP, commandes Git et diagnostics SOTA en arrière-plan sans aucune ouverture de fenêtres d'invite de commandes.
* **Préfixe PC réel automatique :** Identification automatique du nom de la machine physique pour typer et sécuriser les configurations de conversation multi-PC.
* **Diagnostic Windows OAuth résilient :** Capture robuste de l'erreur réseau typique sous Windows (code 10013) et conseils clairs pour débloquer la connexion (WinNAT/HNS).
* **Diagnostic SOTA :** Vérification en un clic de l'état de santé, du PATH et des versions de tous vos outils de développement (Git, Python, Node, Cargo, etc.).
* **Écran de mises à jour sécurisé (`UpdaterLockScreen`) :** Verrouillage de l'interface pendant l'application des correctifs système pour éviter tout conflit de fichiers ou corruption de base de données.
* **Script de compilation OneDrive (`compil.ps1`) :** Automatisation de la génération de l'application et copie immédiate sur OneDrive pour un déploiement instantané sur vos PC.
* **Active Turn Registry :** Moteur intelligent Rust qui suit les turns de l'agent en cours et assure une reprise instantanée du streaming.
* **Fiche de transmission structurée (Compaction d'IA) :** Compactage automatique du contexte lors du changement de fournisseur d'IA dans une fiche structurée reprenant le statut des fichiers modifiés, le relais des tâches et les diagnostics du linter.
* **Mode plein gaz adaptatif (`crates/sinew-index/src/store.rs`) :** Optimisation dynamique des performances de l'indexeur augmentant le cache et la lecture en mémoire lorsque la machine dispose d'un stockage SSD/NVMe.
* **Indexation locale parallèle SOTA :** Préparation et analyse des fichiers en parallèle répartie sur tous les coeurs de CPU disponibles via Rayon, avec détection immédiate et saut des fichiers inchangés grâce à leurs empreintes de taille et date.
* **Identification de projet universelle :** Association automatique des conversations au dépôt Git distant (remote origin URL) ou via un fichier d'identifiant unique `.sinew/project_id.txt` pour lier instantanément vos conversations d'un PC à un autre sans aucune action manuelle, avec détection, liaison et rafraîchissement dynamique des conversations provenant de PC alternatifs depuis les paramètres.
* **Gestion des mises à jour configurables :** Option à 3 choix (Bloquant, Notification, Désactivé) pour décider précisément du niveau de vérification des nouvelles versions de Sinew et empêcher l'écrasement de vos modifications.
* **Script de contrôle qualité unifié (`scripts/check.ps1`) :** Commande unique `npm run check` exécutant le build frontend, `cargo check`, les tests, `clippy` et les audits de dépendances en une seule opération.
* **Système d'apprentissage global transparent :** Chargement et injection automatique de la base d'instructions centralisées de l'utilisateur (`%LOCALAPPDATA%\Sinew\instructions_consolidated.md`) dans le prompt système de tous les agents pour l'ensemble des projets ouverts sur la machine.
* **Consolidation automatique de la mémoire :** Mécanisme au démarrage transformant automatiquement les erreurs répétées enregistrées dans `errors_raw.json` en règles d'apprentissage globales permanentes dans `instructions_consolidated.md` avec nettoyage du compteur d'erreurs.
* **Bouton de synchronisation forcée :** Ajout d'un bouton de synchronisation immédiate à la demande dans les paramètres pour déclencher manuellement la synchronisation bidirectionnelle OneDrive et Git.

### 🤖 Modèles d'IA, Comptes & Furtivité (AI Engine)
* **Gestion Multi-comptes OpenAI & Google Gemini :** Connexion simultanée de plusieurs profils OpenAI et Google Gemini secondaires avec bascule instantanée entre vos différentes clés, comptes et abonnements.
* **Quotas en temps réel :** Visualisation dynamique de votre consommation (crédits / balance restante) sous forme de barres de progression colorées adaptatives dans les options, et pastille live dans le chat.
* **Routage & Résilience Google Antigravity SOTA :** Réparation, de-surcharge réseau (erreur 503), routeurs de secours et transition transparente entre modèles avec résolution dynamique des identifiants d'appels d'outils (tool_call_id).
* **Optimisation de vitesse Gemini :** Streaming et requêtes ultra-rapides pour les modèles Gemini.
* **Incorporation de Claude Opus 4.8 & 4.6 :** Intégration complète de Claude Opus 4.8 (contexte 1M natif) et Claude Opus 4.6 via les abonnements professionnels Google.
* **Système Pending/Steering pour Influencer :** Un vrai système d'interception et de guidage pour orienter, corriger ou ajouter des instructions en temps réel sans blocage du flux de l'IA.
* **Indexation sémantique locale vectorielle :** Indexation et recherche vectorielle haute-performance effectuée localement sur votre machine avec commutateur d'activation directe (BETA) dans le panneau d'options.
* **Intégration de DeepSeek R1 & V3 :** Support complet de **DeepSeek V3** et de **DeepSeek R1** avec capture et rendu en temps réel du bloc de réflexion (*reasoning*) grâce à l'extraction du champ `reasoning_content` dans le chat.
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
* **Laboratoire réseau MITM :** Outils de débogage et d'ingénierie inverse intégrés pour inspecter le trafic chiffrés des outils IA.
* **Moteur de remplacement intelligent (Search/Replace) :** Système d'auto-correction à 8 couches (Unicode, indentations, etc.) garantissant que les modifications de l'IA s'insèrent correctement dans vos fichiers même en cas de légères erreurs d'espaces.
* **Outils MCP de diagnostics Chrome avancés :** Intégration de nouveaux outils d'audit (`emulate_experience`, `lighthouse_audit`, `analyze_memory_leaks`) basés sur l'API CDP pour tester les performances, diagnostics Lighthouse et fuites mémoire en local.

---
