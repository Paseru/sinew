# 🛠️ Sinew — Fonctionnalités Personnalisées & Suggestions (Fork)

Ce document détaille l'ensemble des fonctionnalités développées pour ce fork personnel de **Sinew**. L'objectif est de conserver ces ajustements pour mon propre usage quotidien (à moins que le dépôt officiel ne propose une meilleure alternative à l'avenir), tout en les présentant de manière claire et modulaire afin que l'auteur original de **Sinew** puisse librement s'en inspirer ou réintégrer certaines d'entre elles dans le projet principal.

---

## 📌 Fonctionnalités du fork & Propositions amont

Afin de faciliter la lecture et l'éventuelle récupération du code, ces contributions sont classées selon les différentes étapes d'utilisation de l'application :

### 1. Écran d'accueil & Démarrage

#### 📂 Mode Bac à sable (Lancement sans dossier)
* **🔍 Constat** : L'obligation de devoir cibler ou créer un dossier projet peut freiner l'utilisateur souhaitant simplement tester l'IA, lui confier une tâche générale rapide, ou utiliser des outils globaux (comme le contrôle de navigateur Chrome via MCP) sans modifier son disque.
* **💡 Proposition** : Un bouton direct **`Use without folder (Sandbox)`** sur l'écran de bienvenue. Il instancie un répertoire temporaire sécurisé `.sinew-sandbox` dans le dossier utilisateur (affiché proprement sous le nom de *« Sans dossier »*).
* **🛠️ Fichiers concernés** : `src/components/Welcome.tsx`, `src/App.tsx`, `src-tauri/src/workspace.rs`

---

### 2. Interface de Chat & Expérience Utilisateur

#### ⚡ Bouton « Influencer » dans la File d'Attente (Prompt Queue)
* **🔍 Constat** : Le bouton pour envoyer un prompt mis en attente était matérialisé par une simple flèche peu visible et peu intuitive pour les nouveaux utilisateurs.
* **💡 Proposition** : Remplacement du bouton d'envoi par un badge distinctif affichant clairement le texte **`Influencer`** avec une icône de flèche vers le haut (`solar:arrow-up-bold`), clarifiant instantanément l'action d'injection dans la discussion en cours.
* **🛠️ Fichiers concernés** : `src/components/chat/TodoStrip.tsx`

#### 🚀 Clic droit et menu d'action sur les liens de fichiers locaux
* **🔍 Constat** : L'IA cite régulièrement des chemins de fichiers locaux dans ses réponses (ex: `installers/setup.exe` ou `src/main.rs`), mais ces chemins restaient statiques ou bloqués au clic.
* **💡 Proposition** : Capture dynamique des chemins de fichiers locaux dans les bulles de chat et intégration d'un menu contextuel au clic droit proposant trois actions système immédiates :
  * **Ouvrir dans l'éditeur** interne.
  * **Révéler dans l'Explorateur** de fichiers de l'OS.
  * **Exécuter / Lancer** directement (très utile pour tester des scripts ou des exécutables générés).
* **🛠️ Fichiers concernés** : `src/components/chat/Markdown.tsx`, `src-tauri/src/workspace.rs`

#### 📌 Question Collante (Sticky Question)
* **🔍 Constat** : Lors du défilement des longs historiques de discussion, l'utilisateur perd fréquemment de vue la formulation exacte de la question qu'il vient de poser.
* **💡 Proposition** : Fixer la dernière question de l'utilisateur sous forme de bandeau discret en haut de la fenêtre de chat lors du défilement. Un clic dessus permet de remonter instantanément et de façon fluide à la question d'origine.
* **🛠️ Fichiers concernés** : `src/components/chat/ChatPane.tsx`, `src/styles.css`

#### 📋 Copie de texte libre dans le Chat
* **🔍 Constat** : Certaines portions de l'interface bloquaient la sélection de texte au curseur, empêchant l'utilisateur de copier/coller facilement un bout de code ou de texte d'une bulle de discussion.
* **💡 Proposition** : Ajustement des styles CSS pour autoriser la sélection (`user-select: text`) sur les blocs appropriés sans perturber le drag-and-drop de la fenêtre.
* **🛠️ Fichiers concernés** : `src/styles.css`

---

### 3. Panneau des Options & Préférences (`Settings > Options`)

#### 🧠 Mode d'affichage à 3 niveaux (Réflexion Compacte / Densité d'information)
* **🔍 Constat** : Les longs blocs de réflexion (*thinking blocks*) ou les sorties massives d'exécution d'outils peuvent encombrer visuellement le chat.
* **💡 Proposition** : Ajouter un sélecteur à 3 niveaux dans les options générales permettant de régler finement la densité technique de l'affichage :
  * **Détaillé** : Affichage permanent et complet de toutes les étapes de réflexion et exécutions.
  * **Compact** : Les blocs de réflexion se replient automatiquement sous forme de bandeau résumé (ex: *Thinking (5.2s)*) dès que la génération est finie.
  * **Très compact** : Les blocs de réflexion s'affichent uniquement pendant l'écriture puis **disparaissent complètement** de l'historique une fois terminés, laissant un chat épuré.
* **🛠️ Fichiers concernés** : `src/components/SettingsPane.tsx`, `src/components/chat/AIThinkingBlock.tsx`, `src/lib/ipc.ts`

#### 🔄 Synchronisation OneDrive différentielle & Suivi des suppressions
* **🔍 Constat** : L'utilisation de Sinew sur plusieurs PC nécessite de conserver ses discussions et configurations à jour. De plus, une simple copie de base de données à la fermeture peut écraser des données ou faire réapparaître des conversations supprimées sur l'autre ordinateur.
* **💡 Proposition** : Implémenter un système de synchronisation automatique et transparente via l'espace OneDrive de l'utilisateur. Il inclut un moteur de fusion différentielle SQLite au démarrage et à la fermeture pour conserver les données des deux postes, complété par un suivi rigoureux des IDs de discussions supprimées pour éviter toute réapparition intempestive.
* **🛠️ Fichiers concernés** : `src-tauri/src/lib.rs`, `src/components/SettingsPane.tsx`

#### 🤖 Mode "Power User" (Zéro Jargon & Git Automatique)
* **🔍 Constat** : Les utilisateurs non-développeurs ou moins techniques peuvent se sentir dépassés par les explications trop complexes (jargon) ou la manipulation directe de Git (pull, commit, push) lors de modifications.
* **💡 Proposition** : Un mode activable qui adapte les instructions système de l'IA :
  * **Communication simplifiée** : L'IA produit des réponses très concises, orientées action et sans jargon.
  * **Maintenance automatisée** : L'IA orchestre elle-même les vérifications et commandes Git (pull, commit, push) en arrière-plan suite à ses modifications, déchargeant l'utilisateur de cette gestion.
* **🛠️ Fichiers concernés** : `src-tauri/src/state.rs`, `src-tauri/src/turns.rs`

#### 🌐 Interface en Français (Traduction progressive)
* **🔍 Constat** : Disposer d'une interface dans sa langue maternelle améliore considérablement le confort de navigation et d'usage quotidien pour le grand public.
* **💡 Proposition** : Introduction d'un sélecteur de langue dynamique (English / Français) traduisant les panneaux clés de configuration, boutons et infobulles, avec un rechargement propre et sécurisé de la fenêtre après confirmation.
* **🛠️ Fichiers concernés** : `src/lib/locale.ts`, `src/components/SettingsPane.tsx`

#### ⚡ Diagnostic Système SOTA (State of the Art)
* **🔍 Constat** : Diagnostiquer pourquoi un outil système indispensable (comme Git, Node, Cargo ou Ripgrep) est inaccessible ou mal configuré sur la machine de l'utilisateur peut être fastidieux.
* **💡 Proposition** : Ajouter un module de diagnostic en un clic qui teste en temps réel la présence, le chemin d'accès absolu et la version exacte de tous les outils et compilateurs système essentiels (Git, Node/Npm, Rust/Cargo/Rustc, Python/Pip, Ripgrep).
* **🛠️ Fichiers concernés** : `crates/sinew-app/src/check_sota.rs`, `src/components/SettingsPane.tsx`

#### 📱 Interface responsive via Container Queries
* **🔍 Constat** : Sur des tailles d'écrans intermédiaires ou en mode côte à côte, le panneau des options générales pouvait être tronqué ou difficile à lire en raison de Media Queries basées sur la taille de l'écran global.
* **💡 Proposition** : Remplacer l'approche responsive classique par des *Container Queries* CSS. Les différents modules s'étendent ainsi de façon fluide à 100% de la largeur disponible de leur conteneur parent.
* **🛠️ Fichiers concernés** : `src/styles.css`

---

### 4. Panneaux de Connecteurs (`Settings > Providers & MCP`)

#### 🌐 Sinew Chrome Bridge (Contrôle résilient du navigateur)
* **🔍 Constat** : Le pont de navigation (MCP) utilisant des scripts Python ou des ports rigides était sujet à des plantages de processus ou des conflits de ports réseau (`EADDRINUSE`) si plusieurs instances tournaient.
* **💡 Proposition** : Améliorer en profondeur l'architecture du serveur MCP local :
  * **Wrapper Natif Rust** : Remplacement des scripts de démarrage par un binaire compilé (`native-host-wrapper.exe`) pour une stabilité système absolue.
  * **Auto-résolution des ports** : Gestion transparente et dynamique des conflits de ports réseau.
  * **Simulations "Humaines"** : Intégration de mouvements de souris réalistes et clics via le protocole CDP pour naviguer sur le web et contourner les blocages de sites anti-robots de manière fiable, sans requérir de clés d'API.
* **🛠️ Fichiers concernés** : `sinew-chrome-bridge/mcp_server.js`, `sinew-chrome-bridge/native-host-wrapper/`

#### 👥 Multi-comptes OpenAI
* **🔍 Constat** : Les utilisateurs possédant à la fois un compte professionnel et un compte personnel (ou plusieurs tokens d'accès) devaient se déconnecter et ressaisir leurs clés à chaque changement.
* **💡 Proposition** : Ajouter un bouton **`+`** à côté du fournisseur OpenAI pour enregistrer plusieurs jetons d'accès distincts (OpenAI 2, OpenAI 3, etc.) et basculer instantanément de l'un à l'air depuis l'interface sans interruption.
* **🛠️ Fichiers concernés** : `src/components/SettingsPane.tsx`
