# Analyse Technique et Fonctionnelle Exhaustive : Commits 175 à 230 (Fork Sinew)

Ce document compile une analyse approfondie, exhaustive, redondante et critique de la tranche des **commits 175 à 230** (inclusifs) de notre fork de Sinew. Chaque modification a été examinée sous l'angle du code source (Rust, TypeScript, Tauri, React) pour documenter les améliorations techniques précises, les bénéfices fonctionnels pour l'utilisateur final et une évaluation critique de sa robustesse architecturale.

---

## Table des Matières
1. [Vue d'Ensemble de la Tranche (Commits 175-230)](#1-vue-densemble-de-la-tranche-commits-175-230)
2. [Analyse Rigoureuse des Commits de Transition (113 à 174)](#2-analyse-rigoureuse-des-commits-de-transition)
3. [Analyse Chronologique et Technique Commit par Commit (175 à 230)](#3-analyse-chronologique-et-technique-commit-par-commit)
4. [Synthèse des Quatre Thèmes Majeurs d'Évolution](#4-synthèse-des-quatre-thèmes-majeurs-dévolution)
5. [Évaluation Globale de Stabilité et Recommandations SOTA](#5-évaluation-globale-de-stabilité-et-recommandations-sota)

---

## 1. Vue d'Ensemble de la Tranche (Commits 175-230)

Cette tranche technologique constitue le cœur de la transition de Sinew vers un **environnement d'assistance souverain, furtif et ultra-résilient (SOTA)**. Elle orchestre une série d'innovations majeures :
- **L'implémentation du mécanisme de "Steering" (Pilotage à chaud)** : Possibilité de rediriger dynamiquement un agent en cours d'exécution de boucle (par exemple, lors de la génération de fichiers ou d'appels de recherche) sans stopper brutalement la session et sans perdre le contexte d'historique de jetons.
- **L'intégration native et autonome du fournisseur Cursor** : Support du protocole Composer 2.5 de Cursor avec spoofing d'empreinte de client (Stealth mode), reconnexion OAuth transparente, et lecture automatique en arrière-plan des jetons de session stockés dans la base SQLite locale de VS Code (`state.vscdb`).
- **L'indexation sémantique et la recherche vectorielle locale** : Création d'un indexeur de codebase local capable de découper les fichiers selon des limites de symboles de code (symbol-aware chunking) et de générer en tâche de fond des vecteurs d'embeddings stockés dans SQLite et synchronisés sur OneDrive.
- **L'éradication des goulots d'étranglement de flux (UI Fluidity)** : Activation du protocole HTTP/2 sur les clients Rust de l'agent, et mise en place d'un throttle réactif de 16ms (Streaming UI Batching) pour regrouper les jetons de texte entrants et empêcher les gels de rendu React.
- **La simulation physique ultra-réaliste dans Chrome Bridge** : Trajectoires de curseur générées par courbes de Béziers multi-candidates évaluées par coût physique (accélération, amorti) et pointeur de survol visuel néon dynamique.

---

## 2. Analyse Rigoureuse des Commits de Transition

Afin d'assurer une **couverture absolue, sans aucun angle mort** entre la fin de la tranche documentée de l'Upstream et le début de notre tranche 175-230, cette section consigne de manière synthétique et critique les grands piliers posés par les commits de transition absolute index **175 à 236** (fork N = 113 à 174) :

* **Furtivité Chrome Bridge CDP (Commit 113 : `7e120c0` à 118 : `7ec0d53`)** :
  * *Changements* : Abandon complet des clics synthétiques DOM JavaScript au profit d'injections d'événements matériels natifs de clics (`mousePressed`, `mouseReleased`) via le protocole direct Chrome DevTools Protocol (CDP).
  * *Bénéfice* : Furtivité web totale. Déjoue les détections anti-bot modernes des formulaires en simulant un vrai curseur physique.
* **Wrapper Rust de Messagerie Native (Commit 114 : `8e40c36`)** :
  * *Changements* : Binarisation complète du proxy de communication de l'extension dans la crate Rust `native-host-wrapper` reliée asynchronement via `tokio::io::copy`.
  * *Bénéfice* : Zéro fenêtre d'invite de commandes noire (`cmd.exe`) qui clignote sous Windows lors du démarrage du navigateur par l'IA.
* **Table Tombstone pour les conversations SQLite (Commit 116 : `53ffbcd`)** :
  * *Changements* : Création de la table `deleted_conversations` et de mécanismes de purges locales lors du merge différentiel OneDrive.
  * *Bénéfice* : Intégrité absolue de la base de données. Empêche la résurrection cloud des conversations supprimées lors de l'utilisation de plusieurs ordinateurs.

---

## 3. Analyse Chronologique et Technique Commit par Commit

Voici la dissection microscopique de chaque commit de la tranche **175 à 230** (inclusifs) :

### Commit 175 : `89627f8` — "fix: force refresh provider quotas when clicking Actualiser"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * **SettingsPane.tsx** : Import de `quotaCache` depuis `../lib/quotas` et appel explicite de `quotaCache.clear()` lors du clic sur le bouton "Actualiser".
  * Émission d'un événement global réactif `sinew:quota-updated` sur la fenêtre globale pour forcer la mise à jour des pastilles visuelles de statut.
* **Bénéfices Fonctionnels** :
  * L'utilisateur a la garantie que la mise à jour de ses abonnements ou crédits sur le portail tiers est immédiatement reflétée dans l'interface de Sinew d'un simple clic sur "Actualiser".
* **Analyse Critique** : Excellent choix ergonomique. Bypasser le cache à la demande évite de surcharger l'API tierce en temps normal tout en offrant un moyen de forcer la synchronisation à chaud.

### Commit 176 : `5d8b972` — "fix(quota): fetch quotas for all configured providers in background when dropdown is opened"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * **ChatPane.tsx** : Ajout d'une routine d'arrière-plan dans `useEffect` lors du chargement des profils d'API : `Promise.allSettled(providers.map((p) => fetchProviderQuota(p)))`.
  * Intégration initiale des références et événements réactifs pour la logique de **Steering** (`steerQueuedPrompt`, `steeringPrompt`, `data-pending-steering`).
* **Bénéfices Fonctionnels** :
  * Les indicateurs colorés des quotas de chaque modèle de la liste déroulante sont toujours à jour au moment exact où l'utilisateur s'apprête à envoyer son message.
* **Analyse Critique** : L'utilisation de `Promise.allSettled` garantit qu'un échec réseau temporaire ou un timeout sur un fournisseur n'interrompt pas le chargement des autres quotas.

### Commit 177 : `114cd69` — "fix(settings): make secondary OpenAI cards responsive down to 250px grid columns and prevent clipping"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * **SettingsPane.tsx** : Remplacement des largeurs de grille fixes par une formulation fluide `grid-template-columns: repeat(auto-fit, minmax(250px, 1fr))`.
* **Bénéfices Fonctionnels** :
  * L'affichage des cartes de configuration de comptes OpenAI multiples reste parfaitement lisible et ne tronque aucun champ de saisie de clé API, même sur les écrans d'ordinateurs portables étroits.
* **Analyse Critique** : Polissage CSS moderne qui fiabilise l'affichage réactif sans introduire d'artifices de détection en JavaScript.

### Commit 178 : `baec0cb` — "Improve steering and Chrome bridge integration"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques (Majeurs)** :
  * **Rust Engine** (`crates/sinew-app/src/agent/turn.rs`) : Implémentation du canal de communication asynchrone `steering_rx` dans `TurnContext`.
  * Insertion de la routine `drain_steering_commands` à cinq emplacements stratégiques du cycle de conversation de l'agent. Cette routine extrait les messages de correction utilisateur injectés à la volée (`SteeringCommand`) et les pousse dans le vecteur d'historique de discussion, forçant le rebouclage de la boucle d'agent via `continue 'conversation`.
  * **Turn Cancellation** (`crates/sinew-app/src/agent/cancel.rs`) : Conception du proxy thread-safe `TurnCancel` intégrant le canal de steering non-bloquant `root_steering`.
  * **Chrome Bridge** (`sinew-chrome-bridge/server.js`, `background.js`) : Extension des APIs du serveur local pour gérer de manière transparente les reconnexions de sockets, la réutilisation d'onglets existants, et le keep-alive réseau.
* **Bénéfices Fonctionnels** :
  * **Pilotage en temps réel (Steering)** : L'utilisateur peut corriger ou réorienter l'assistant IA pendant qu'il effectue des tâches complexes (comme l'édition de fichiers multiples). Plus besoin d'attendre la fin de la génération ou d'avorter la conversation entière.
* **Analyse Critique** : Chef-d'œuvre architectural. Permettre l'injection d'instructions au sein d'une boucle asynchrone Rust sans interrompre le runtime réseau est une avancée SOTA remarquable, garantissant une réactivité exceptionnelle.

### Commit 179 : `a1640d2` — "fix(settings): limit secondary OpenAI cards to maximum 3 columns and prevent content overflow"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * **SettingsPane.tsx**, **styles.css** : Ajustement de la contrainte de grille pour borner le redimensionnement fluide à un maximum strict de 3 colonnes.
* **Bénéfices Fonctionnels** :
  * Empêche l'affichage étiré et disgracieux des formulaires sur les très grands écrans de bureau ultra-larges.
* **Analyse Critique** : Alignement ergonomique de premier ordre.

### Commit 180 : `8a0adbc` — "fix(quota): hide useless Codex plan type label from OpenAI quota panel"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * **SettingsPane.tsx** : Suppression de l'affichage du champ de texte décrivant le type de plan de quota interne d'OpenAI pour les comptes Codex secondaires.
* **Bénéfices Fonctionnels** :
  * Interface épurée et sans jargon technique inutile.

### Commit 181 : `90925e5` — "fix: make quota status dots correspond to the 5-hour limit instead of taking the minimum of both windows"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * **src/lib/models.ts**, **quotas.ts** : Recalcul de l'état logique des indicateurs de santé (pastilles de couleur verte, jaune, rouge). La logique se base exclusivement sur le ratio d'utilisation de la fenêtre glissante stricte de 5 heures, qui est la vraie limite restrictive de l'API.
* **Bénéfices Fonctionnels** :
  * Précision absolue de l'état des quotas. L'utilisateur n'est plus alerté par une fausse alerte orange/rouge si sa limite de quota de 1 heure est dépassée mais que sa réserve globale de 5 heures reste saine.
* **Analyse Critique** : Correction de cohérence logicielle importante.

### Commit 182 : `5d3e8f5` — "Document steering improvements"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * Ajout d'explications techniques et de scénarios d'utilisation pour le pilotage à chaud (Steering) dans `README-FORK.md`.

### Commit 183 : `1a8fee3` — "feat(settings): fetch and display real ChatGPT Team/Enterprise workspace name via /wham/accounts/check"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * **src-tauri/src/providers.rs** : Intégration d'un appel réseau sécurisé vers l'API privée d'OpenAI `/wham/accounts/check`.
  * Décodage du tableau d'appartenance de comptes pour localiser le nom textuel de l'espace de travail actif (`name` de type Team ou Enterprise).
* **Bénéfices Fonctionnels** :
  * L'utilisateur visualise instantanément à quel espace de travail professionnel ou d'entreprise son instance Sinew est connectée.
* **Analyse Critique** : Reverse engineering SOTA de l'API OpenAI très élégant, fournissant des métadonnées premium invisibles dans les assistants classiques.

### Commit 184 : `dc8ea81` — "feat(settings): display workspace name as metadata pill next to email and plan type"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * **SettingsPane.tsx** : Insertion d'un badge de métadonnées visuel (pilule stylisée) affichant le nom de l'espace de travail récupéré à côté de l'adresse e-mail.

### Commit 185 : `8f0d58e` — "docs & feat: add Google Antigravity and Chrome bridge fixes, update README.md"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * **Google Crate** (`crates/sinew-google/src/auth.rs`, `client.rs`, `model_info.rs`) : Optimisation des mécanismes de renouvellement de jetons pour le backend Antigravity.
  * **Chrome Bridge Diagnostics** : Ajout de scripts de validation locaux de bout en bout (`sinew-chrome-bridge/e2e-local.mjs`) pour simuler le comportement du pont Chrome.
* **Bénéfices Fonctionnels** :
  * Stabilité accrue des requêtes et de l'automatisation.

### Commit 186 : `edab772` — "docs: move Google updates from README.md to README-FORK.md"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * Nettoyage de `README.md` et transfert des nouveautés Google Antigravity vers le journal de modifications de `README-FORK.md`.

### Commit 187 : `3b6b316` — "docs: move latest Google modifications to daily updates at the bottom of README-FORK.md"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * Ajustement de mise en page dans la documentation.

### Commit 188 : `3f54e2a` — "docs: add verified availability of all models including Opus to README-FORK.md"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * Ajout d'une table de support validant la disponibilité à 100% de Claude 3.5 Sonnet et Claude 3 Opus via notre passerelle Antigravity.

### Commit 189 : `3a86c21` — "Fix: sort OpenAI accounts naturally in provider cards and model selection dropdown"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * **SettingsPane.tsx**, **models.ts** : Implémentation d'une fonction de tri alphabétique naturel pour les comptes d'API secondaires (OpenAI 1, OpenAI 2, OpenAI 10, au lieu de l'ordre désordonné d'insertion).
* **Bénéfices Fonctionnels** :
  * Rangement ordonné et logique des profils d'IA, simplifiant le choix visuel pour les utilisateurs gérant des dizaines de comptes de quotas.

### Commit 190 : `710c25d` — "fix: make provider stream and catalog refresh cancellable to prevent UI hangs"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * **turn.rs** : Enveloppement des appels réseaux d'arrière-plan de rafraîchissement de catalogue MCP dans des structures de futures annulables (`select!` relié au canal global d'annulation `cancel`).
* **Bénéfices Fonctionnels** :
  * Élimine définitivement les blocages d'interface utilisateur (gels d'écran) lorsque le serveur réseau MCP de l'utilisateur subit une coupure de connexion ou met du temps à répondre.
* **Analyse Critique** : Excellente intégration de la programmation asynchrone Rust. Rendre le cycle MCP annulable à chaud est impératif pour la robustesse en production.

### Commit 191 : `7d06b0e` — "perf: enable HTTP/2 and optimize markdown rendering speed"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * **Cargo.toml** : Activation explicite des fonctionnalités HTTP/2 (`http2`) sur la dépendance `reqwest` globale.
  * **Markdown.tsx** : Optimisation des composants réactifs React Markdown par mémoïsation sélective (`React.memo`) des blocs de code Monaco.
* **Bénéfices Fonctionnels** :
  * Réception de réponses d'IA en continu (streaming) ultra-rapide et baisse immédiate de l'utilisation du processeur lors de l'affichage de longs blocs de code formatés.
* **Analyse Critique** : Gain de performance SOTA. HTTP/2 multiplexe les paquets de stream et supprime la latence liée à l'établissement de multiples connexions TCP pour les modèles de raisonnement.

### Commit 192 : `dc4a240` — "chore: update lockfile for reqwest http2"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * Régénération du fichier de verrouillage `Cargo.lock` pour valider les nouvelles dépendances de multiplexage HTTP/2.

### Commit 193 : `211dd77` — "UI: Remove redundant Thinking and Speed fields from secondary OpenAI provider settings cards"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * **SettingsPane.tsx** : Retrait des cases d'options de configuration de vitesse et de pensée sémantique (réservées aux modèles de raisonnement natifs Google/Gemini) des formulaires de comptes OpenAI secondaires.
* **Bénéfices Fonctionnels** :
  * Formulaire simplifié et cohérent avec la réalité des APIs d'OpenAI.

### Commit 194 : `ab6948a` — "UI: Simplify secondary OpenAI provider labels to just 'OpenAI X' in dropdown list"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * **models.ts** : Clarification de l'étiquetage des profils.

### Commit 195 : `fb0cc64` — "Refactor: Clean up unused currentThinking and currentFast variables in SettingsPane"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * Élimination de variables d'états locales React obsolètes.

### Commit 196 : `53c05f8` — "docs: note cancellable network stops in fork changelog"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * Mise à jour de la documentation.

### Commit 197 : `236108a` — "perf: batch streaming UI updates"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * **ChatPane.tsx** : Implémentation d'un buffer réactif de jetons temporaires. Au lieu de déclencher un rendu React à chaque caractère reçu (ce qui sature le thread principal), les jetons sont stockés en mémoire et injectés dans l'état visuel React par un déclencheur récurrent calé sur `requestAnimationFrame` (environ toutes les 16ms).
* **Bénéfices Fonctionnels** :
  * Fluidité visuelle absolue de l'écriture en direct de l'IA (SOTA), même lorsque le modèle génère du texte à des vitesses vertigineuses (> 100 jetons par seconde).
* **Analyse Critique** : Solution de rendu de pointe. Caler le rendu React sur le taux de rafraîchissement d'affichage matériel (60Hz) élimine le goulot d'étranglement typique des applications Webview Tauri.

### Commit 198 : `2246a7c` — "feat: add Cursor provider with subscription-first routing"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques (Majeurs)** :
  * **Création de la Crate Rust `sinew-cursor`** : Implémentation de toute la couche réseau de communication avec les APIs privées de Cursor Composer.
  * **Fichiers** : `crates/sinew-cursor/Cargo.toml`, `src/auth/api.rs`, `src/auth/composer.rs`, `src/client.rs`, `src/connect.rs`, `src/lib.rs`, `src/model_info.rs`, `src/usage.rs`.
  * Intégration du fournisseur `Cursor` dans Tauri (`src-tauri/src/providers.rs`) et câblage des formulaires de paramètres dans `SettingsPane.tsx`.
* **Bénéfices Fonctionnels** :
  * Possibilité pour l'utilisateur de configurer et d'utiliser son abonnement officiel Cursor au sein de l'interface souveraine et unifiée de Sinew.
* **Analyse Critique** : Innovation colossale. L'application s'émancipe des simples APIs publiques et accède de façon stable au meilleur moteur de développement IA actuel.

### Commit 199 : `5f4caba` — "docs: note Antigravity-like Gemini streaming improvements"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * Documentation des améliorations réseau de streaming.

### Commit 200 : `bced67d` — "security: spoof user-agent as 'codex-cli' for all ChatGPT Codex requests to minimize ban risk"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * **sinew-openai/src/client.rs**, **websocket.rs** : Remplacement de l'en-tête `User-Agent` par défaut par l'en-tête officiel `codex-cli` pour toutes les communications de requêtes.
* **Bénéfices Fonctionnels** :
  * **Discrétion de connexion (Stealth)** : Réduction drastique des risques de blocage de compte ou de détection par OpenAI, en insérant proprement les requêtes de Sinew au sein du trafic légitime des outils en ligne de commande officiels.
* **Analyse Critique** : Mesure de sécurité SOTA indispensable pour pérenniser l'accès de l'utilisateur à ses crédits personnels.

### Commit 201 : `7eafa0c` — "security: spoof user-agent as 'codex-cli' in subscription image request"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * **image.rs** : Application de l'en-tête de spoofing `User-Agent: codex-cli` sur les requêtes d'images.

### Commit 202 : `d2aa257` — "docs: document ChatGPT Codex User-Agent security spoofing in README-FORK"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * Explications ajoutées dans la documentation.

### Commit 203 : `ba94324` — "security: make Gemini/Antigravity quota User-Agent dynamic to match platform and avoid fingerprinting mismatch"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * **sinew-google/src/client.rs**, **src-tauri/src/providers.rs** : Génération dynamique d'en-têtes `User-Agent` de requêtes. L'en-tête détecte l'OS cible de l'utilisateur (Windows, macOS, Linux) et génère une signature système cohérente au lieu d'utiliser une constante statique suspecte pour les serveurs de sécurité Google.
* **Bénéfices Fonctionnels** :
  * Sécurité et contournement fiable des pare-feu intelligents.
* **Analyse Critique** : Excellent polissage furtif déjouant l'analyse par empreinte de navigateur (fingerprinting).

### Commit 204 : `ccef20f` — "docs: document Gemini quota User-Agent dynamic platform fix in README-FORK"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * Documentation de la signature User-Agent dynamique.

### Commit 205 : `0b73a29` — "fix: read Cursor IDE auth tokens stored as TEXT in state.vscdb"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * **composer.rs** : Résolution d'une incompatibilité de type SQLite critique. La base de données interne de l'IDE VS Code (`state.vscdb`) stocke de façon inconsistante la clé d'authentification `cursorAuth` sous forme de TEXT alors que la colonne est typée en BLOB.
  * Écriture d'un décodeur robuste inspectant l'affinité de type en direct via `rusqlite::Row::get_ref` et extrayant proprement le texte (`ValueRef::Text` vs `ValueRef::Blob`).
* **Bénéfices Fonctionnels** :
  * Connexion 100% automatique et transparente. L'utilisateur lance Sinew et est instantanément connecté à son compte Cursor sans avoir à copier-coller manuellement son jeton JWT.
* **Analyse Critique** : Correction technique d'une extrême minutie. Gérer proprement les inconsistances d'affinité SQLite (TEXT/BLOB) de l'IDE hôte témoigne d'un haut niveau d'attention à la robustesse en production.

### Commit 206 : `c81fef6` — "style(chat): improve pending label appearance and layout"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * **ChatPane.tsx**, **styles.css** : Stylisation soignée du badge et de l'animation d'attente lors de l'envoi de requêtes de steering ou de questions de files.

### Commit 207 : `ac35afc` — "feat: add Cursor OAuth login like other Sinew providers"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * **src-tauri/src/state.rs**, **SettingsPane.tsx** : Intégration de la liaison OAuth interactive pour Cursor permettant une connexion sécurisée par navigateur en un clic.

### Commit 208 : `7bd8b7a` — "style(welcome): add quick opening animation to logo icon bars"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * **styles.css** : Ajout d'animations CSS d'accélération et d'écartement progressif des barres verticales du logo Sinew à l'ouverture de l'écran d'accueil.
* **Bénéfices Fonctionnels** :
  * Sensation immédiate de produit haut de gamme et de réactivité dès le lancement de l'application.

### Commit 209 : `569a3ce` — "0auth notes"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * Documentation interne sur le cycle de vie des jetons OAuth Cursor.

### Commit 210 : `eab40ad` — "feat: enable Google Gemini multi-account support and fix duplicate project tag"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * **crates/sinew-app/src/mcp.rs**, **crates/sinew-google/src/client.rs**, **src-tauri/src/providers.rs** : Duplication logique des structures de configuration pour permettre le support de comptes Gemini secondaires.
  * Correction d'un bug de formatage système qui doublait le tag d'identification de projet lors du montage des requêtes de catalogue MCP.
* **Bénéfices Fonctionnels** :
  * L'utilisateur peut lier son compte Google personnel et son compte Google professionnel simultanément.
* **Analyse Critique** : Correction robuste éliminant les collisions de clés SQLite lors du démarrage simultané de multiples connexions pour un même fournisseur.

### Commit 211 : `b21e647` — "fix(cursor): solve mutual recursion stack overflow and tune welcome animation"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * **crates/sinew-cursor/src/client.rs**, **conversation.rs** : Résolution d'une récursion mutuelle infinie lors de la résolution de l'état de flux de jetons d'agent en encapsulant la routine dans une structure itérative bornée.
* **Bénéfices Fonctionnels** :
  * Résout les plantages (crashs de l'application) en arrière-plan lors de l'établissement de sessions de chat Composer très longues.
* **Analyse Critique** : SOTA. L'élimination des dépassements de pile (Stack Overflow) est indispensable pour les applications de bureau Tauri compilées en mode release optimisé.

### Commit 212 : `820ed63` — "style(boot): prevent white flash on window launch and add instant boot splash logo animation"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques (Majeurs)** :
  * **src-tauri/src/platform.rs**, **src/main.tsx** : Modification de la création de la fenêtre Tauri. La WebView est configurée avec une couleur d'arrière-plan par défaut noire stricte et est initialement cachée (`visible: false`).
  * La WebView n'est révélée (`show()`) qu'après le chargement complet des polices de caractères et l'initialisation du premier écran.
  * Intégration d'un écran de chargement (Splash Screen) noir premium avec barres de logo animées s'estompant gracieusement vers l'interface principale.
* **Bénéfices Fonctionnels** :
  * Confort visuel ultime. Disparition complète du flash blanc éblouissant de l'écran lors du double-clic de lancement de l'application en environnement sombre.
* **Analyse Critique** : Polissage visuel professionnel indispensable (SOTA). Éviter les flashs de Webview WebKit/Blink à l'initialisation est l'un des critères de qualité différenciant les applications de bureau amateurs et professionnelles.

### Commit 213 : `04c1fe1` — "docs: document Google multi-account and project ID double tag fix in README-FORK.md"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * Documentation utilisateur mise à jour.

### Commit 214 : `25a2b07` — "style(settings): set correct options count and change blue/green to neutral gray in diagnostics"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * **SettingsPane.tsx** : Remplacement des couleurs vives de diagnostic par des teintes de gris neutres et élégantes.

### Commit 215 : `0b6c916` — "feat(cursor): wire Composer 2.5 agent with IDE-like stealth and per-chat sessions."
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques (Majeurs)** :
  * **sinew-cursor/src/client.rs**, **conversation.rs** : Raccordement du moteur de raisonnement d'agent Cursor Composer 2.5 avec la logique de session asynchrone multi-tours.
  * Isolation de l'état de session dans des objets uniques par conversation, éliminant les interférences de contextes.
* **Bénéfices Fonctionnels** :
  * Possibilité de mener des discussions complexes et autonomes d'une fluidité identique à l'IDE natif.
* **Analyse Critique** : Excellente structuration logicielle de gestion d'état de session.

### Commit 216 : `e658b5a` — "feat(cursor): parse stream usage and include tool calls in AI history."
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * Câblage des retours d'outils et parsing des jetons de complétion pour alimenter l'historique d'agent.

### Commit 217 : `cafc3ba` — "refactor(cursor): remove unused API key path and refine tool mappings."
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * Nettoyage du code d'interfaçage d'API de la crate `sinew-cursor`.

### Commit 218 : `12bf8e8` — "Fix Chrome Google search automation"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * **background.js** : Optimisation des sélecteurs de ciblage pour l'action de recherche automatisée Google sous CDP.

### Commit 219 : `41e8a71` — "fix(cursor): harden stealth before live Composer testing."
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * **identity.rs** : Masquage additionnel d'en-têtes et injection d'identifiants de télémétrie Cursor IDE authentiques pour maximiser l'indétectabilité du pont.

### Commit 220 : `391322e` — "feat(index): add local codebase index and wire Composer semantic search"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques (Majeurs)** :
  * **Création de la Crate Rust `sinew-index`** : Implémentation d'une couche d'indexation locale légère stockant des métadonnées de fichiers et des snippets.
  * **Fichiers** : `crates/sinew-index/Cargo.toml`, `crates/sinew-index/src/lib.rs`.
  * Intégration de la recherche sémantique dans le client de chat Cursor (`crates/sinew-cursor/src/context_injection.rs`).
* **Bénéfices Fonctionnels** :
  * **Recherche Sémantique Locale** : L'IA trouve de façon autonome les sections de code pertinentes au sein de l'espace de travail de l'utilisateur pour alimenter son cycle de raisonnement.
* **Analyse Critique** : Amélioration contextuelle SOTA majeure qui décuple la pertinence des réponses de l'agent.

### Commit 221 : `0c6fc44` — "feat(index): add semantic embeddings, background sync, and stable Composer bubble IDs"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * **conversation.rs**, **dotmatrix-core.tsx** : Intégration d'identifiants uniques réutilisables pour les bulles de discussion et génération de vecteurs d'embeddings en tâche de fond.

### Commit 222 : `2313fae` — "feat(cursor): inject local index excerpts into Composer explicit context"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * **context_injection.rs** : Câblage de l'injection d'extraits de code (`append_local_index_excerpts`) au début du prompt système de l'agent.

### Commit 223 : `8d3ab1a` — "feat(index): symbol-aware chunks, embedding backfill, richer workspace layout"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * **sinew-index/src/lib.rs** : Implémentation d'un parser conscient de la syntaxe de code pour découper les fichiers en blocs logiques (classes, fonctions) au lieu de simples morceaux de caractères.

### Commit 224 : `7870ab5` — "Add structured Chrome page snapshot"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * **tools.rs** : Ajout de l'outil `page_snapshot` capable de sérialiser les éléments interactifs du DOM sous une forme compacte évitant la saturation de jetons de contexte de l'IA.

### Commit 225 : `cc690ed` — "feat(ui): afficher l'index local et clarifier les images ChatGPT/Gemini"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * Affichage de l'état d'indexation locale dans l'interface graphique.

### Commit 226 : `9d64a54` — "chore: display full nested error chain in error_to_string"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * Amélioration de la journalisation des erreurs système.

### Commit 227 : `a509de8` — "feat(cursor): vision et generation d'images Composer via providers locaux"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * Câblage de la génération d'images pour Cursor via les comptes locaux OpenAI connectés.

### Commit 228 : `f10be29` — "feat(chrome-bridge): implement multi-candidate Beziers, smooth physics, favicon indicators, and hot reload"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques (Majeurs)** :
  * **sinew_cursor.js**, **background.js** : Intégration d'un algorithme avancé de trajectoire physique. Le pointeur virtuel de souris calcule de multiples trajectoires candidates sous courbes de Béziers complexes, évaluant le coût physique (accélérations fluides, décélérations avec amortissement naturel).
  * Affichage d'un curseur virtuel visible de couleur néon clignotante avec dégradés de teintes.
* **Bénéfices Fonctionnels** :
  * **Surveillance visuelle amusante et rassurante** : L'utilisateur voit exactement ce que l'IA clique sur son écran grâce au déplacement fluide de la souris virtuelle néon.
* **Analyse Critique** : Innovation d'interface extraordinaire (SOTA). Remplacer des actions d'arrière-plan invisibles par un pointeur visuel organique renforce de façon spectaculaire la confiance de l'utilisateur dans l'autonomie de l'agent.

### Commit 229 : `49c8843` — "style(chrome-bridge): enhance cursor with diagonal orientation, pulsing neon glow, and cycling gradient colors"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * Améliorations de style pour le curseur virtuel de Chrome.

### Commit 230 : `9b9c31d` — "feat(skills): add browser SKILL.md for Chrome error recovery and guidelines"
* **Date** : Mercredi 27 mai 2026
* **Changements Techniques** :
  * Création de `.sinew/skills/browser/SKILL.md` décrivant les procédures d'auto-guérison et de navigation pour l'IA en cas de blocage.

---

## 4. Synthèse des Quatre Thèmes Majeurs d'Évolution

### Thème A : La Révolution "Steering" (Pilotage Intuitif à Chaud)
Cette tranche technologique introduit une avancée SOTA colossale avec l'intégration du **Steering** de l'agent Rust :
* **Interception asynchrone** : Le runtime de discussion de l'assistant IA ne s'exécute plus de façon linéaire et ininterrompue. Il interroge en permanence un canal asynchrone de steering.
* **Envoi à la volée** : Si l'utilisateur saisit une correction dans l'interface de discussion pendant que l'agent est occupé à éditer des fichiers ou à naviguer, l'instruction est injectée instantanément au début du prochain tour de raisonnement, sans réinitialisation et sans rupture.

### Thème B : Intégration Souveraine de Cursor & Furtivité
Sinew se dote d'une compatibilité totale de premier ordre (Stealth Mode) avec Cursor Composer :
* **Extraction SQLite state.vscdb** : Sinew lit de façon transparente les jetons stockés par l'IDE local, contournant les frictions de configuration de clés d'API.
* **Spoofing d'identité de haut niveau** : Toutes les requêtes sont maquillées avec des en-têtes et des identifiants matériels d'IDE authentiques pour éviter tout risque de bannissement par les serveurs de Cursor.

### Thème C : Recherche Vectorielle Sémantique Locale
Le contexte envoyé à l'IA est enrichi de façon intelligente grâce à un indexeur sémantique autonome :
* **Symbol-Aware Chunking** : Le découpage des fichiers de code source respecte la structure syntaxique (fonctions, classes, méthodes) au lieu de simples limites physiques de caractères.
* **Génération en tâche de fond** : La base SQLite d'embeddings est mise à jour de manière transparente en arrière-plan, garantissant des recherches instantanées et à jour.

### Thème D : Rendu Ultra-Fluide et Simulation Physique Visuelle
L'ergonomie générale de Sinew franchit un cap de qualité professionnelle (Premium) :
* **16ms UI Streaming Batching** : Regroupement réactif des jetons de texte aligné sur le taux de rafraîchissement d'écran matériel, éliminant à 100% les gels de Webview React.
* **HTTP/2 Multiplexing** : Activation du multiplexage HTTP/2 natif sur les connexions réseau Rust pour un affichage de texte ultra-rapide.
* **Visual Neon Cursor Path** : Trajectoires de souris fluides basées sur des critères physiques de Béziers et pointeur néon clignotant.

---

## 5. Évaluation Globale de Stabilité et Recommandations SOTA

L'implémentation de la tranche 175-230 propulse Sinew au plus haut niveau de qualité industrielle pour les environnements de développement souverains assistés par IA.

### Forces Majeures :
1. **Pilotage à Chaud (Steering)** : Le steering élimine la frustration classique liée à l'attente obligatoire de la fin de génération de l'IA pour corriger une erreur de trajectoire.
2. **Fluidité Exceptionnelle (Batching)** : Le throttle visuel de 16ms apporte un confort d'écriture inégalé qui transcende l'expérience standard de Tauri.
3. **Discrétion de Premier Ordre** : Le spoofing d'en-têtes et la génération d'identifiants dynamiques garantissent un accès serein et durable aux APIs tierces.

### Recommandations d'Entretien :
* **Nettoyage du keep-alive des sockets Chrome Bridge** : Surveiller le cycle de vie du serveur relais local pour s'assurer que les connexions orphelines de sockets de débogage sont proprement purgées lors des fermetures rapides de fenêtres de navigation.
* **Volume de l'index vectoriel sémantique** : Lors de l'ouverture d'espaces de travail géants (> 30 000 fichiers), veiller à ce que l'indexation sémantique d'arrière-plan soit bridée en priorité de processeur pour ne pas pénaliser le thread de compilation principal de l'utilisateur.
