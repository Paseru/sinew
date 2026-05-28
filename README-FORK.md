# 🛠️ Mon Fork de Sinew — Fonctionnalités & Améliorations

Ce document répertorie toutes les améliorations majeures développées sur mon fork personnel de **Sinew**. Conçu pour optimiser mon workflow quotidien, il propose des fonctionnalités exclusives, prêtes à être récupérées par l'auteur original s'il le souhaite !

---

## 🚀 1. Démarrage & Sessions

* **📦 Mode Sandbox : lancez Sinew en un clic sans projet**
  * Lancement instantané pour tester l'IA ou utiliser les outils MCP de manière isolée sans ouvrir de dossier de travail.
  * 📂 *Fichiers : `src/components/Welcome.tsx`, `src-tauri/src/workspace.rs`*

---

## 💬 2. Interface de Chat & Expérience Utilisateur

* **🚀 Clic droit sur n'importe quel fichier : ouvrir, révéler ou exécuter direct**
  * Ouvrez instantanément un fichier cité par l'IA dans l'éditeur, affichez-le dans l'explorateur ou exécutez-le directement via son application par défaut d'un simple clic droit dans le chat.
  * 📂 *Fichiers : `src/components/chat/Markdown.tsx`*
* **📌 Question collante : reste fixée en haut**
  * La dernière question posée reste visible au sommet de l'écran lors du défilement. Un clic dessus vous y ramène instantanément.
  * 📂 *Fichiers : `src/components/chat/ChatPane.tsx`*
* **📋 Copie libre : texte débloqué dans le chat**
  * La sélection et le copier-coller de texte sont entièrement déverrouillés dans toutes les bulles de discussion.
  * 📂 *Fichiers : `src/styles.css`*
* **⚡ Bouton « Influencer »** : Un badge distinctif `[ Influencer ]` pour injecter instantanément un prompt mis en attente et guider le flux de l'IA.
  * 📂 *Fichiers : `src/components/chat/TodoStrip.tsx`*

---

## ⚙️ 3. Options & Confort (`Settings > Options`)

* **🧠 3 niveaux de réflexion : Détaillé / Compact / Très compact**
  * Ajustez la densité technique du chat : complet (détaillé), replié une fois terminé (compact), ou totalement invisible (très compact).
  * 📂 *Fichiers : `src/components/chat/AIThinkingBlock.tsx`, `src/lib/ipc.ts`*
* **🤖 Mode Power User : réponses ultra-concises + Git auto (commit/pull/push)**
  * L'IA répond de manière directe (zéro jargon) et prend en charge en tâche de fond toute la maintenance Git après chaque modification.
  * 📂 *Fichiers : `src-tauri/src/state.rs`*
* **🌐 Version française complète + synchro OneDrive & SQLite automatique**
  * Traduction dynamique et progressive de toute l'interface. Sauvegarde et fusion différentielle en temps réel de votre historique et préférences entre tous vos PC, avec gestion intelligente des suppressions.
  * 📂 *Fichiers : `src-tauri/src/lib.rs`, `src-tauri/src/conversations.rs`, `src/lib/locale.ts`, `src/lib/frRuntime.ts`*
* **⚡ Diagnostic SOTA : état de tous tes outils en un clic**
  * Un tableau de contrôle en temps réel pour vérifier instantanément l'état et la version de vos outils (Git, Node, Rust, Python, Ripgrep).
  * 📂 *Fichiers : `crates/sinew-app/src/check_sota.rs`*

---

## 🔌 4. Connecteurs & Intégrations (`Settings > Providers & MCP`)

* **🌐 Chrome Bridge ultra-stable : pilotage natif Rust avec clics humains**
  * Pilotez Google Chrome via un pont MCP natif Rust contournant les blocages grâce à des clics souris "humains" (CDP) et une gestion automatique des conflits de ports.
  * 📂 *Fichiers : `sinew-chrome-bridge/mcp_server.js`*
* **👥 Multi-comptes : bascule instantanée entre clés**
  * Connectez plusieurs comptes OpenAI et Google (Gemini) en parallèle et passez d'un profil à l'autre en un instant dans le sélecteur de modèles.
  * 📂 *Fichiers : `src/components/SettingsPane.tsx`, `src-tauri/src/providers.rs`*
* **📊 Quotas en temps réel : barres + pastille live dans le chat**
  * Suivi de vos limites réelles d'utilisation (Gemini, ChatGPT Codex, OpenRouter) via des barres colorées réactives dans les options et un témoin dynamique dans le champ de saisie du chat.
  * 📂 *Fichiers : `src/lib/quotas.ts`, `src-tauri/src/providers.rs`*
* **🤖 Routage & Résilience Google Antigravity SOTA : réparation & optimisation des modèles Google**
  * Fiabilité ultime : connexion aux quotas Gemini AI Ultra, déclaration d'outils réparée, header haute priorité anti-surcharge 503 et bascules serveurs de secours automatiques.
  * 📂 *Fichiers : `crates/sinew-google/src/client.rs`, `src/lib/models.ts`*

---

## 📅 27/05 — Guidage dynamique & Ajustements Google Antigravity

* **🧭 Guidage dynamique Pending/Steering : bouton Influencer intelligent sans blocage du flux**
  * Les consignes d'orientation ne forcent plus d'arrêt immédiat : elles sont mises en file d'attente (badge `Pending`) et injectées aux points de contrôle logiques du moteur.
  * 📂 *Fichiers : `crates/sinew-app/src/agent/cancel.rs`, `crates/sinew-app/src/agent/turn.rs`, `src-tauri/src/turns.rs`*
* **🤖 Intégration Cursor & Composer 2.5 : OAuth sécurisé, mimétisme d'IDE et anonymisation (Stealth)**
  * Connexion via OAuth sécurisée et renouvellement de session automatique. Simule les en-têtes officiels de Cursor et intègre un filtre furtif anonymisant (`sanitize.rs`) pour déjouer le brand fingerprinting sur les serveurs Cursor.
  * 📂 *Fichiers : `crates/sinew-cursor/`, `crates/sinew-cursor/src/sanitize.rs`, `src-tauri/src/providers.rs`*
* **🔍 Indexation sémantique locale : recherche vectorielle avec badge d'état interactif dans la barre latérale**
  * Indexation vectorielle autonome en arrière-plan avec symbol-aware chunking. Les résultats sont injectés directement comme contexte, et l'état s'affiche en temps réel sous le nom du projet.
  * 📂 *Fichiers : `crates/sinew-index/`, `crates/sinew-app/src/codebase_search.rs`, `src/components/Workspace.tsx`*
* **🌐 Extension Chrome de pointe : zéro timeout, courbes physiques de Bézier et diagnostic premium**
  * Résolution des blocages de navigation, déplacements souris simulés par courbes physiques de Bézier, masquage de la barre d'avertissement de débogage et menu d'exécution clic-droit depuis le FileTree et le chat.
  * 📂 *Fichiers : `sinew-chrome-bridge/mcp_server.js`, `src/components/FileTree.tsx`, `src/components/chat/Markdown.tsx`*
* **🛠️ Diagnostics Monaco temps réel : remontée d'erreurs en tâche de fond pour auto-correction par l'IA**
  * Remontée immédiate des erreurs de compilation de l'éditeur Monaco vers le moteur Rust. L'IA utilise l'outil `read_lints` pour détecter les erreurs de syntaxe de manière autonome.
  * 📂 *Fichiers : `src/components/EditorPane.tsx`, `crates/sinew-app/src/read_lints.rs`*
* **🧠 Compaction avancée des logs : repli automatique des détails et diffs d'outils réussis en mode compact**
  * En mode compact/très compact, cache les diffs d'écriture, arguments de lecture et logs bash d'outils réussis pour garder un chat lisible (ils s'ouvrent uniquement en cas d'erreur).
  * 📂 *Fichiers : `src/components/chat/ToolCard.tsx`, `src/components/chat/stream.ts`*
* **⚡ Zéro popup de console Windows : commandes Git, MCP et SOTA lancées silencieusement sans flash noir**
  * Lancement transparent des serveurs Node/Python, des commandes Git et de l'analyse SOTA en tâche de fond sans aucun clignotement de fenêtres d'invite de commandes.
  * 📂 *Fichiers : `crates/sinew-app/src/bash.rs`, `src-tauri/src/platform.rs`, `src-tauri/src/git.rs`*
* **🎨 Animation de démarrage : splash screen instantané anti-flash blanc et logo s'ouvrant de façon fluide**
  * Suppression totale du flash blanc au démarrage via un Splash Screen statique (dans `index.html`), complété par des animations d'apparition fluides sur l'écran d'accueil (`Welcome.tsx`).
  * 📂 *Fichiers : `index.html`, `src/components/Welcome.tsx`, `src/styles.css`*
* **🏷️ Préfixe de PC réel : étiquetage automatique des conversations avec le nom d'hôte de la machine**
  * Préfixe automatiquement les nouvelles conversations avec le nom réel du PC actif (`%COMPUTERNAME%` / `$HOSTNAME`) pour s'y retrouver instantanément lors de la synchro multi-PC.
  * 📂 *Fichiers : `crates/sinew-app/src/store.rs`*

---

## 📅 28/05 — Abonnement Gemini et Sélection Dynamique de Modèles d'Images

* **🍌 Abonnement Gemini (Google OAuth) sans clé API dans l'outil d'images**
  * Ajout de l'interrupteur toggle « Utiliser l'abonnement Gemini » (symétrique à OpenAI) pour s'authentifier directement avec ton compte Google connecté, sans clé API.
  * 📂 *Fichiers : `crates/sinew-app/src/image.rs`, `src/components/SettingsPane.tsx`, `src/types.ts`*
* **🎨 Menu déroulant de sélection des 3 derniers modèles d'images phares**
  * Intégration d'un sélecteur de modèles d'images complet : passez librement entre `gpt-image-2`, `gpt-image-1.5`, `dall-e-3` pour OpenAI, et `gemini-3.1-flash-image-preview`, `gemini-3-pro-image-preview`, `gemini-2.5-flash-image` pour Gemini.
  * 📂 *Fichiers : `crates/sinew-app/src/store.rs`, `crates/sinew-app/src/image.rs`, `src/components/SettingsPane.tsx`*
* **⚙️ Intégration de sécurité & Synchro automatique**
  * Réinitialisation de l'abonnement d'images si le fournisseur Google est déconnecté, et sauvegarde immédiate des préférences dans le profil utilisateur.
  * 📂 *Fichiers : `src-tauri/src/providers.rs`, `src/components/SettingsPane.tsx`*

---

## 📅 29/05 — Fournisseur DeepSeek V3 & R1 (Reasoner) complet avec clés API personnelles

* **🚀 Intégration native de DeepSeek dans l'application**
  * Ajout du fournisseur officiel **DeepSeek** dans le menu des modèles et de configuration.
  * Prise en charge complète de la validation de clé API sécurisée via le fichier local `deepseek-auth.json`.
  * Support de **DeepSeek V3** (`deepseek-chat`) pour des tâches générales rapides avec support natif des outils (tools).
  * 📂 *Fichiers : `crates/sinew-deepseek/`, `src-tauri/Cargo.toml`, `src-tauri/src/providers.rs`, `src-tauri/src/lib.rs`*
* **🧠 Réflexion en continu & Visualisation de la pensée pour DeepSeek R1**
  * Support complet du modèle phare **DeepSeek R1** (`deepseek-reasoner`).
  * Capture et rendu en temps réel du bloc de réflexion (*reasoning*) grâce à l'extraction et l'affichage fluide du champ `reasoning_content` dans le streaming de chat.
  * Désactivation sécurisée des outils sur le modèle R1 car l'API officielle de DeepSeek ne supporte pas encore les appels d'outils sur ce modèle.
  * 📂 *Fichiers : `crates/sinew-deepseek/src/stream.rs`, `crates/sinew-deepseek/src/wire.rs`, `src/lib/models.ts`*
* **⚙️ Interface utilisateur dédiée dans l'onglet des fournisseurs**
  * Intégration d'une carte de connexion élégante avec masquage de clé, validation immédiate au collage et indicateur d'état dynamique (*Connected*, *Connecting*, *Needs attention*, *Disconnected*).
  * Affichage en temps réel du **montant restant (crédits / balance active en USD/CNY)** et de la barre de progression par rapport au montant total rechargé (*Topped-up balance*).
  * 📂 *Fichiers : `src/components/SettingsPane.tsx`, `src/lib/ipc.ts`, `src/types.ts`, `src/lib/quotas.ts`, `src-tauri/src/providers.rs`*

---

## 📅 30/05 — Synchronisation Multi-PC Intelligente & Réparation du Pont Chrome en Un Clic

* **🌐 Bouton « Configurer / Réparer le pont local » en un clic dans l'interface**
  * Ajout d'une fonctionnalité de réparation transparente en un clic : si le pont Chrome ne répond pas sur un nouveau PC synchronisé par OneDrive, un avertissement clair et un bouton bleu de réparation automatique s'affichent dans les paramètres MCP.
  * L'activation exécute de manière invisible le script d'enregistrement système Windows en tâche de fond pour copier les dépendances locales, ajouter les clés de registre Windows nécessaires et recharger les serveurs MCP.
  * 📂 *Fichiers : `src-tauri/src/conversations.rs`, `src-tauri/src/lib.rs`, `src/lib/ipc.ts`, `src/components/SettingsPane.tsx`*
* **📦 Empaquetage natif des ressources (Tauri Resources) & Résolution intelligente par priorité**
  * L'extension et le pont local (`sinew-chrome-bridge`) sont désormais déclarés en tant que **ressources d'empaquetage officielles dans Tauri**. Ils sont physiquement intégrés au sein de l'installateur compilé (MSI/EXE) et distribués à tous les utilisateurs.
  * Le moteur Rust de Sinew utilise une **résolution intelligente par priorité** : il recherche d'abord le script d'installation dans le dossier des ressources internes de l'application installée (`app_handle.path().resource_dir()`), puis bascule automatiquement vers le dossier de développement local de votre espace de travail en cas d'absence.
  * 📂 *Fichiers : `src-tauri/tauri.conf.json`, `src-tauri/src/conversations.rs`*
* **🧠 Résolution des importations et fiabilité des builds**
  * Correction d'une visibilité d'import sur `validate_api_key` dans `sinew-deepseek` pour assurer un build de production parfait.
  * 📂 *Fichiers : `crates/sinew-deepseek/src/lib.rs`*

