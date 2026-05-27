# 🛠️ Mon Fork de Sinew — Fonctionnalités & Améliorations

Ce document liste les fonctionnalités développées pour mon usage quotidien sur ce fork personnel de **Sinew**. Elles sont présentées de manière claire et modulaire, prêtes à être récupérées par l'auteur original s'il le souhaite !

---

## 🚀 1. Démarrage & Sessions

* **📦 Mode Sandbox (Sans dossier)** : Lancez Sinew en un clic sans ouvrir de projet. Idéal pour tester l'IA ou utiliser les outils MCP de manière isolée.
  * 📂 *Fichiers : `src/components/Welcome.tsx`, `src-tauri/src/workspace.rs`*

---

## 💬 2. Interface de Chat & Expérience Utilisateur

* **⚡ Bouton « Influencer »** : Un badge distinctif `[ Influencer ]` pour injecter instantanément un prompt mis en attente et guider le flux de l'IA.
  * 📂 *Fichiers : `src/components/chat/TodoStrip.tsx`*
* **🚀 Exécution directe depuis le Chat** : Clic droit sur n'importe quel chemin de fichier cité par l'IA pour l'ouvrir dans l'éditeur, le révéler dans l'explorateur ou l'exécuter directement.
  * 📂 *Fichiers : `src/components/chat/Markdown.tsx`*
* **📌 Question Collante (Sticky)** : La dernière question posée reste fixée en haut lors du défilement. Cliquez dessus pour y remonter instantanément.
  * 📂 *Fichiers : `src/components/chat/ChatPane.tsx`*
* **🟢 Pastille de Quota dans le Chat** : Un indicateur coloré dynamique s'affiche à côté du sélecteur de modèle dans le chat composer pour refléter votre quota en temps réel. Un clic sur la pastille ouvre directement la page de configuration du fournisseur. Le popover de sélection affiche également des pastilles de couleur individuelles pour chaque modèle.
  * 📂 *Fichiers : `src/components/chat/ChatPane.tsx`*
* **📋 Copie libre** : La sélection de texte est entièrement débloquée dans les bulles de chat pour copier facilement n'importe quel extrait.
  * 📂 *Fichiers : `src/styles.css`*

---

## ⚙️ 3. Options & Confort (`Settings > Options`)

* **🧠 Affichage à 3 niveaux (Réflexion Compacte)** : Choisissez la densité technique du chat :
  * *Détaillé* : Visibilité maximale.
  * *Compact* : La réflexion de l'IA se replie automatiquement une fois terminée.
  * *Très compact* : La réflexion disparaît complètement pour un chat ultra-propre.
  * 📂 *Fichiers : `src/components/chat/AIThinkingBlock.tsx`, `src/lib/ipc.ts`*
* **🤖 Mode "Power User"** : L'IA répond de manière ultra-concise (zéro jargon) et gère automatiquement toute la maintenance Git (commit, pull, push) en arrière-plan.
  * 📂 *Fichiers : `src-tauri/src/state.rs`*
* **🌐 Version Française** : Traduction dynamique des panneaux de configurations et boutons clés pour un meilleur confort visuel.
  * 📂 *Fichiers : `src/lib/locale.ts`*
* **🔄 Synchro OneDrive & SQLite** : Fusion différentielle automatique de votre historique et préférences entre plusieurs PC avec gestion intelligente des suppressions.
  * 📂 *Fichiers : `src-tauri/src/lib.rs`*
* **⚡ Diagnostic SOTA** : Un panneau de contrôle en un clic pour tester en temps réel l'état et la version de vos outils (Git, Node, Rust, Python, Ripgrep).
  * 📂 *Fichiers : `crates/sinew-app/src/check_sota.rs`*

---

## 🔌 4. Connecteurs & Intégrations (`Settings > Providers & MCP`)

* **🌐 Sinew Chrome Bridge ultra-stable** : Pilotez Google Chrome via un pont MCP natif en Rust (`native-host-wrapper.exe`). Gère automatiquement les conflits de ports et utilise des clics souris "humains" (CDP) pour contourner les protections anti-robots.
  * 📂 *Fichiers : `sinew-chrome-bridge/mcp_server.js`*
* **👥 Multi-comptes OpenAI** : Enregistrez plusieurs clés API distinctes via le bouton `[ + ]` et basculez instantanément de l'une à l'autre.
  * 📂 *Fichiers : `src/components/SettingsPane.tsx`*
* **📊 API & Suivi en temps réel des Quotas** : Ajout de fonctions et d'interfaces pour lire les quotas réels et les limites actives des abonnements :
  * `get_antigravity_quota` (Google OAuth / Cloud Code) : interroge l'API `fetchAvailableModels` pour mapper les fenêtres de quotas de Gemini.
  * `get_openai_codex_rate_limits` (OpenAI OAuth / ChatGPT) : interroge l'API `/wham/usage` (et son fallback `/backend-api/wham/usage` pour les comptes Business) afin de suivre les fenêtres d'utilisation courte/longue.
  * `getOpenRouterKeyDetails` (OpenRouter API) : interroge `/auth/key` pour extraire la limite de crédit et le solde restant en USD.
  * *Rendu visuel* : Barres de progression dynamique à couleur adaptative (Vert >80%, Bleu >50%, Rose >20%, Rouge <20%).
  * 📂 *Fichiers : `src/lib/quotas.ts`, `src-tauri/src/providers.rs`, `src/components/SettingsPane.tsx`*
