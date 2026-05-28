# Sinew — Fork de julienpiron.fr

Cette version a été optimisée en profondeur pour offrir une expérience utilisateur haut de gamme (SOTA), une autonomie maximale en arrière-plan, et des intégrations d'intelligence artificielle inégalées.

---

## 🖱️ Interface, Confort & Ergonomie (Premium UI)

* **🎨 Animation de démarrage premium :** Une animation de boot moderne, fluide et élégante à l'ouverture de l'application.
* **🧠 3 niveaux de réflexion :** Choix entre Détaillé, Compact ou Très compact pour configurer précisément la verbosité de l'IA et le masquage des détails techniques dans le chat.
* **📌 Question collante (Sticky Question) :** La question en cours de traitement reste épinglée en haut de l'écran pendant que vous faites défiler le fil de discussion.
* **🖱️ Menu clic droit interactif :**
  * **Sur les onglets :** Clic droit pour fermer l'onglet (raccourci `Ctrl+F4`), les autres ou tous les onglets situés à droite, copier le chemin (absolu ou relatif) ou révéler dans le Finder/Explorateur.
  * **Sur les fichiers dans le chat :** Clic droit direct pour ouvrir le fichier dans l'éditeur, le révéler dans le dossier système ou l'exécuter dans le terminal.
  * **Sur l'arbre des fichiers (File Tree) :** Option d'exécution directe au clic droit.
* **🔎 Polices dynamiques ajustables :** Boutons tactiles réactifs (`+` et `-`) dans les options pour ajuster instantanément à chaud la taille du texte de l'éditeur de code Monaco et du chat de l'IA.
* **🌐 Version française complète :** L'interface entière et toutes les actions de l'application s'adaptent automatiquement en français ou en anglais selon vos préférences.
* **📋 Sélection et copie libre :** Déblocage de la sélection et copie de texte directement dans le fil de discussion.
* **📏 Démarcation visuelle du panneau de configuration :** Ajout d'une ligne de séparation verticale élégante et nette à gauche du panneau de configuration pour structurer le menu d'onglets des paramètres.

---

## 💾 Autonomie, Sauvegarde & Robustesse Système

* **💾 Sauvegarde automatique (Auto-Save SOTA) :** Enregistrement automatique et transparent en arrière-plan 1,5 seconde après l'arrêt de la frappe. Activable ou désactivable d'un clic dans vos options.
* **📦 Mode Sandbox :** Lancement de l'application en un clic sans aucun projet ouvert pour tester l'IA ou utiliser les outils MCP de manière isolée.
* **☁️ Synchro OneDrive & SQLite automatique :** Synchronisation transparente de vos conversations, configurations de projets et bases de données SQLite entre vos différents ordinateurs.
* **⚡ Zéro popup console Windows :** Lancement asynchrone et silencieux de tous les outils et serveurs en arrière-plan sans aucune ouverture intempestive de fenêtres d'invite de commandes.
* **🏷️ Préfixe PC réel automatique :** Identification automatique du nom de la machine physique pour typer et sécuriser les configurations de conversation multi-PC.
* **🔑 Diagnostic Windows OAuth résilient :** Capture robuste de l'erreur réseau typique sous Windows (code 10013) et conseils clairs pour débloquer la connexion (WinNAT/HNS).
* **⚡ Diagnostic SOTA :** Vérification en un clic de l'état de santé, du PATH et des versions de tous vos outils de développement (Git, Python, Node, Cargo, etc.).
* **🔒 Écran de mises à jour sécurisé :** Verrouillage propre de l'interface pendant l'application des correctifs système pour éviter toute corruption de données.
* **🚀 Script de compilation OneDrive (`compil.ps1`) :** Automatisation de la génération de l'application et copie immédiate sur OneDrive pour un déploiement instantané sur vos PC.

---

## 🤖 Modèles d'IA, Comptes & Furtivité (AI Engine)

* **👥 Gestion Multi-comptes :** Connexion simultanée de plusieurs profils OpenAI secondaires avec bascule instantanée entre vos différentes clés et abonnements.
* **📊 Quotas en temps réel :** Visualisation dynamique de votre consommation (crédits / balance restante) sous forme de barres de progression et pastille live dans le chat.
* **🤖 Routage & Résilience Google Antigravity SOTA :** Réparation, optimisation et routage intelligent de vos requêtes vers les modèles Google les plus performants.
* **⚡ Optimisation de vitesse Gemini :** Streaming et requêtes ultra-rapides pour les modèles Gemini, basés sur l'architecture réseau optimisée de Google Antigravity.
* **🔥 Incorporation d'Opus par Google :** Intégration de Claude 3.5 Opus via les abonnements professionnels Google.
* **🧭 Système Pending/Steering pour Influencer :** Un vrai système d'interception et de guidage pour orienter, corriger ou ajouter des instructions en temps réel en cours de génération (Pending/Steering).
* **🔍 Indexation sémantique locale vectorielle :** Indexation et recherche vectorielle haute-performance effectuée localement sur votre machine avec badge d'état interactif dans la barre latérale.
* **🤖 Intégration expérimentale de Composer 2.5 :** Prise en charge du protocole haute-performance `agent.v1` de Cursor Composer, avec masquage automatique du sélecteur d'intelligence inutile pour une interface épurée.
* **🛡️ Sécurité & Furtivité WebSocket :** Spoofing d'empreinte réseau avancé pour éliminer tout risque de détection ou de blocage sur les flux de ChatGPT.

---

## 🔌 Extensions & Ponts locaux (MCP & Bridge)

* **🌐 Extension Chrome nouvelle génération :** Pilotage d'actions de navigation ultra-stables en natif Rust avec mouvements et clics à vitesse humaine (mouvements Beziers, physique fluide) et mode silencieux.
* **🌐 Réparation Chrome en un clic :** Bouton bleu de configuration automatique si le pont Chrome ne répond pas sur un nouveau PC.
* **📦 Empaquetage des ressources Tauri :** Le pont local et l'extension Chrome sont intégrés directement au sein de l'installateur compilé (MSI/EXE).
* **🛠️ Outils Rust optimisés :** Outils haute-performance (`list_dir` et `delete_file`) intégrés nativement à l'agent de workspace.
* **🛠️ Diagnostics Monaco en temps réel :** Remontée automatique des lints et erreurs de compilation de l'éditeur de code à l'IA en temps réel.
* **🧠 Logs ultra-compacts :** Nettoyage automatique du contexte de discussion pour éliminer le bruit et optimiser la consommation de jetons.
* **🔍 Laboratoire réseau MITM :** Outils de débogage et d'ingénierie inverse intégrés pour inspecter le trafic chiffré des outils IA.
