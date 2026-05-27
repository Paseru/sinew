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
* **🌐 Version Française** : Traduction dynamique et progressive des panneaux de configurations, boutons et éléments clés pour un meilleur confort visuel.
  * 📂 *Fichiers : `src/lib/locale.ts`, `src/lib/frRuntime.ts`*
* **🔄 Synchro OneDrive & SQLite** : Fusion différentielle automatique de votre historique et préférences entre plusieurs PC avec gestion intelligente des suppressions. La synchronisation de l'historique et des suppressions (tombstones) s'effectue désormais en temps réel et en arrière-plan dès qu'une action de modification (création, renommage, suppression) a lieu (plus besoin d'attendre la fermeture de l'application).
  * 📂 *Fichiers : `src-tauri/src/lib.rs`, `src-tauri/src/conversations.rs`*
* **⚡ Diagnostic SOTA** : Un panneau de contrôle en un clic pour tester en temps réel l'état et la version de vos outils (Git, Node, Rust, Python, Ripgrep).
  * 📂 *Fichiers : `crates/sinew-app/src/check_sota.rs`*

---

## 🔌 4. Connecteurs & Intégrations (`Settings > Providers & MCP`)

* **🌐 Sinew Chrome Bridge ultra-stable** : Pilotez Google Chrome via un pont MCP natif en Rust (`native-host-wrapper.exe`). Gère automatiquement les conflits de ports et utilise des clics souris "humains" (CDP) pour contourner les protections anti-robots.
  * 📂 *Fichiers : `sinew-chrome-bridge/mcp_server.js`*
* **👥 Multi-comptes OpenAI & Google** : Enregistrez plusieurs clés API OpenAI distinctes, ou connectez plusieurs comptes Google (Gemini) en parallèle via le bouton `[ + ]`. Basculez instantanément de l'un à l'autre dans le sélecteur de modèles.
  * 📂 *Fichiers : `src/components/SettingsPane.tsx`, `src-tauri/src/providers.rs`, `src-tauri/src/lib.rs`, `crates/sinew-google/src/auth.rs`, `crates/sinew-google/src/client.rs`, `src/lib/models.ts`*
* **📊 Suivi en temps réel des Quotas** : Ajout de fonctions natives (`get_antigravity_quota` pour Gemini, `get_openai_codex_rate_limits` pour ChatGPT Plus/Pro, et `getOpenRouterKeyDetails` pour OpenRouter) pour suivre vos limites réelles d'utilisation. Les quotas s'affichent sous forme de barres colorées adaptatives (Vert/Bleu/Rose/Rouge) dans les options, et via une pastille de statut dynamique directement intégrée dans le champ de saisie du chat. Seuls les 8 modèles officiels de votre abonnement Antigravity actif sont conservés et affichés dans les quotas.
  * 📂 *Fichiers : `src/lib/quotas.ts`, `src-tauri/src/providers.rs`, `src/components/SettingsPane.tsx`, `src/components/chat/ChatPane.tsx`*
* **🤖 Routage & Résilience Google Antigravity (SOTA)** : Amélioration majeure du connecteur Google Antigravity pour une robustesse optimale de vos modèles d'abonnement :
  * *Branchement dynamique des quotas* : Liaison exacte du niveau d'effort sélectionné en UI (Low, Medium, High) avec les barres de quota correspondantes du compte Google AI Ultra.
  * *Identifiants API & Outils corrigés* : Câblage technique des modèles réels (`claude-opus-4-6-thinking`, `claude-sonnet-4-6`, `gpt-oss-120b-medium`) évitant les erreurs 404. Désactivation spécifique de la déclaration des outils MCP pour `gpt-oss-120b` (résolvant l'erreur 500).
  * *Haute Priorité & Résilience 503* : Injection de l'en-tête officiel `x-goog-api-client` (simulant l'extension native) pour intégrer la file d'attente à haute priorité et bascule automatique asynchrone sur les serveurs de secours (`sandbox`/`autopush`) si le serveur principal signale une surcharge (erreur 503).
  * *Bypass de Signature* : Ajout automatique du jeton de contournement `skip_thought_signature_validator` pour éviter tout rejet lié aux signatures de réflexion historiques manquantes lors du changement de modèle.
  * *Correction d'affichage du projet* : Suppression de l'affichage en double de l'ID du projet Google (anglais et français) sous forme de badges.
  * 📂 *Fichiers : `crates/sinew-google/src/client.rs`, `crates/sinew-google/src/model_info.rs`, `src/lib/models.ts`, `src/components/SettingsPane.tsx`*

---

## 📅 27/05 — Guidage dynamique & Ajustements Google Antigravity

* **🧭 Guidage dynamique Pending/Steering** : Le bouton « Influencer » ne force plus un arrêt immédiat par défaut. La consigne est envoyée en arrière-plan (badge `Pending`) et intégrée aux points de contrôle logiques du moteur. Gère l'arrêt réseau propre lors des retry/503.
  * 📂 *Fichiers : `crates/sinew-app/src/agent/cancel.rs`, `crates/sinew-app/src/agent/turn.rs`, `src-tauri/src/turns.rs`, `src/components/chat/ChatPane.tsx`, `src/components/chat/TodoStrip.tsx`, `src/components/chat/stream.ts`*
* **🤖 Intégration de Cursor & Agent Composer 2.5** : Connexion via OAuth sécurisée (remplace l'extraction instable de `state.vscdb`) avec mimétisme d'IDE (checksum/client version). Incorpore un filtre furtif anonymisant (`sanitize.rs`) qui remplace les mentions 'Sinew'/'Hyrak' par 'Cursor' pour déjouer le brand fingerprinting. Permet de purger les projets récents de l'accueil.
  * 📂 *Fichiers : `crates/sinew-cursor/`, `crates/sinew-cursor/src/sanitize.rs`, `src-tauri/src/providers.rs`, `src/components/Welcome.tsx`, `src/lib/recents.ts`*
* **🔍 Indexation Sémantique Locale & Codebase Search** : Module `sinew-index` pour générer en arrière-plan des embeddings vectoriels locaux. L'IA injecte ces résultats de recherche sémantique comme contexte. Affiche l'état d'indexation (badge) sous le nom du projet dans la barre latérale.
  * 📂 *Fichiers : `crates/sinew-index/`, `crates/sinew-app/src/codebase_search.rs`, `src/components/Workspace.tsx`*
* **🌐 Extension Chrome (Sinew Chrome Bridge) de Pointe** : Élimination du timeout de 20s en navigation. Utilise des courbes physiques de Bézier pour simuler des mouvements humains, masque la barre d'avertissement de débogage et ajoute un menu de diagnostic. Menu d'exécution direct clic-droit depuis le chat et l'arbre des fichiers.
  * 📂 *Fichiers : `sinew-chrome-bridge/mcp_server.js`, `sinew-chrome-bridge/popup.js`, `sinew-chrome-bridge/background.js`, `.sinew/skills/browser/SKILL.md`, `src/components/FileTree.tsx`, `src/components/chat/Markdown.tsx`*
* **🛠️ Diagnostics Monaco & read_lints en Temps Réel** : Remontée instantanée des erreurs de compilation de l'éditeur Monaco. L'outil `read_lints` permet à l'IA d'interroger directement ces alertes (et linters `cargo`, `eslint`, `ruff`) pour se corriger.
  * 📂 *Fichiers : `src/components/EditorPane.tsx`, `crates/sinew-app/src/read_lints.rs`, `crates/sinew-app/src/editor_diagnostics.rs`*
* **🧠 Compaction Avancée des Tools & Logs** : En modes `Compact` ou `Très compact`, les cartes d'outils réussis (lecture, écriture, bash, todo) replient automatiquement leurs diffs, arguments et détails techniques complexes pour préserver la clarté du chat.
  * 📂 *Fichiers : `src/components/chat/ToolCard.tsx`, `src/components/chat/PlanningNextMoveBlock.tsx`, `src/components/chat/stream.ts`*
* **⚡ Suppression des Popups de Console sur Windows** : Masquage total des clignotements d'invites de commandes Windows (`cmd.exe`/`powershell.exe`) lors du démarrage des serveurs MCP (Node/Python), des commandes Git ou de l'analyse globale SOTA.
  * 📂 *Fichiers : `crates/sinew-app/src/bash.rs`, `src-tauri/src/platform.rs`, `src-tauri/src/git.rs`, `crates/sinew-app/src/check_sota.rs`*
* **🤖 Ajustements Google Antigravity & Quotas** : Mappage des modèles réels (Opus/Sonnet/Gemini/GPT-OSS), simulation de l'environnement client réel (OS & Arch) sur les en-têtes User-Agent, et robustesse accrue avec fallbacks asynchrones en cas d'erreur ou surcharge 503.
  * 📂 *Fichiers : `crates/sinew-google/src/client.rs`, `src-tauri/src/providers.rs`*
* **⚡ Expérience Gemini façon Antigravity** : Fluidification extrême de Gemini 3.5 Flash en activant HTTP/2 côté Rust et en désactivant le linter de coloration syntaxique auto en cours de streaming.
  * 📂 *Fichiers : `Cargo.toml`, `src/components/chat/Markdown.tsx`*
* **🛡️ Sécurisation & Spoofing ChatGPT Codex** : Spoofing complet de toutes les requêtes (WebSockets, SSE, et images DALL-E 3) avec le User-Agent officiel `"codex-cli"` pour éliminer les risques de détection/bannissement.
  * 📂 *Fichiers : `crates/sinew-openai/src/client.rs`, `crates/sinew-openai/src/websocket.rs`, `crates/sinew-app/src/image.rs`*
* **🎨 Splash Screen & Correction d'Encodage** : Splash Screen statique instantané (défini dans `index.html`) pour supprimer le flash blanc au démarrage. Remplacement et échappement unicode de tous les points médians (`·`) pour éliminer définitivement les bugs d'affichage Windows.
  * 📂 *Fichiers : `index.html`, `src/styles.css`, `crates/sinew-app/src/agent/turn.rs`*
* **🏷️ Préfixe automatique de conversation** : Les conversations créées sont automatiquement préfixées avec le nom réel du PC actif (ex: `[PCSALON] ` ou `[PCPORTABLE] `) basé sur les variables d'environnement (`%COMPUTERNAME%` / `$HOSTNAME`), facilitant le tri multi-PC.
  * 📂 *Fichiers : `crates/sinew-app/src/store.rs`*
