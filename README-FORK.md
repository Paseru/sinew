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

* **🧭 Guidage dynamique Pending/Steering** : Le bouton « Influencer » ne force plus un arrêt immédiat par défaut. La consigne est envoyée au moteur comme message d’orientation en attente, puis intégrée dès le prochain point de contrôle logique.
* **💬 Retour visuel immédiat** : Le message apparaît tout de suite dans le chat avec un badge **Pending**, et le bouton passe aussi en état **Pending** pendant l’attente.
* **⚙️ Points de contrôle côté moteur** : L’agent relit les consignes d’orientation avant une nouvelle requête modèle, après une réponse assistant et après l’exécution d’outils.
* **🛑 Arrêt fiable en cas de blocage réseau** : Les phases sensibles (chargement du catalogue MCP, connexion au modèle IA et pauses de retry après erreur réseau/503) écoutent maintenant le bouton **Arrêter**. Si un fournisseur rame ou tourne en boucle sur des retries, l'utilisateur peut reprendre la main proprement sans fermer l'application.
* **🛡️ Fallback sécurisé** : Si le moteur ne peut pas accepter l’orientation en direct, l’application conserve l’ancien comportement fiable : arrêt propre puis relance avec la consigne.
  * 📂 *Fichiers : `crates/sinew-app/src/agent/cancel.rs`, `crates/sinew-app/src/agent/turn.rs`, `src-tauri/src/turns.rs`, `src/components/chat/ChatPane.tsx`, `src/components/chat/TodoStrip.tsx`, `src/components/chat/stream.ts`, `src/lib/ipc.ts`, `src/types.ts`, `src/styles.css`*
* **🤖 Ajustements Google Antigravity & Quotas** :
  * *Disponibilité vérifiée de tous les modèles* : Validation et prise en charge opérationnelle confirmée de l'ensemble des modèles du catalogue Antigravity, y compris **Claude Opus** (`claude-opus-4-6` mappé sur `claude-opus-4-6-thinking`), Sonnet (`claude-sonnet-4-6`), Gemini (3.1 Pro, 3.5 Flash) et GPT-OSS (120B).
  * *Détection Dynamique de la Plateforme (OS & Architecture)* : Génération d'un en-tête `user-agent` réaliste simulant précisément l'environnement hôte de l'utilisateur (Windows, macOS, Linux, avec architectures x86_64 ou arm64) pour toutes les requêtes de chat ainsi que l'appel d'API de récupération des quotas (auparavant figé sur `windows/amd64`), éliminant tout risque de mismatch d'empreinte.
  * *Stabilisation de l'onboarding et du suivi des quotas* : Utilisation par défaut du point de terminaison de production (`cloudcode-pa`), et sécurisation de la récupération des quotas via des en-têtes standardisés (`x-goog-api-client: gl-node/22.21.1` et `user-agent` approprié) pour éviter les blocages.
  * 📂 *Fichiers : `crates/sinew-google/src/client.rs`, `src-tauri/src/providers.rs`*
* **⚡ Expérience Gemini façon Antigravity** : Optimisation ciblée pour que Gemini 3.5 Flash se sente aussi fluide et puissant dans Sinew que dans Antigravity.
  * *Transport réseau aligné* : activation explicite de HTTP/2 côté `reqwest` pour rapprocher le backend Rust du comportement bas-latence observé dans le `language_server.exe` d’Antigravity.
  * *Rendu Markdown allégé* : désactivation de la détection automatique coûteuse de langage pendant le streaming, tout en gardant la coloration des blocs qui déclarent leur langage.
  * *Batch UI par frame* : fusion des micro-fragments (`text_chunk`, `thinking_chunk`, deltas d’outils) et rendu via `requestAnimationFrame`, avec flush immédiat avant les événements structurants pour préserver l’ordre exact.
  * 📂 *Fichiers : `Cargo.toml`, `src/components/chat/Markdown.tsx`, `src/components/chat/ChatPane.tsx`*
* **🛡️ Sécurisation & Spoofing ChatGPT Codex** :
  * *Masquage complet de l'identité* : toutes les requêtes (WebSockets, flux HTTP SSE et appels d'API de chat/quotas) utilisant le compte ChatGPT Codex transmettent désormais l'en-tête `user-agent` officiel `"codex-cli"` pour éviter toute détection.
  * *Génération d'images sous couverture* : correction de l'outil de création d'images par abonnement (DALL-E 3) qui n'envoyait pas le bon User-Agent, désormais aligné sur `"codex-cli"`.
  * 📂 *Fichiers : `crates/sinew-openai/src/client.rs`, `crates/sinew-openai/src/websocket.rs`, `crates/sinew-app/src/image.rs`*

---

## 📅 28/05 — Intégration Cursor (Composer), Indexation Sémantique & Optimisations Système

* **🤖 Intégration de Cursor & Agent Composer 2.5** :
  * *Authentification OAuth Sécurisée* : Connexion fluide via OAuth avec renouvellement automatique des jetons de session (remplace la lecture locale SQLite instable de `state.vscdb` pour éviter les soucis de droits d'accès).
  * *Stealth & Mimétisme IDE* : En-têtes HTTP personnalisés (`x-cursor-client-version` réglable via `SINEW_CURSOR_CLIENT_VERSION`, `user-agent`, `x-cursor-checksum` calculé dynamiquement) simulant un client Cursor légitime pour éviter tout blocage.
  * *Support Multi-sessions & Multimodal* : Routage des abonnements prioritaires, gestion de la vision (analyse d'images) et génération d'images via providers locaux ou DALL-E dans l'agent Composer.
  * *Gestion des dossiers récents* : Possibilité de supprimer des dossiers de l'historique directement depuis l'écran d'accueil.
  * 📂 *Fichiers : `crates/sinew-cursor/`, `src-tauri/src/providers.rs`, `src/lib/models.ts`, `src/components/SettingsPane.tsx`, `src/components/Welcome.tsx`, `src/lib/recents.ts`*

* **🔍 Indexation Sémantique Locale & Codebase Search** :
  * *Indexation Arrière-plan & Embeddings* : Module `sinew-index` pour analyser le projet localement et générer des embeddings vectoriels des fichiers avec découpage intelligent respectueux de la structure des symboles.
  * *Badge d'état dans la Sidebar* : Affichage dynamique sous le nom du projet du nombre de fichiers et fragments indexés, ainsi que du moteur actif (sémantique ou classique).
  * *Context Injection* : Intégration transparente des résultats de recherche sémantique comme contexte explicite injecté directement dans les prompts envoyés à l'agent.
  * 📂 *Fichiers : `crates/sinew-index/`, `crates/sinew-app/src/codebase_search.rs`, `src/components/chat/ChatPane.tsx`, `src/components/Workspace.tsx`*

* **🌐 Extension Chrome (Sinew Chrome Bridge) de Pointe** :
  * *Suppression du Timeout de 20s* : Résolution d'un délai d'attente bloquant lors de la navigation et de la recherche d'onglets cibles via CDP.
  * *Stealth & Trajectoire Bézier* : Déplacements du curseur simulés via des courbes de Bézier physiques multi-candidates et masquage complet de la barre d'avertissement de débogage Chrome.
  * *Design Premium de la Popup* : Redesign complet avec thème sombre moderne, lueur néon, états de diagnostic pliables et indicateur d'attachement du debugger en temps réel.
  * *Exécution depuis l'Explorateur & Chat* : Intégration d'un menu d'ouverture directe et d'exécution dans les liens de fichiers du chat et dans le menu contextuel clic-droit du FileTree.
  * 📂 *Fichiers : `sinew-chrome-bridge/mcp_server.js`, `sinew-chrome-bridge/popup.js`, `sinew-chrome-bridge/background.js`, `.sinew/skills/browser/SKILL.md`, `src/components/FileTree.tsx`, `src/components/chat/Markdown.tsx`*

* **⚡ Suppression des Popups de Console sur Windows** :
  * *Lancement Silencieux des Processus* : Suppression définitive des clignotements intempestifs de fenêtres d'invite de commandes Windows (`cmd.exe`/`powershell.exe`) lors du démarrage des serveurs d'outils MCP, des commandes Git ou de l'analyse globale SOTA.
  * 📂 *Fichiers : `crates/sinew-app/src/bash.rs`, `src-tauri/src/platform.rs`, `src-tauri/src/git.rs`, `crates/sinew-app/src/check_sota.rs`*

* **🎨 Ajustements UI, Encodage & Mode Très Compact** :
  * *Correction des Bugs d'Encodage Windows* : Remplacement et échappement unicode de tous les caractères point médian (`·`) pour éviter les plantages d'affichage.
  * *Rendu Grand Écran & Démarrage* : Fixation de la largeur `#root` à 100% sur écran large et animations d'introduction stylisées de l'icône de boot.
  * *Mode Ultra Pur* : Amélioration du mode "Très compact" pour masquer les tools réussis et n'afficher que l'animation d'état en cours.
  * 📂 *Fichiers : `src/styles.css`, `src/components/Welcome.tsx`, `crates/sinew-app/src/agent/turn.rs`, `src/components/chat/AIThinkingBlock.tsx`*
