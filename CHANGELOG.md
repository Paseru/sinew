# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased] - 2026-05-30 01:22:35

### Added
- **Règles anti-boucle locales (AGENTS.md)** : ajout de consignes explicites pour garder `cwd` dans le workspace, relire `CHANGELOG.md` juste avant chaque modification et utiliser les chemins Windows absolus afin d'éviter les erreurs répétées vues avec OpenAI.
- **Traçabilité (CHANGELOG.md)** : ajout de cette entrée pour documenter la nouvelle règle de prévention.

### Improved
- **Performance globale adaptative (`crates/sinew-app/src/store.rs`)** : la base principale de conversations utilise un cache adapte aux coeurs disponibles, la lecture memoire SQLite et un delai d'attente plus robuste pour mieux exploiter les PC puissants sans reglage fixe.
- **Recherche de fichiers plus rapide (`crates/sinew-app/src/workspace.rs`, `crates/sinew-app/Cargo.toml`)** : la recherche dans l'espace de travail se repartit maintenant sur les coeurs CPU disponibles afin de reduire les temps d'attente sur les gros projets.
- **Interface plus fluide pendant les grosses lectures (`src-tauri/src/workspace.rs`)** : les listes de fichiers, recherches et lectures de fichiers passent sur un couloir de travail separe pour eviter de figer l'application pendant les operations disque lourdes.
- **Tracabilite (`CHANGELOG.md`)** : ajout de cette entree pour documenter les optimisations globales demandees.
## ðŸš€ PrÃ©sentation des FonctionnalitÃ©s Majeures (Fork Premium julienpiron.fr)

Cette version a Ã©tÃ© optimisÃ©e en profondeur pour offrir une expÃ©rience utilisateur haut de gamme (SOTA), une autonomie maximale en arriÃ¨re-plan, et des intÃ©grations d'intelligence artificielle inÃ©galÃ©es.

### ðŸŽ¨ Interface, Confort & Ergonomie (Premium UI)
* **Animation de dÃ©marrage premium :** Une animation de boot moderne, fluide et Ã©lÃ©gante Ã  l'ouverture de l'application.
* **3 niveaux de rÃ©flexion :** Choix entre DÃ©taillÃ©, Compact ou TrÃ¨s compact pour configurer prÃ©cisÃ©ment la verbositÃ© de l'IA et le masquage des dÃ©tails techniques dans le chat.
* **Question collante (Sticky Question) :** La question en cours de traitement reste Ã©pinglÃ©e en haut de l'Ã©cran pendant que vous faites dÃ©filer le fil de discussion.
* **Menu clic droit interactif sur les onglets de l'Ã©diteur :** Clic droit (ou `F10`) sur les onglets pour fermer l'onglet (raccourci `Ctrl+F4`), les autres, Ã  sa droite ou tous, copier le chemin (absolu ou relatif) et rÃ©vÃ©ler dans le Finder/Explorateur.
* **Menu clic droit d'exÃ©cution :** Clic droit sur les fichiers dans le chat et l'arbre des fichiers pour les ouvrir, les rÃ©vÃ©ler ou les exÃ©cuter directement.
* **Polices dynamiques ajustables :** Boutons tactiles rÃ©actifs (`+` et `-`) dans les options pour ajuster instantanÃ©ment Ã  chaud la taille du texte de l'Ã©diteur Monaco et du chat.
* **Version franÃ§aise complÃ¨te :** L'interface entiÃ¨re et toutes les actions de l'application s'adaptent automatiquement en franÃ§ais ou en anglais.
* **SÃ©lection et copie libre :** DÃ©blocage de la sÃ©lection et copie de texte directement dans le fil de discussion du chat.
* **DÃ©marcation visuelle :** Ligne de sÃ©paration verticale Ã©lÃ©gante Ã  gauche du panneau de configuration des paramÃ¨tres.
* **DÃ©coupage du bundle Vite (-80% de taille) :** Monaco Editor et xterm.js sont isolÃ©s dans des sous-lots sÃ©parÃ©s pour un chargement instantanÃ©l'interface utilisateur.
* **Visualisation du plan d'action (Planning Board) :** IntÃ©gration d'un bloc dynamique interactif (`PlanningNextMoveBlock`) montrant en temps rÃ©el les prochaines Ã©tapes planifiÃ©es par le Swarm d'agents.
* **AperÃ§u d'image immersif (Lightbox) :** Visionneuse d'images de discussion immersive avec zoom Ã  la molette de souris, dÃ©placement panoramique, rotation, tÃ©lÃ©chargement et fermeture par clic extÃ©rieur.

### ðŸ’¾ Autonomie, Sauvegarde & Robustesse SystÃ¨me
* **Sauvegarde automatique (Auto-Save SOTA) :** Enregistrement automatique et transparent en arriÃ¨re-plan 1,5 seconde aprÃ¨s l'arrÃªt de la frappe. Activable ou dÃ©sactivable d'un clic dans vos options.
* **Mode Sandbox :** Lancement de l'application en un clic sans aucun projet ouvert pour tester l'IA ou utiliser les outils MCP de maniÃ¨re isolÃ©e.
* **Synchro OneDrive & SQLite automatique :** Synchronisation transparente de vos conversations, configurations de projets, jetons de connexion/clÃ©s d'authentification (`*-auth.json`, `*-device.json`, `*-stream-state.json`), fichiers d'apprentissage globaux (`errors_raw.json` et `instructions_consolidated.md`), et bases de donnÃ©es SQLite entre vos diffÃ©rents ordinateurs.
* **ZÃ©ro popup console Windows :** Lancement asynchrone et silencieux de tous les outils, serveurs MCP, commandes Git et diagnostics SOTA en arriÃ¨re-plan sans aucune ouverture de fenÃªtres d'invite de commandes.
* **PrÃ©fixe PC rÃ©el automatique :** Identification automatique du nom de la machine physique pour typer et sÃ©curiser les configurations de conversation multi-PC.
* **Diagnostic Windows OAuth rÃ©silient :** Capture robuste de l'erreur rÃ©seau typique sous Windows (code 10013) et conseils clairs pour dÃ©bloquer la connexion (WinNAT/HNS).
* **Diagnostic SOTA :** VÃ©rification en un clic de l'Ã©tat de santÃ©, du PATH et des versions de tous vos outils de dÃ©veloppement (Git, Python, Node, Cargo, etc.).
* **Ã‰cran de mises Ã  jour sÃ©curisÃ© (`UpdaterLockScreen`) :** Verrouillage de l'interface pendant l'application des correctifs systÃ¨me pour Ã©viter tout conflit de fichiers ou corruption de base de donnÃ©es.
* **Script de compilation OneDrive (`compil.ps1`) :** Automatisation de la gÃ©nÃ©ration de l'application et copie immÃ©diate sur OneDrive pour un dÃ©ploiement instantanÃ© sur vos PC.
* **Active Turn Registry :** Moteur intelligent Rust qui suit les turns de l'agent en cours et assure une reprise instantanÃ©e du streaming.
* **Fiche de transmission structurÃ©e (Compaction d'IA) :** Compactage automatique du contexte lors du changement de fournisseur d'IA dans une fiche structurÃ©e reprenant le statut des fichiers modifiÃ©s, le relais des tÃ¢ches et les diagnostics du linter.
* **Mode plein gaz adaptatif (`crates/sinew-index/src/store.rs`) :** Optimisation dynamique des performances de l'indexeur augmentant le cache et la lecture en mÃ©moire lorsque la machine dispose d'un stockage SSD/NVMe.
* **Indexation locale parallÃ¨le SOTA :** PrÃ©paration et analyse des fichiers en parallÃ¨le rÃ©partie sur tous les coeurs de CPU disponibles via Rayon, avec dÃ©tection immÃ©diate et saut des fichiers inchangÃ©s grÃ¢ce Ã  leurs empreintes de taille et date.
* **Identification de projet universelle :** Association automatique des conversations au dÃ©pÃ´t Git distant (remote origin URL) ou via un fichier d'identifiant unique `.sinew/project_id.txt` pour lier instantanÃ©ment vos conversations d'un PC Ã  un autre sans aucune action manuelle, avec dÃ©tection, liaison et rafraÃ®chissement dynamique des conversations provenant de PC alternatifs depuis les paramÃ¨tres.
* **Gestion des mises Ã  jour configurables :** Option Ã  3 choix (Bloquant, Notification, DÃ©sactivÃ©) pour dÃ©cider prÃ©cisÃ©ment du niveau de vÃ©rification des nouvelles versions de Sinew et empÃªcher l'Ã©crasement de vos modifications.
* **Script de contrÃ´le qualitÃ© unifiÃ© (`scripts/check.ps1`) :** Commande unique `npm run check` exÃ©cutant le build frontend, `cargo check`, les tests, `clippy` et les audits de dÃ©pendances en une seule opÃ©ration.
* **SystÃ¨me d'apprentissage global transparent :** Chargement et injection automatique de la base d'instructions centralisÃ©es de l'utilisateur (`%LOCALAPPDATA%\Sinew\instructions_consolidated.md`) dans le prompt systÃ¨me de tous les agents pour l'ensemble des projets ouverts sur la machine.
* **Consolidation automatique de la mÃ©moire :** MÃ©canisme au dÃ©marrage transformant automatiquement les erreurs rÃ©pÃ©tÃ©es enregistrÃ©es dans `errors_raw.json` en rÃ¨gles d'apprentissage globales permanentes dans `instructions_consolidated.md` avec nettoyage du compteur d'erreurs.
* **Bouton de synchronisation forcÃ©e :** Ajout d'un bouton de synchronisation immÃ©diate Ã  la demande dans les paramÃ¨tres pour dÃ©clencher manuellement la synchronisation bidirectionnelle OneDrive et Git.

### ðŸ¤– ModÃ¨les d'IA, Comptes & FurtivitÃ© (AI Engine)
* **Gestion Multi-comptes OpenAI & Google Gemini :** Connexion simultanÃ©e de plusieurs profils OpenAI et Google Gemini secondaires avec bascule instantanÃ©e entre vos diffÃ©rentes clÃ©s, comptes et abonnements.
* **Quotas en temps rÃ©el :** Visualisation dynamique de votre consommation (crÃ©dits / balance restante) sous forme de barres de progression colorÃ©es adaptatives dans les options, et pastille live dans le chat.
* **Routage & RÃ©silience Google Antigravity SOTA :** RÃ©paration, de-surcharge rÃ©seau (erreur 503), routeurs de secours et transition transparente entre modÃ¨les avec rÃ©solution dynamique des identifiants d'appels d'outils (tool_call_id).
* **Optimisation de vitesse Gemini :** Streaming et requÃªtes ultra-rapides pour les modÃ¨les Gemini.
* **Incorporation de Claude Opus 4.8 & 4.6 :** IntÃ©gration complÃ¨te de Claude Opus 4.8 (contexte 1M natif) et Claude Opus 4.6 via les abonnements professionnels Google.
* **SystÃ¨me Pending/Steering pour Influencer :** Un vrai systÃ¨me d'interception et de guidage pour orienter, corriger ou ajouter des instructions en temps rÃ©el sans blocage du flux de l'IA.
* **Indexation sÃ©mantique locale vectorielle :** Indexation et recherche vectorielle haute-performance effectuÃ©e localement sur votre machine avec commutateur d'activation directe (BETA) dans le panneau d'options.
* **IntÃ©gration de DeepSeek R1 & V3 :** Support complet de **DeepSeek V3** et de **DeepSeek R1** avec capture et rendu en temps rÃ©el du bloc de rÃ©flexion (*reasoning*) grÃ¢ce Ã  l'extraction du champ `reasoning_content` dans le chat.
* **Pont Cursor Composer 2.5 (agent.v1) :** Moteur haute-performance autonome sur connexions HTTP/2 persistantes gÃ©rant toutes les modifications chirurgicales de fichiers, avec installation automatique et invisible en arriÃ¨re-plan, et masquage du sÃ©lecteur d'intelligence inutile.
* **SÃ©curitÃ© & FurtivitÃ© WebSocket :** Spoofing d'empreinte rÃ©seau avancÃ© pour Ã©liminer tout risque de dÃ©tection ou de blocage sur les flux de ChatGPT.
* **WebSocket OpenAI :** Transport temps-rÃ©el haute performance basÃ© sur WebSocket pour des rÃ©ponses fluides et Ã  latence minimale avec OpenAI.

### ðŸ”Œ Extensions & Ponts locaux (MCP & Bridge)
* **Extension Chrome nouvelle gÃ©nÃ©ration :** Pilotage d'actions de navigation ultra-stables en natif Rust avec mouvements et clics Ã  vitesse humaine (mouvements Beziers, physique fluide) et mode silencieux.
* **RÃ©paration Chrome en un clic :** Bouton bleu de configuration automatique si le pont Chrome ne rÃ©pond pas sur un nouveau PC.
* **Empaquetage des ressources Tauri :** Le pont local et l'extension Chrome sont intÃ©grÃ©s directement au sein de l'installateur compilÃ© (MSI/EXE).
* **Outils Rust & ripgrep Sidecar :** IntÃ©gration de Ripgrep en binaire natif sidecar et de nouveaux outils (`list_dir`, `delete_file`) pour accÃ©lÃ©rer la recherche et la gestion des fichiers par 10x.
* **Diagnostics Monaco en temps rÃ©el :** RemontÃ©e automatique des lints et erreurs de compilation de l'Ã©diteur de code Ã  l'IA en temps rÃ©el.
* **Logs ultra-compacts :** Nettoyage automatique du contexte de discussion pour Ã©liminer le bruit et optimiser la consommation de jetons.
* **Laboratoire rÃ©seau MITM :** Outils de dÃ©bogage et d'ingÃ©nierie inverse intÃ©grÃ©s pour inspecter le trafic chiffrÃ©s des outils IA.
* **Moteur de remplacement intelligent (Search/Replace) :** SystÃ¨me d'auto-correction Ã  8 couches (Unicode, indentations, etc.) garantissant que les modifications de l'IA s'insÃ¨rent correctement dans vos fichiers mÃªme en cas de lÃ©gÃ¨res erreurs d'espaces.
* **Outils MCP de diagnostics Chrome avancÃ©s :** IntÃ©gration de nouveaux outils d'audit (`emulate_experience`, `lighthouse_audit`, `analyze_memory_leaks`) basÃ©s sur l'API CDP pour tester les performances, diagnostics Lighthouse et fuites mÃ©moire en local.

---


