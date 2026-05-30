# Changelog

All notable changes to this project will be documented in this file.

## [2026-05-30 19:42:00]
- `src-tauri/src/rules.rs` : Ajout d'un filet de sécurité à la consolidation par IA. Avant d'écraser `instructions_consolidated.md`, l'ancienne version est sauvegardée (`instructions_consolidated.bak.md`), et la réécriture est refusée si le résultat de l'IA est anormalement court (moins de la moitié de l'actuel) afin d'éviter toute perte silencieuse de règles en cas de réponse tronquée du modèle.

## [2026-05-30 19:17:08]
- `crates/sinew-app/src/agent/repeat_guard.rs` (nouveau) : Ajout du maillon « Capture » manquant du système d'auto-amélioration. Détecte quand l'agent rejoue la même commande shell ou le même appel d'outil en erreur sans progresser : injecte un rappel fort à 3 répétitions, puis enregistre l'incident dans `errors_raw.json` et coupe le tour à 4. Ces incidents alimentent désormais automatiquement la consolidation en règles globales (avant, `errors_raw.json` n'était jamais rempli par le code).
- `crates/sinew-app/src/agent/turn.rs` : Branchement du détecteur de boucle dans la boucle d'outils (observation de chaque résultat, injection du rappel dans le prompt système, enregistrement + arrêt propre du tour avec événement d'erreur quand une boucle est avérée).
- `crates/sinew-app/src/agent.rs` : Déclaration du module `repeat_guard`.
- `crates/sinew-app/Cargo.toml` : Ajout de la dépendance `chrono` (horodatage des incidents).
## [2026-05-30 18:22:51]
- `src/components/SettingsPane.tsx` : Déplacement de "Optimisation Magique Auto" en tête de l'onglet Power User sous forme de carte pleine largeur responsive. Rétablissement de l'accès au menu de choix de modèle d'analyse indépendamment de l'état d'activation de l'optimisation. Transfert de l'option "Recherche de mise à jour automatique" vers l'onglet Système (Diagnostics) pour une cohérence thématique parfaite.
- `src/styles.css` : Ajout de la classe `.settings-pane__select` pour habiller proprement les listes déroulantes de paramètres.

## [2026-05-30 18:21:34]
- `crates/sinew-anthropic/src/client.rs` : Ajout de la méthode `get_usage` pour récupérer les limites de quota Anthropic via l'API OAuth `/api/oauth/usage` avec gestion du rafraîchissement des jetons.
- `src-tauri/src/providers.rs` : Ajout de la commande Tauri `get_anthropic_usage` et définition du cache associé pour récupérer l'utilisation de l'abonnement Anthropic.
- `src-tauri/src/lib.rs` : Exposition de la commande Tauri `get_anthropic_usage` au processus frontal.
- `src/lib/ipc.ts` : Déclaration de la méthode IPC `getAnthropicUsage`.
- `src/lib/quotas.ts` : Implémentation de la récupération et du parsing du quota pour le fournisseur Anthropic en utilisant les données de l'API d'abonnement Anthropic.

## [2026-05-30 18:18:38]
- `src/components/SettingsPane.tsx` : Correction des problèmes d'encodage pour le bouton "Jour" et "Système" dans l'onglet Apparence, remplacement par des emojis corrects. Remplacement des réglages de taille numérique par une structure de classe CSS unifiée et correction de l'icône de titre pour DeepSeek.
- `src/styles.css` : Ajout de la classe `.settings-pane__number-adjuster` pour uniformiser la disposition des ajusteurs numériques. Conversion de la règle responsive `@media` en `@container` query sur les lignes à deux colonnes pour qu'elles se comportent correctement suivant la taille du panneau de réglages, et élargissement de la bascule adaptative des cartes de 500px à 600px pour une meilleure lisibilité mobile.

## [2026-05-30 17:51:00]
- `src/components/chat/chatUtils.ts` : Ajout de la fonction `playNotificationSound` utilisant l'API Web Audio pour jouer un carillon agréable à la fin d'un chat.
- `src/components/chat/ChatPane.tsx` : Importation et déclenchement de la sonnerie de notification lors de la réception de l'événement `turn_finished`.
- `src/components/SettingsPane.tsx` : Ajout de l'option de configuration pour activer/désactiver la sonnerie de fin de chat sous l'onglet Apparence, avec synchronisation de l'état dans le stockage local.

## [2026-05-30 16:58:12]
- `src/components/Workspace.tsx` : Correction du comportement des menus de fermeture d'onglets (Fermer les autres, Fermer à droite, Fermer tous) pour qu'ils ferment également l'onglet "Paramètres" s'il est ouvert et actif, évitant ainsi le rendu d'un écran noir vide.
- `src/components/EditorPane.tsx` : Mise à jour du composant `EditorTabContextMenu` pour accepter la propriété `settingsOpen`. Le bouton "Fermer les onglets à droite" reste désormais cliquable même sur le dernier onglet fichier si l'onglet Paramètres est ouvert (puisqu'il est situé à sa droite).

## [2026-05-30 16:55:47]
- `REFACTORING_PLAN.md` : Création du rapport d'audit global. Identification et priorisation des 3 meilleures cibles de rationalisation : unification des providers LLM (Rust), découpage des composants monolithiques SettingsPane/ChatPane (React), et standardisation du pipeline d'outils (Rust).

## [2026-05-30 16:17:12]
- `src-tauri/src/workspace.rs` : Automatisation du déploiement du daemon Linux "Super SSH". Si le binaire local est absent, la commande de démarrage télécharge automatiquement la dernière release stable du daemon `sinew-agent-daemon-linux` via `curl` sur le serveur distant depuis GitHub, garantissant une utilisation sans configuration pour les utilisateurs.
- `.github/workflows/release.yml` : Ajout d'une étape de compilation native du daemon (`cargo build --release -p sinew-agent-daemon`) sur les runners Linux de l'intégration continue. Le binaire est ensuite publié en tant qu'asset additionnel (`sinew-agent-daemon-linux`) sur les releases GitHub.



## [2026-05-30 16:01:00]
- `src-tauri/src/workspace.rs` : Création de la commande Tauri `mount_super_ssh_workspace` qui orchestre la connexion "Super SSH". Cette commande déploie dynamiquement le daemon natif compilé sur le serveur distant via SCP, le démarre en arrière-plan, puis met en place un port forwarding SSH local (47990 -> 127.0.0.1:47990) sans bloquer l'interface.
- `src-tauri/src/workspace.rs` : Ajout de la méthode `proxy_to_daemon` et interception des requêtes de fichiers (`list_workspace_entries_command`, `list_workspace_files_command`, `read_workspace_file_command`, `write_workspace_file_command`, `search_workspace_files_command`). Si l'espace de travail est préfixé par `super-ssh://`, les commandes ne lisent pas le disque local mais encapsulent l'appel en JSON et le transmettent au daemon distant sur le port 47990 pour une latence nulle.
- `src-tauri/src/turns.rs` : Modification du routeur d'agents (`run_turn_via_daemon`). En mode Super SSH, la communication ne s'établit plus sur le pipe nommé Windows `\\.\pipe\sinew-agent-ipc` mais via une connexion TCP directe (`127.0.0.1:47990`) vers le daemon Linux du serveur.
- `crates/sinew-app/src/workspace.rs` : Dérivation et exposition complète des traits de désérialisation (`Deserialize`) sur toutes les structures liées aux résultats de requêtes de l'espace de travail (`WorkspaceEntry`, `FileDocument`, `WorkspaceSearchResult`, etc.) pour supporter le parsing des données distantes renvoyées par le proxy.
- `src-tauri/src/lib.rs` : Exposition de la commande système `mount_super_ssh_workspace` au processus IPC Tauri pour permettre au client React de l'invoquer depuis `Welcome.tsx`.

## [2026-05-30 16:09:40]
- `src-tauri/src/turns.rs` : Assouplissement du prompt système de la règle "Maquettes Visuelles Automatiques" suite aux retours utilisateurs. L'agent est désormais encouragé à générer proactivement des diagrammes Mermaid pour illustrer ses explications ou son architecture, au lieu de s'en priver par peur de bloquer l'édition de fichiers.
- `src/components/SettingsPane.tsx` : Mise à jour de la description UI de la règle des maquettes visuelles pour refléter le changement (passage d'un comportement passif "uniquement si demandé" à un comportement proactif "Génère spontanément des schémas").

## [2026-05-30 16:05:00]
- `crates/sinew-agent-daemon/src/protocol.rs` & `crates/sinew-agent-daemon/src/main.rs` : Ajout des requêtes `ListEntries`, `ListAllFiles`, `ReadFile` et `WriteFile` au protocole IPC du démon. Cela permet au frontend de lire et écrire des fichiers distants via le proxy TCP en mode Super SSH, sans nécessiter de point de montage SSHFS, garantissant une latence nulle.
- `src/components/Welcome.tsx` : Ajout de l'option de sélection "Super SSH (Native Agent)" dans le formulaire de connexion SSH pour utiliser le nouveau mode de connexion proxy distant SOTA.
- `src/lib/ipc.ts` : Ajout de la méthode `mountSuperSshWorkspace` qui appelle la commande Tauri `connect_super_ssh` de `@backend_ssh` pour gérer la connexion "Super SSH".

## [2026-05-30 16:01:00]
- `crates/sinew-agent-daemon/src/main.rs` : Adaptation du démon persistant pour écouter sur le port TCP `127.0.0.1:47990` sur les environnements Linux (en plus de l'écoute sur le Named Pipe sous Windows), permettant la création de l'architecture "Super SSH" et la connexion proxy du frontend vers le serveur distant.


- `src/components/SettingsPane.tsx` : Amélioration de "Optimisation Magique Auto" qui s'active de manière totalement invisible lors de l'appui sur "Entrée" dans le panneau de chat.
- `src/components/chat/ChatPane.tsx` : Interception de l'envoi de message pour appeler l'API d'optimisation, basculer le mode (Action, Plan, Objectif) en temps réel, puis relayer automatiquement le prompt structuré à l'agent sans aucune friction.
- `src-tauri/src/turns.rs` : Remplacement complet de la méthode d'extraction du JSON par une recherche textuelle du bloc JSON au lieu du pattern `trim` pour garantir que le prompt de retour soit correctement identifié même s'il est noyé dans le texte.

## [2026-05-30 16:02:15]
- `src/components/SettingsPane.tsx` : Ajout de deux nouvelles options avancées (Power User) dans l'interface : "Résolution Stricte des Problèmes" (pour interdire à l'agent de contourner les erreurs) et "Implémentation Complète" (pour interdire les faux blocs de code ou les commentaires TODOs).
- `src/lib/ipc.ts` : Transmission des nouvelles préférences `strictProblemSolving` et `fullImplementation` via les payloads IPC d'estimation et d'envoi de messages.
- `src-tauri/src/state.rs` : Création des constantes `DEFAULT_STRICT_PROBLEM_SOLVING_PROMPT` et `DEFAULT_FULL_IMPLEMENTATION_PROMPT` définissant les consignes à injecter dans le contexte de l'IA.
- `src-tauri/src/models.rs` & `src-tauri/src/turns.rs` & `src-tauri/src/context.rs` : Injection de ces nouvelles instructions système aux prompts racines du moteur Rust.

## [2026-05-30 16:01:20]
- `src-tauri/src/state.rs` & `src-tauri/src/turns.rs` : Ajout d'une consigne système permanente (`DEFAULT_SSH_OPTIMIZATION_PROMPT`) pour les agents. Lorsqu'ils détectent être sur un espace de travail distant monté en SSHFS, les agents utiliseront désormais leurs outils MCP natifs (`ssh_exec`) pour installer des utilitaires (SOTA) et exécuter des scripts directement sur le serveur, afin de contourner la latence réseau des outils de recherche de fichiers en local.



## [2026-05-30 15:44:34]
- `src/components/Workspace.tsx` : Suppression du composant de connexion SSH dans la barre latérale inférieure de l'espace de travail. Le bouton "Se connecter" et ses états associés ont été retirés pour éviter toute confusion avec le changement de projet, réservant ainsi l'accès SSH à l'écran de bienvenue.



## [2026-05-30 15:44:10]
- `src-tauri/src/turns.rs` : Masquage de l'avertissement répétitif (`WARN`) signalant l'absence du démon d'agent (qui bascule silencieusement sur le moteur local) en abaissant le niveau de log à `DEBUG` pour ne plus polluer la console. Arrêt propre de la tentative de création de processus si le binaire `sinew-agent-daemon.exe` n'est pas compilé.

## [2026-05-30 15:43:00]
- `src-tauri/src/turns.rs` & `src/lib/ipc.ts` : Ajout d'une commande Tauri `optimize_prompt` qui fait appel au modèle sélectionné pour agir comme "Prompt Engineer". Le modèle analyse le brouillon de l'utilisateur, détermine le mode optimal (`act`, `plan`, `goal`), et retourne une consigne réécrite de qualité professionnelle.
- `src/components/chat/ChatPane.tsx` : Intégration d'un bouton "Baguette magique" (Optimiser) directement dans le panneau de saisie. Un clic envoie le brouillon à l'IA d'optimisation, remplace le texte par la consigne SOTA structurée, et bascule le menu déroulant sur le mode recommandé automatiquement.

## [2026-05-30 15:35:10]
- `src-tauri/src/models.rs` : Neutralisation de warnings du compilateur Rust (`dead_code`) sur les structures `OptimizePromptInput` et `OptimizePromptOutput`.

## [2026-05-30 15:27:26]
- `src-tauri/src/tray.rs` : Nettoyage du code Rust. Suppression des imports de modules inutilisés (`DesktopState`, `Runtime`, `Manager`) et des variables mortes (`handle`, `tray`) signalés par le compilateur pour maintenir une base de code propre et sans avertissements.

## [2026-05-30 15:25:37]
- `Cargo.lock`, `Cargo.toml`, `package.json`, `src-tauri/tauri.conf.json` : Mise à jour de la version vers la `0.1.27` via la fusion avec le dépôt upstream.
- `crates/sinew-app/src/store.rs` : Amélioration SOTA du mode Plan. L'IA a interdiction de détailler l'implémentation (code, shell), mais doit impérativement conserver les choix de design, les technologies et les paramètres décidés lors de la discussion pour un plan plus précis et actionnable.
- `src-tauri/src/state.rs` : Simplification du prompt système pour supprimer l'obligation fastidieuse des mises à jour utilisateur toutes les 30 secondes en mode exploration, rendant l'IA plus silencieuse et efficace.

## [2026-05-30 15:21:45]
- `src/components/SettingsPane.tsx` : Rétablissement intégral des emojis et des caractères accentués suite à une corruption d'encodage (passage de UTF-8 à ISO-8859-1). Remplacement par regex de toutes les entités corrompues (ex: soleil, lune, ordinateur, etc.) pour garantir un affichage propre du panneau de paramètres.

## [2026-05-30 15:14:38]
- `src/components/SettingsPane.tsx` : Correction des erreurs d'encodage (mojibake) où les caractères accentués français apparaissaient sous la forme de caractères corrompus à cause d'une précédente écriture dans un format de texte incorrect. Le fichier a été restauré en UTF-8 pur.
- `AGENTS.md` : Correction identique de l'encodage pour restaurer les caractères français.

## [2026-05-30 15:10:00]
- `src-tauri/Cargo.toml` & `Cargo.toml` : Activation du plugin natif `tray-icon` pour afficher l'icône de Sinew dans la zone de notification Windows.
- `src-tauri/src/tray.rs` : Création du module de gestion de l'icône système (Tray Icon) pour proposer le menu des projets récents lors d'un clic droit sur l'icône Sinew (en bas à droite).
- `src-tauri/src/lib.rs` : Intégration et exposition des commandes Tauri (`get_recent_workspaces_command`, `record_recent_workspace_command`, `clear_recent_workspaces_command`) pour que les projets récents soient sauvegardés directement sur le disque (en Rust) plutôt que dans le `localStorage` volatile du navigateur.
- `src/lib/recents.ts` : Réécriture de la gestion des projets récents pour s'interfacer avec le backend Rust de manière asynchrone, garantissant la persistance des projets même si les données du navigateur sont effacées.
- `src/App.tsx` : Modification du processus de démarrage de l'application pour utiliser le dernier projet enregistré via le backend Rust, garantissant que Sinew s'ouvre toujours sur le dernier espace de travail de façon très fiable et ne demande plus le dossier à chaque ouverture.

## [2026-05-30 15:07:15]
- `src/lib/recents.ts` & `src/components/Welcome.tsx` : Amélioration SOTA pour le "Sans dossier" (Sandbox). Au lieu de le cacher ou de l'afficher comme un dossier système brut, il est désormais intégré à l'historique avec une interface dédiée.
- src/components/chat/TodoStrip.tsx : Implémentation d'une vue Kanban temps réel pour le mode Swarm (Essaim d'agents), remplaçant la liste plate par des colonnes 'À faire', 'En cours', 'Bloqué' et 'Terminé'.
- crates/sinew-app/src/agent/tool_dispatch.rs : Implémentation du mode Auto-Lint Ghost-Loop ! Les appels à 'edit_file' et 'write_file' incluent désormais instantanément les retours des linteurs (cargo, eslint, etc.) s'ils ont échoué, forçant l'agent à s'auto-corriger dans la foulée.
- src/components/chat/ChatPane.tsx : Extraction chirurgicale des fonctions utilitaires pures (formatBytes, formatTurnDuration, etc.) vers 'chatUtils.ts' pour alléger le composant monolithique sans risque de conflits.: icône de boîte distinctive (`solar:box-bold-duotone`), nom "Brouillon actif (Sandbox)", et un sous-titre clair ("Dernier espace de travail temporaire") masquant le chemin technique.

## [2026-05-30 15:04:00]
- src/components/SettingsPane.tsx : Déplacement des barres de quotas sous les boutons d'action (Se déconnecter/Se connecter) afin de gagner de l'espace horizontal.
- src/components/SettingsPane.tsx : Découpage de l'onglet massif 'Options' en 3 nouveaux onglets dédiés dans la navigation principale ('Apparence', 'Power User', et 'Système') pour aérer l'interface.
- src/components/SettingsPane.tsx : Déplacement de la 'Synchronisation Multi-PC', 'Recherche Sémantique', et 'Apprentissage Automatique IA' vers le nouvel onglet 'Système'. Harmonisation de leur apparence dans la grille dédiée.
- src/components/chat/TodoStrip.tsx : Implémentation d'une vue Kanban temps réel pour le mode Swarm (Essaim d'agents), remplaçant la liste plate par des colonnes 'À faire', 'En cours', 'Bloqué' et 'Terminé'.
- crates/sinew-app/src/agent/tool_dispatch.rs : Implémentation du mode Auto-Lint Ghost-Loop ! Les appels à 'edit_file' et 'write_file' incluent désormais instantanément les retours des linteurs (cargo, eslint, etc.) s'ils ont échoué, forçant l'agent à s'auto-corriger dans la foulée.
- src/components/chat/ChatPane.tsx : Extraction chirurgicale des fonctions utilitaires pures (formatBytes, formatTurnDuration, etc.) vers 'chatUtils.ts' pour alléger le composant monolithique sans risque de conflits.sur les cartes fournisseurs et éviter qu'elles ne soient écrasées sur de petites résolutions.
- src/components/SettingsPane.tsx : Retrait des mentions explicites de numéros de version (ex: V3 & R1) pour la description de DeepSeek, car ces informations évoluent vite.
- src/lib/frRuntime.ts : Raccourcissement de "Limite atteinte" en "Limite" pour optimiser l'affichage.
- src/components/SettingsPane.tsx : Masquage automatique de la description du fournisseur une fois connecté pour gagner de la place verticalement.
- src/components/SettingsPane.tsx : Correction de l'affichage interne des sous-cartes de fournisseurs pour que les barres de quotas ne détruisent plus l'alignement des emails.
- src/components/SettingsPane.tsx : Optimisation extrême des sous-cartes : les quotas sont maintenant empilés proprement sur toute la largeur, et les détails (emails, plan, etc.) sont fusionnés sur une seule ligne condensée.
- src/components/SettingsPane.tsx : Transformation des quotas des sous-cartes en blocs pleine largeur avec l'info au-dessus de la barre pour exploiter tout l'espace horizontal.
- src/components/SettingsPane.tsx : Découpage de l'onglet massif 'Options' en 3 nouveaux onglets dédiés dans la navigation principale ('Apparence', 'Power User', et 'Système') pour aérer l'interface.
- src/components/SettingsPane.tsx : Déplacement de la 'Synchronisation Multi-PC', 'Recherche Sémantique', et 'Apprentissage Automatique IA' vers le nouvel onglet 'Système'. Harmonisation de leur apparence dans la grille dédiée.
- src/components/chat/TodoStrip.tsx : Implémentation d'une vue Kanban temps réel pour le mode Swarm (Essaim d'agents), remplaçant la liste plate par des colonnes 'À faire', 'En cours', 'Bloqué' et 'Terminé'.
- crates/sinew-app/src/agent/tool_dispatch.rs : Implémentation du mode Auto-Lint Ghost-Loop ! Les appels à 'edit_file' et 'write_file' incluent désormais instantanément les retours des linteurs (cargo, eslint, etc.) s'ils ont échoué, forçant l'agent à s'auto-corriger dans la foulée.
- src/components/chat/ChatPane.tsx : Extraction chirurgicale des fonctions utilitaires pures (formatBytes, formatTurnDuration, etc.) vers 'chatUtils.ts' pour alléger le composant monolithique sans risque de conflits.
## [2026-05-30 15:02:20]
- `src/lib/recents.ts` : Exclusion automatique du dossier temporaire (`.sinew-sandbox` ou "Sans dossier") de la liste des projets récents affichés sur la page d'accueil pour éviter de polluer l'historique de l'utilisateur, tout en conservant la possibilité de le rouvrir automatiquement au prochain lancement si c'était le dernier projet actif.

## [2026-05-30 14:57:30]
- `src/components/SettingsPane.tsx` : Ajout d'une fonctionnalité permettant de masquer les fournisseurs de modèles non utilisés (bouton œil barré sur les fournisseurs non connectés) et de les réafficher depuis une nouvelle section "Ajouter un fournisseur" en bas de page. Seuls OpenAI, Google et DeepSeek sont affichés par défaut (ainsi que les fournisseurs ayant déjà des identifiants/comptes).


## [2026-05-30 14:56:52]
- `src/components/SettingsPane.tsx` : Correction d'un problème d'affichage où le composant `QuotaBar` en mode inline dépassait du bord de la carte pour les fournisseurs ayant de longs libellés de quotas (ex: "Claude & GPT-OSS"). Ajout de `maxWidth: "100%"`, `overflow: "hidden"` et `textOverflow: "ellipsis"` pour tronquer proprement le texte à l'intérieur.

## [2026-05-30 14:55:00]
- `src/components/chat/ToolCard.tsx` : Suppression complète du bouton "Auto" / "Auto-fix", car les relances et correctifs automatiques doivent être gérés par l'agent de manière autonome sans nécessiter d'intervention manuelle.

## [2026-05-30 14:54:12]
- `src/components/SettingsPane.tsx` : Ajout de paliers de couleurs supplémentaires pour le solde DeepSeek : Rouge (<10$), Orange (<20$), Jaune (<40$), Vert clair (<60$), Émeraude (<80$), Cyan (<100$) et Bleu Tech (>=100$).

## [2026-05-30 14:51:15]
- `src/components/chat/ToolCard.tsx` : Modification du bouton "Auto" en "Auto-fix" avec une nouvelle icône baguette magique, car l'ancienne icône de sliders ressemblait à un caractère chinois et son utilité n'était pas claire.

## [2026-05-30 14:52:10]
- `src/components/SettingsPane.tsx` : Ajout d'un code couleur dynamique pour l'affichage du solde (ex: DeepSeek) : rouge si le solde est inférieur à 10$, orange s'il est entre 10$ et 20$, et vert au-delà.

## [2026-05-30 14:48:45]
- `src/components/SettingsPane.tsx` : Agrandissement de la police (de 11px à 13px) et changement de la couleur en vert émeraude (`#10b981`) pour le texte du solde restant DeepSeek lorsque le pourcentage est nul, afin qu'il soit bien plus visible et lisible ("plus gros").

## [2026-05-30 14:46:07]
- `src/lib/quotas.ts` : Suppression du calcul inutile du pourcentage pour DeepSeek car le point de terminaison de l'API rendait toujours `100%` (le dénominateur est souvent égal au reste), renvoyant `null` à la place.
- `src/components/SettingsPane.tsx` : Modification de l'affichage en ligne du composant `QuotaBar` pour masquer la barre de progression complète et afficher uniquement le texte centré et agrandi lorsque le pourcentage restant est nul (`null`), ce qui permet d'afficher la ligne de crédit DeepSeek ("Restant $17.08") plus clairement sans une barre `100%` trompeuse.


- `src-tauri/src/turns.rs` : Implémentation de la lecture "Hot-Reload" en temps réel des règles d'IA (Cerveau Cloud via OneDrive). L'application lit désormais dynamiquement `instructions_consolidated.md` directement depuis OneDrive (avant de basculer sur LocalAppData) à chaque nouveau message, permettant des améliorations SOTA immédiates sans recharger ni recompiler l'application.

## [2026-05-30 14:21:14]
- `AGENTS.md` : Clarification de la règle sur `cwd` dans le terminal pour éviter la confusion avec la règle d'utilisation des chemins absolus (les chemins absolus sont interdits pour l'outil bash mais requis pour read/write).

## [2026-05-30 14:14:00]
- `crates/sinew-cursor/src/agent/run_h2.rs` : Remplacement de l'appel `hyper::body::to_bytes(resp.into_body()).await` par `resp.into_body().collect().await.map(|c| c.to_bytes())` pour s'adapter à l'API de Hyper 1.x et corriger l'erreur de compilation `E0425: cannot find function to_bytes in module hyper::body` remontée dans `build-error.txt`.

## [2026-05-30 14:10:18]
- `AGENTS.md` : Ajout d'une règle d'auto-amélioration globale pour documenter l'obligation de fournir le paramètre `limit` avec `grep`/`glob` et de vérifier l'existence des fichiers avant l'appel (via `Test-Path`), suite aux erreurs de l'agent.

## [2026-05-30 14:06:32]
- `src-tauri/capabilities/default.json` : Extension des fenêtres autorisées à toutes (`*` au lieu de `main, sinew-window-*`) et ajout explicite de la permission `dialog:allow-message` pour s'assurer que le frontend puisse exécuter `window.confirm` sur n'importe quelle fenêtre Tauri sans blocage ACL.

## [2026-05-30 14:04:09]
- `src-tauri/capabilities/default.json` : Ajout des permissions `dialog:allow-confirm` et `dialog:allow-ask` pour autoriser le frontend à utiliser les boîtes de dialogue de confirmation natives via `window.confirm`, résolvant l'erreur d'autorisation ACL `Command plugin:dialog|confirm not allowed by ACL` identifiée dans le fichier de log `frontend-error.log`.

## [2026-05-30 11:26:12]
- `src/components/chat/ToolCard.tsx` : Remplacement du libellé du bouton de dépannage de "Auto-réparer" par "Auto" pour plus de clarté.

## [2026-05-30 11:17:33]
- `src/styles.css` : Ajustement des styles de l'encart SSH pour forcer l'affichage de l'état de connexion et du bouton sur une seule ligne (sans retour à la ligne) avec troncature automatique du texte en cas de manque d'espace.

## [2026-05-30 11:15:47]
- src/components/SettingsPane.tsx : Ajout de la sérialisation du champ id dans settingsToJson pour préserver les identifiants uniques des serveurs MCP (tels que sinew-chrome ou mcp_ssh_mcp) lors de leur affichage et de leur édition dans la configuration avancée du frontend, résolvant le problème d'outils manquants dû à des ID non concordants entre le backend et le frontend.
- crates/sinew-app/src/edit.rs : Normalisation en minuscules des chemins relatifs sous Windows lors de la modification de fichiers pour assurer la cohérence avec l'outil de lecture et éviter le blocage de sécurité (read-before-write) dû aux différences de casse.

## [2026-05-30 11:14:58]
- `src-tauri/src/rules.rs` : Suppression automatique du marqueur de début de fichier UTF-8 (BOM `\u{FEFF}`) lors de la lecture de `errors_raw.json` pour éviter l'erreur d'analyse JSON `Format errors_raw.json invalide: expected value at line 1 column 1` qui bloquait la consolidation des règles par l'IA.

## [2026-05-30 04:01:40]
- `src/components/Welcome.tsx` : Enregistrement de l'hôte SSH connecté dans le stockage local du navigateur (`localStorage`) pour pouvoir l'identifier plus tard.
- `src/components/Workspace.tsx` : Ajout d'un encart de connexion SSH au bas de la colonne de gauche (barre latérale). Il affiche un indicateur vert avec le nom du serveur si connecté, ainsi qu'un bouton de déconnexion. Sinon, il propose un bouton "Se connecter" qui ouvre un petit formulaire intégré avec gestion des connexions rapides.
- `src/styles.css` : Ajout des styles graphiques pour le nouvel encart de connexion SSH en bas à gauche de l'interface.

## [2026-05-30 03:57:31]
- `src/lib/quotas.ts` : Correction du calcul du pourcentage DeepSeek — la barre incluait seulement le solde rechargé (`toppedUpBalance`) comme dénominateur, ce qui donnait toujours ≥100% tant que des crédits gratuits (`grantedBalance`) étaient disponibles. Le dénominateur devient `toppedUpBalance + grantedBalance` pour refléter le total réel.

## [2026-05-30 03:55:30]
- **Icônes globales — 12 fichiers, ~50 icônes modernisées** :
  - `circle` → `square` : `close` (20 occurrences), `add` (8), `minus` (3) — style carré plus net et cohérent.
  - `trash-bin-trash` → `trash-bin-minimalistic` : (6 occurrences) icône poubelle plus moderne.
  - `linear` → `bold` : toutes les flèches (`alt-arrow-right/down/up`, `square-alt-arrow-up/down`) pour une meilleure visibilité.
  - Spécifiques : `wrench` → `tuning`, `download-linear` → `download-square`, `play-linear` → `play-circle`, `rewind-back` → `undo-left`, `clock-circle` → `history`.

## [2026-05-30 03:53:56]
- `src/components/chat/ChatPane.tsx` : 12 icônes modernisées — zoom lightbox (`magnifer-zoom` → `minimize/maximize-square`), pièce jointe (`paperclip-bold` → `paperclip-rounded-bold`), retour (`alt-arrow-left` → `arrow-left`), scroll question (`arrow-up` → `arrow-to-top-left`), téléchargement (`download-linear` → `download-square`), retour arrière (`rewind-back` → `undo-left`), archives `linear` → `bold` (3 occurrences), historique (`clock-circle` → `history`).
- `src/components/chat/ToolCard.tsx` : Icône clé à molette (`wrench` → `tuning`) plus moderne, stop (`stop-circle-linear` → `stop-bold`).
- `src/components/chat/MermaidDiagram.tsx` : Zoom (`minus-circle`/`add-circle` → `minimize/maximize-square`) plus cohérent avec la lightbox.
- `src/components/chat/TodoStrip.tsx` : Flèches expand/collapse (`alt-arrow-down/up-linear` → `bold`) plus visibles.

## [2026-05-30 03:53:45]
- `crates/sinew-app/src/agent/turn.rs` : Passage de `info!` à `debug!` pour compaction et turn_finished (uniformité totale).

## [2026-05-30 03:51:25]
- `src-tauri/src/main.rs` : Ajout d'un panic hook global qui capture toutes les panics Rust dans `logs/panic.log` avant le crash.
- `src-tauri/src/lib.rs` : Ajout de la commande `log_frontend_error` qui écrit les erreurs du frontend dans `logs/frontend-error.log`.
- `src/main.tsx` : Ajout de `window.onerror` and `window.onunhandledrejection` qui capturent toutes les erreurs JS/React et les envoient au backend.
- `src/lib/ipc.ts` : Ajout de la méthode `logFrontendError`.
- **Couverture erreurs totale** : panics Rust + erreurs React/JS + erreurs bridge = tout dans `logs/`.

## [2026-05-30 03:48:15]
- `scripts/agent-bridge/run-stream.mjs` : Ajout logger JSON fichier vers `logs/agent-bridge.log` + timers (bridge_start, h2_connected, mcp_tool_exec, bridge_end).
- `scripts/agent-bridge/h2-bridge.mjs` : Ajout logger JSON fichier vers `logs/h2-bridge.log` + timers (bridge_start, h2_stream_end/error).
- `%LOCALAPPDATA%/sinew/ChromeBridge/server.js` : Mirror des logs vers `logs/chrome-bridge.log`.
- `%LOCALAPPDATA%/sinew/ChromeBridge/mcp_server.js` : Ajout log fichier vers `logs/chrome-mcp.log`.
- **Centralisation totale** : tous les logs (Rust + Node.js bridges) convergent maintenant dans `%LOCALAPPDATA%/dev/hyrak/sinew/data/logs/`.

## [2026-05-30 03:41:07]
- `crates/sinew-app/src/web.rs` : Ajout timers web_search + web_fetch.
- `crates/sinew-app/src/store.rs` : Ajout timer load_conversation.
- `crates/sinew-cursor/src/agent/run_h2.rs` : Ajout timer Cursor bridge h2 (durée totale + output tokens).
- `crates/sinew-index/src/search.rs` : Ajout timer workspace search.
- Couverture complète atteinte : tous les points d'entrée/sortie majeurs sont maintenant tracés avec durée.

## [2026-05-30 03:35:24]
- `src-tauri/src/rules.rs` : Refonte du prompt système de `ai_consolidate_rules()` avec un système de confiance à 3 niveaux (🟢 ACTIVE / 🟡 CANDIDATE / 🔴 OBSOLÈTE), traçabilité complète (origine des erreurs, dates, règles remplacées), et dégradation automatique des règles obsolètes (2+ mois sans mise à jour ou contredites par une règle plus récente).

## [2026-05-30 03:24:31]
- `src-tauri/src/lib.rs` : Changement du filtre de log par défaut de `info` à `trace` pour tous les crates Sinew (`sinew_app`, `sinew_cursor`, `sinew_openai`, `sinew_anthropic`, `sinew_google`, `sinew_kimi`, `sinew_deepseek`, `sinew_openrouter`, `sinew_index`, `sinew_core`). Les libs externes restent à `warn`/`debug` pour éviter le bruit. Le fichier de log passe de `desktop-app.log` à `logs/sinew.log` avec rotation à 64 Mo.
- `crates/sinew-app/src/agent/turn.rs` : Ajout de timers de précision (stream setup, premier token, exécution de chaque outil, compaction automatique, durée totale du tour) avec `tracing::debug!` et `tracing::info!`.
- `crates/sinew-app/src/store.rs` : Ajout d'un timer sur `save_conversation` (temps SQLite + sérialisation).
- `crates/sinew-anthropic/src/client.rs` : Ajout d'un timer HTTP round-trip sur le stream Anthropic.
- `crates/sinew-openai/src/client.rs` : Ajout d'un timer HTTP round-trip sur le stream SSE OpenAI.
- `crates/sinew-google/src/client.rs` : Ajout d'un timer HTTP round-trip sur le stream Google Antigravity.
- `crates/sinew-deepseek/src/client.rs` : Ajout d'un timer HTTP round-trip dans `send_json`.
- `crates/sinew-kimi/src/client.rs` : Ajout d'un timer HTTP round-trip dans `send_json` (inclut les retries 401).
- `crates/sinew-openrouter/src/client.rs` : Ajout d'un timer HTTP round-trip sur le stream OpenRouter.
- **Centralisation logs** : Tous les logs sont maintenant dans `%LOCALAPPDATA%/dev/hyrak/sinew/data/logs/sinew.log`.

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
- `src/lib/ipc.ts` : Exposition de la méthode API `listSshHosts`.
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
- `crates/sinew-agent-daemon/src/main.rs` : Implémentation du serveur d'écoute asynchrone multithread gérant les connexions entrantes sur le Named Pipe et le traitement des messages JSON-RPC de limités par des retours à la ligne (`\n`).


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
* **Visualisation du plan d'action (Planning Board) :** Intégration d'un bloc dynamique interactif (`PlanningNextMoveBlock`) montrant en temps réel les prochaines étapes planifiées by le Swarm d'agents.
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
* **Bouton de synchronisation forcée :** Ajout d'un bouton de synchronisation immédiate à la demande dans les paramètres pour déclencher manuellement la synchronisation du dossier OneDrive local.

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











