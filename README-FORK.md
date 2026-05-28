# Sinew — Fork de julienpiron.fr

Cette version a été optimisée en profondeur pour offrir une expérience utilisateur haut de gamme (SOTA), une autonomie maximale en arrière-plan, et des intégrations d'intelligence artificielle inégalées.

---

## 🖱️ Interface & Confort Visuel (Premium UI)

* **🖱️ Menu clic droit sur les onglets de l'éditeur (Context Menu)**
  * **Bénéfice :** Un confort de navigation indispensable pour fermer les onglets (l'onglet actif, les autres ou tous les onglets à sa droite) en un clic.
  * **Ergonomie :** Intègre la copie rapide du chemin complet/relatif et l'accès direct au fichier dans l'explorateur système (Finder/Explorer). Entièrement traduit en français ou en anglais selon vos préférences.
  * 📂 *Fichiers : `src/components/Workspace.tsx`, `src/components/EditorPane.tsx`, `src/styles.css`*
* **🔎 Sélecteurs tactiles de taille de police (Éditeur & Chat)**
  * **Bénéfice :** Ajustez le confort visuel de votre plan de travail. Deux sélecteurs (`+` et `-`) font varier précisément la taille des polices de l'éditeur de code Monaco et du chat de l'IA (de 10px à 22px).
  * **Performance :** L'ajustement s'effectue à chaud sans aucune latence via l'injection de variables CSS documentaires appliquées dès le démarrage.
  * 📂 *Fichiers : `src/components/SettingsPane.tsx`, `src/components/EditorPane.tsx`, `src/App.tsx`, `src/styles.css`*
* **🧭 Affichage visuel des plans d'action de l'IA (Planning Board)**
  * **Bénéfice :** Transparence totale. Un bloc visuel dynamique et interactif (`PlanningNextMoveBlock.tsx`) affiche en temps réel les prochaines étapes planifiées par le Swarm IA dans le fil de discussion.
  * 📂 *Fichiers : `src/components/chat/PlanningNextMoveBlock.tsx`*
* **🔄 Guidage dynamique dans le chat & Bouton « Influencer »**
  * **Bénéfice :** Un bouton intelligent « Influencer » permet d'injecter des instructions en plein vol sans interrompre le cycle de l'agent.
  * 📂 *Fichiers : `src/components/chat/ChatPane.tsx`*

---

## 💾 Autonomie, Sauvegarde & Robustesse (SOTA Save & Sync)

* **💾 Sauvegarde automatique intelligente (Auto-Save SOTA)**
  * **Bénéfice :** Plus besoin de faire `Ctrl+S`. Vos fichiers modifiés s'enregistrent silencieusement en arrière-plan 1,5 seconde après l'arrêt de la frappe. Activable ou désactivable à la volée dans vos options.
  * 📂 *Fichiers : `src/components/SettingsPane.tsx`, `src/components/EditorPane.tsx`*
* **☁️ Synchronisation OneDrive Multi-PC Automatique**
  * **Bénéfice :** Vos conversations et configurations de projets sont automatiquement et silencieusement synchronisées en tâche de fond via OneDrive. Passez d'un ordinateur à un autre sans aucune perte.
  * 📂 *Fichiers : `src-tauri/src/conversations.rs`, `src-tauri/src/lib.rs`*
* **🔑 Diagnostic Windows OAuth résistant aux conflits de ports**
  * **Bénéfice :** Fini les échecs de connexion mystérieux. En cas d'erreur de port réseau (code 10013) typique sous Windows, l'IA capture l'exception et vous conseille instantanément de vérifier les plages d'adresses ou de redémarrer WinNAT/HNS.
  * 📂 *Fichiers : `src-tauri/src/providers.rs`*
* **🚀 Script de compilation intelligente (`compil.ps1`)**
  * **Bénéfice :** Déploiement cloud immédiat. Recherche et copie automatique du fichier `.exe` généré par Tauri directement sur le bureau OneDrive pour une mise à jour instantanée.
  * 📂 *Fichiers : `scripts/compil.ps1`*
* **🔒 Écran de verrouillage de sécurité lors des mises à jour (`UpdaterLockScreen`)**
  * **Bénéfice :** Empêche toute corruption de l'historique SQLite en bloquant proprement l'interface pendant l'application des correctifs système.
  * 📂 *Fichiers : `src/components/UpdaterLockScreen.tsx`*

---

## 🤖 Intégrations d'IA Avancées & Furtivité (AI Engine)

* **🤖 Pont standalone Cursor Composer 2.5 via le protocole `agent.v1`**
  * **Bénéfice :** Remplacement complet de l'ancien protocole `IdempotentSSE` par un serveur-pont Node.js autonome (`agent-bridge`) gérant le streaming Protobuf/gRPC sur des connexions HTTP/2 persistantes vers les serveurs de Cursor.
  * **Outils supportés :** Lecture de fichiers (`read`), listing de dossiers (`list_dir`), création (`write_file`), suppression (`delete_file`) et édition chirurgicale par bloc (`edit/replace`).
  * 📂 *Fichiers : `scripts/agent-bridge/`, `crates/sinew-cursor/src/agent/`, `scripts/prepare-agent-bridge.mjs`*
* **🧠 Réflexion en continu & Visualisation de la pensée pour DeepSeek R1**
  * **Bénéfice :** Support complet des modèles phares **DeepSeek V3** (tâches rapides avec outils) et **DeepSeek R1** (visualisation en temps réel de la réflexion interne dans le streaming du chat).
  * 📂 *Fichiers : `crates/sinew-deepseek/`, `src-tauri/src/providers.rs`, `src/components/SettingsPane.tsx`*
* **🏢 Badge d'espace de travail ChatGPT Team/Enterprise**
  * **Bénéfice :** Affiche le nom réel de votre espace de travail d'entreprise (Team / Enterprise) directement dans les paramètres pour les comptes OpenAI connectés.
  * 📂 *Fichiers : `src/components/SettingsPane.tsx`, `src/lib/ipc.ts`*
* **🍌 Abonnement Gemini (Google OAuth) sans clé API**
  * **Bénéfice :** Authentification simplifiée avec votre compte Google connecté pour utiliser l'outil de génération d'images, sans aucune clé API externe requise.
  * 📂 *Fichiers : `crates/sinew-app/src/image.rs`, `src/components/SettingsPane.tsx`*
* **🎨 Menu déroulant de sélection des modèles d'images phares**
  * Passez librement entre les meilleurs générateurs d'images du marché (`gpt-image-2`, `gpt-image-1.5`, `dall-e-3` pour OpenAI, et `gemini-3.1-flash-image-preview`, `gemini-2.5-flash-image` pour Gemini).
  * 📂 *Fichiers : `crates/sinew-app/src/store.rs`, `src/components/SettingsPane.tsx`*
* **🛡️ Sécurité avancée & Spoofing User-Agent pour les WebSocket OpenAI**
  * Spoofing d'empreinte sous l'en-tête `"user-agent": "codex-cli"` sur les flux WebSocket persistants de ChatGPT Codex pour éliminer tout risque de détection.
  * 📂 *Fichiers : `crates/sinew-openai/src/websocket.rs`*

---

## 🔌 Extensions, Outils & MCP (MCP & Chrome Bridge)

* **🌐 Réparation automatique en un clic du pont Chrome**
  * **Bénéfice :** Si le pont Chrome ne répond pas sur un nouveau PC synchronisé, un avertissement clair et un bouton bleu de réparation automatique s'affichent dans les paramètres MCP.
  * 📂 *Fichiers : `src-tauri/src/conversations.rs`, `src-tauri/src/lib.rs`, `src/components/SettingsPane.tsx`*
* **📦 Empaquetage natif des ressources (Tauri Resources)**
  * **Bénéfice :** L'extension et le pont local (`sinew-chrome-bridge`) sont désormais déclarés comme ressources d'empaquetage officielles et intégrés directement au sein de l'installateur compilé (MSI/EXE).
  * 📂 *Fichiers : `src-tauri/tauri.conf.json`, `src-tauri/src/conversations.rs`*
* **🛠️ Nouveaux outils natifs d'analyse pour l'agent Rust (`list_dir` et `delete_file`)**
  * Outils haute-performance en Rust pour lister les dossiers complexes (`list_dir`) et supprimer des fichiers temporaires (`delete_file`) de manière optimisée pour le workspace.
  * 📂 *Fichiers : `crates/sinew-app/src/list_dir.rs`, `crates/sinew-app/src/delete_file.rs`*
* **🔍 Suite de tests locaux & Analyse MITM (Laboratoire réseau)**
  * Ensemble de scripts d'ingénierie inverse dans `scripts/mitm/` facilitant l'interception et le débogage en temps réel du trafic chiffré des outils IA.
  * 📂 *Fichiers : `scripts/mitm/`, `scripts/verify_all.py`*
* **📦 Distribution d'installateurs pré-compilés à la racine (`build/`)**
  * Bundles d'installation prêts à l'emploi (NSIS `.exe` et `.msi`) inclus directement à la racine pour un déploiement et un test immédiats.
  * 📂 *Fichiers : `build/`*
