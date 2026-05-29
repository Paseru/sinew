# Changelog

All notable changes to this project will be documented in this file.

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
