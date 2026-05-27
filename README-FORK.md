# 🛠️ Mon Fork de Sinew — Fonctionnalités & Améliorations

Ce document liste les fonctionnalités développées pour mon usage quotidien sur ce fork personnel de **Sinew**. Elles sont présentées de manière claire et modulaire, prêtes à être récupérées par l'auteur original s'il le souhaite !

---

## 🚀 1. Démarrage & Sessions

* **📦 Mode Sandbox (Sans dossier)** : Lancez Sinew en un clic sans ouvrir de projet. Idéal pour tester l'IA ou utiliser les outils MCP de manière isolée.
  * 📂 *Fichiers : `src/components/Welcome.tsx`, `src-tauri/src/workspace.rs`*
* **🏷️ Préfixe automatique de conversation** : Les conversations créées sont automatiquement préfixées de `[Bureau] ` ou `[Perso] ` en fonction du nom d'hôte de la machine (`%COMPUTERNAME%`), facilitant le tri et l'identification lors de la synchronisation Multi-PC.
  * 📂 *Fichiers : `crates/sinew-app/src/store.rs`*
* **🎨 Écran de Démarrage & Splash Logo** : Suppression complète du flash blanc au lancement de la fenêtre grâce à un Splash Screen intégré directement dans le HTML statique. Le logo de démarrage s'anime de manière fluide (ouverture des barres de l'icône) avant le chargement complet de l'application React.
  * 📂 *Fichiers : `index.html`, `src/components/Welcome.tsx`, `src/styles.css`*
* **🛑 Désactivation de la mise à jour automatique** : L'auto-updater officiel est désactivé pour éviter que vos modifications personnelles et les fonctionnalités propres à ce fork ne soient écrasées par les versions amont standard.
  * 📂 *Fichiers : `src-tauri/tauri.conf.json`, `src/components/Welcome.tsx`*

---

## 💬 2. Interface de Chat & Expérience Utilisateur

* **⚡ Bouton « Influencer » (Pending/Steering)** : Un badge distinctif `[ Influencer ]` pour injecter instantanément un prompt mis en attente et guider le flux de l'IA sans forcer d'arrêt immédiat (le moteur intègre les consignes d'orientation utilisateur au prochain checkpoint logique).
  * 📂 *Fichiers : `src/components/chat/TodoStrip.tsx`, `crates/sinew-app/src/agent/cancel.rs`, `crates/sinew-app/src/agent/turn.rs`, `src-tauri/src/turns.rs`*
* **🚀 Exécution directe depuis le Chat / l'Explorateur** : Clic droit sur n'importe quel chemin de fichier cité par l'IA ou dans l'arbre des fichiers du FileTree pour l'exécuter directement avec son application système par défaut, ou l'ouvrir dans l'éditeur.
  * 📂 *Fichiers : `src/components/chat/Markdown.tsx`, `src/components/FileTree.tsx`*
* **📌 Question Collante (Sticky)** : La dernière question posée reste fixée en haut lors du défilement. Cliquez dessus pour y remonter instantanément.
  * 📂 *Fichiers : `src/components/chat/ChatPane.tsx`*
* **📋 Copie libre** : La sélection de texte est entièrement débloquée dans les bulles de chat pour copier facilement n'importe quel extrait.
  * 📂 *Fichiers : `src/styles.css`*
* **🛠️ Diagnostics Monaco & read_lints** : Remontée instantanée des avertissements et erreurs de compilation de l'éditeur de code Monaco vers le backend. L'outil `read_lints` permet à l'IA d'interroger directement ces diagnostics locaux (ainsi que `cargo`, `eslint`, `ruff`) pour corriger son code de façon autonome.
  * 📂 *Fichiers : `src/components/EditorPane.tsx`, `crates/sinew-app/src/read_lints.rs`, `crates/sinew-app/src/editor_diagnostics.rs`*
* **🧠 Compaction Visuelle des Tools & Logs** : En modes `Compact` ou `Très compact`, les cartes d'outils réussis (lecture, écriture, bash, todo) replient automatiquement leurs diffs, arguments et détails techniques pour garder le chat lisible. Le mode `Très compact` cache également les thinking blocks réussis pour n'afficher que l'animation d'état en cours.
  * 📂 *Fichiers : `src/components/chat/ToolCard.tsx`, `src/components/chat/PlanningNextMoveBlock.tsx`, `src/components/chat/AIThinkingBlock.tsx`, `src/components/chat/stream.ts`*
* **🔤 Correction d'Encodage Windows** : Remplacement et échappement unicode de tous les caractères point médian (`·`) dans les messages d'erreur et logs d'outils pour éliminer définitivement les bugs d'affichage sur Windows.
  * 📂 *Fichiers : `crates/sinew-app/src/agent/turn.rs`, `src/components/chat/ChatPane.tsx`*

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
* **⚡ Lancement Silencieux (Suppression des Popups Windows)** : Masquage total des clignotements intempestifs d'invites de commandes (`cmd.exe`/`powershell.exe`) lors du démarrage des serveurs d'outils MCP (Node/Python), des commandes Git, ou de l'analyse globale SOTA.
  * 📂 *Fichiers : `crates/sinew-app/src/bash.rs`, `src-tauri/src/platform.rs`, `src-tauri/src/git.rs`, `crates/sinew-app/src/check_sota.rs`*
* **⚡ Diagnostic SOTA** : Un panneau de contrôle en un clic pour tester en temps réel l'état et la version de vos outils (Git, Node, Rust, Python, Ripgrep).
  * 📂 *Fichiers : `crates/sinew-app/src/check_sota.rs`*

---

## 🔌 4. Connecteurs & Intégrations (`Settings > Providers & MCP`)

* **🌐 Sinew Chrome Bridge de Pointe** : Pilotez Google Chrome via un pont MCP natif en Rust (`native-host-wrapper.exe`). Gère automatiquement les conflits de ports, utilise des courbes physiques de Bézier multi-candidates pour simuler des mouvements de souris humains (CDP), masque la barre d'avertissement de débogage de Chrome, et propose un menu de diagnostic premium.
  * 📂 *Fichiers : `sinew-chrome-bridge/mcp_server.js`, `sinew-chrome-bridge/popup.js`, `sinew-chrome-bridge/background.js`, `.sinew/skills/browser/SKILL.md`*
* **🤖 Cursor & Agent Composer 2.5 (OAuth)** : Connexion sécurisée via OAuth (remplace l'extraction SQLite locale instable de `state.vscdb`) avec mimétisme d'IDE (checksum, client version). Comprend un filtre d'anonymisation furtif (`sanitize.rs`) remplaçant toute mention de 'Sinew' ou 'Hyrak' par 'Cursor' dans les requêtes sortantes pour éviter le blocage par brand fingerprinting.
  * 📂 *Fichiers : `crates/sinew-cursor/`, `crates/sinew-cursor/src/sanitize.rs`, `src-tauri/src/providers.rs`, `src/lib/models.ts`, `src/components/SettingsPane.tsx`*
* **🔍 Indexation Sémantique Locale (Embeddings)** : Module `sinew-index` pour analyser en arrière-plan le projet et générer des embeddings vectoriels locaux. L'IA injecte ces résultats de recherche sémantique comme contexte. Un badge d'état interactif en barre latérale affiche le nombre de fichiers et fragments indexés.
  * 📂 *Fichiers : `crates/sinew-index/`, `crates/sinew-app/src/codebase_search.rs`, `src/components/Workspace.tsx`*
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
* **🛡️ Sécurisation & Spoofing ChatGPT Codex & Gemini** :
  * *Spoofing ChatGPT Codex* : Toutes les requêtes (WebSockets, SSE et images DALL-E 3) utilisant le compte ChatGPT Codex transmettent le User-Agent officiel `"codex-cli"` pour éliminer les risques de détection/bannissement.
  * *Détection Dynamique de Plateforme (Gemini)* : Génération d'un `user-agent` réaliste simulant précisément l'environnement hôte de l'utilisateur (OS et architecture) pour éviter les incohérences d'empreinte sur les appels quotas/chat Gemini.
  * *Expérience Gemini Ultra-Fluide* : Activation explicite de HTTP/2 sur les requêtes Rust et désactivation de la coloration syntaxique auto pendant le streaming pour un temps de réponse instantané.
  * 📂 *Fichiers : `crates/sinew-openai/src/client.rs`, `crates/sinew-openai/src/websocket.rs`, `crates/sinew-google/src/client.rs`, `src/components/chat/Markdown.tsx`, `Cargo.toml`*

---

## 📅 27/05 — Guidage dynamique & Ajustements Globaux

Cette journée rassemble les optimisations et intégrations majeures du fork (les détails techniques et fichiers concernés sont documentés dans les sections thématiques ci-dessus) :
* **🧭 Guidage Pending/Steering** : Injection à chaud des orientations utilisateur via le bouton « Influencer » sans forcer d'arrêt immédiat du flux.
* **🤖 Routage Cursor & Composer 2.5** : Connexion OAuth sécurisée, mimétisme d'en-têtes et anonymisation furtive des prompts sortants.
* **🔍 Indexation Sémantique Locale** : Indexation vectorielle autonome en arrière-plan avec affichage de l'état dans la barre latérale.
* **🛡️ Sécurisation & Spoofing** : Masquage réseau et spoofing d'en-têtes (Codex, Antigravity) pour éviter les rejets et files d'attente lentes.
* **⚡ Suppression des Popups de Console** : Masquage total des clignotements d'invites de commandes Windows (serveurs MCP, Git, diagnostics SOTA).
* **🛠️ Diagnostics Monaco & read_lints** : Remontée directe des avertissements de l'éditeur Monaco pour correction autonome par l'IA.
* **🧠 Compaction Visuelle des Tools** : Masquage automatique des détails et diffs d'outils réussis en mode Compact/Très compact.
* **🎨 Splash Screen & Correction d'Encodage** : Splash screen statique anti-flash blanc et échappement des points médians (`·`).
