# Sinew

<p align="center">
  <img src=".github/assets/hero.png" alt="Sinew — the coding harness you shape" width="100%" />
</p>

**Sinew** est un harnais de développement IA de bureau hautement personnalisable sous Tauri 2, React, Rust, Monaco et xterm.js. Contrairement aux autres solutions verrouillées, Sinew vous permet de connecter vos propres abonnements ou clés et de modeler précisément les outils et invites vus par l'agent.

---

## 🚀 Fonctionnalités Majeures (Fork Premium julienpiron.fr)

Cette version a été optimisée en profondeur pour offrir une expérience utilisateur haut de gamme (SOTA), une autonomie maximale en arrière-plan, et des intégrations d'intelligence artificielle inégalées.

### 🎨 Interface, Confort & Ergonomie (Premium UI)
* **Animation de démarrage premium :** Une animation de boot moderne, fluide et élégante à l'ouverture de l'application.
* **3 niveaux de réflexion :** Choix entre Détaillé, Compact ou Très compact pour configurer précisément la verbosité de l'IA et le masquage des détails techniques dans le chat.
* **Question collante (Sticky Question) :** La question en cours de traitement reste épinglée en haut de l'écran pendant que vous faites défiler le fil de discussion.
* **Menu clic droit interactif sur les onglets de l'éditeur :** Clic droit (ou `F10`) sur les onglets pour fermer l'onglet (raccourci `Ctrl+F4`), les autres, à sa droite ou tous, copier le chemin (absolu ou relatif) et révéler dans le Finder/Explorateur.
* **Menu clic droit d'exécution :** Clic droit sur les fichiers dans le chat et l'arbre des fichiers pour les ouvrir, les révéler ou les exécuter directement.
* **Polices dynamiques ajustables :** Boutons tactiles réactifs (`+` et `-`) dans les options pour ajuster instantanément à chaud la taille du texte de l'éditeur Monaco et du chat.
* **Version française complète :** L'interface entière et toutes les actions de l'application s'adaptent automatiquement en français ou en anglais.
* **Sélection et copie libre :** Déblocage de la sélection et copie de texte directement dans le fil de discussion du chat.
* **Démarcation visuelle :** Ligne de séparation verticale élégante à gauche du panneau de configuration des paramètres.
* **Découpage du bundle Vite (-80% de taille) :** Monaco Editor et xterm.js sont isolés dans des sous-lots séparés pour un chargement instantané de l'interface utilisateur.
* **Visualisation du plan d'action (Planning Board) :** Intégration d'un bloc dynamique interactif (`PlanningNextMoveBlock`) montrant en temps réel les prochaines étapes planifiées par le Swarm d'agents.
* **Aperçu d'image immersif (Lightbox) :** Visionneuse d'images de discussion immersive avec zoom à la molette de souris, déplacement panoramique, rotation, téléchargement et fermeture par clic extérieur.

### 💾 Autonomie, Sauvegarde & Robustesse Système
* **Sauvegarde automatique (Auto-Save SOTA) :** Enregistrement automatique et transparent en arrière-plan 1,5 seconde après l'arrêt de la frappe. Activable ou désactivable d'un clic dans vos options.
* **Mode Sandbox :** Lancement de l'application en un clic sans aucun projet ouvert pour tester l'IA ou utiliser les outils MCP de manière isolée.
* **Synchro OneDrive & SQLite automatique :** Synchronisation transparente de vos conversations, configurations de projets, jetons de connexion/clés d'authentification (`*-auth.json`, `*-device.json`, `*-stream-state.json`), fichiers d'apprentissage globaux (`errors_raw.json` et `instructions_consolidated.md`), et bases de données SQLite entre vos différents ordinateurs.
* **Zéro popup console Windows :** Lancement asynchrone et silencieux de tous les outils, serveurs MCP, commandes Git et diagnostics SOTA en arrière-plan sans aucune ouverture de fenêtres d'invite de commandes.
* **Préfixe PC réel automatique :** Identification automatique du nom de la machine physique pour typer et sécuriser les configurations de conversation multi-PC.
* **Diagnostic Windows OAuth résilient :** Capture robuste de l'erreur réseau typique sous Windows (code 10013) et conseils clairs pour débloquer la connexion (WinNAT/HNS).
* **Diagnostic SOTA :** Vérification en un clic de l'état de santé, du PATH et des versions de tous vos outils de développement (Git, Python, Node, Cargo, etc.).
* **Écran de mises à jour sécurisé (`UpdaterLockScreen`) :** Verrouillage de l'interface pendant l'application des correctifs système pour éviter tout conflit de fichiers ou corruption de base de données.
* **Script de compilation OneDrive (`compil.ps1`) :** Automatisation de la génération de l'application et copie immédiate sur OneDrive pour un déploiement instantané sur vos PC.
* **Active Turn Registry :** Moteur intelligent Rust qui suit les turns de l'agent en cours et assure une reprise instantanée du streaming.
* **Fiche de transmission structurée (Compaction d'IA) :** Compactage automatique du contexte lors du changement de fournisseur d'IA dans une fiche structurée reprenant le statut des fichiers modifiés, le relais des tâches et les diagnostics du linter.
* **Mode plein gaz adaptatif (`crates/sinew-index/src/store.rs`) :** Optimisation dynamique des performances de l'indexeur augmentant le cache et la lecture en mémoire lorsque la machine dispose d'un stockage SSD/NVMe.
* **Indexation locale parallèle SOTA :** Préparation et analyse des fichiers en parallèle répartie sur tous les coeurs de CPU disponibles via Rayon, avec détection immédiate et saut des fichiers inchangés grâce à leurs empreintes de taille et date.
* **Identification de projet universelle :** Association automatique des conversations au dépôt Git distant (remote origin URL) ou via un fichier d'identifiant unique `.sinew/project_id.txt` pour lier instantanément vos conversations d'un PC à un autre sans aucune action manuelle, avec détection, liaison et rafraîchissement dynamique des conversations provenant de PC alternatifs depuis les paramètres.
* **Gestion des mises à jour configurables :** Option à 3 choix (Bloquant, Notification, Désactivé) pour décider précisément du niveau de vérification des nouvelles versions de Sinew et empêcher l'écrasement de vos modifications.
* **Script de contrôle qualité unifié (`scripts/check.ps1`) :** Commande unique `npm run check` exécutant le build frontend, `cargo check`, les tests, `clippy` et les audits de dépendances en une seule opération.
* **Système d'apprentissage global transparent :** Chargement et injection automatique de la base d'instructions centralisées de l'utilisateur (`%LOCALAPPDATA%\Sinew\instructions_consolidated.md`) dans le prompt système de tous les agents pour l'ensemble des projets ouverts sur la machine.
* **Consolidation automatique de la mémoire :** Mécanisme au démarrage transformant automatiquement les erreurs répétées enregistrées dans `errors_raw.json` en règles d'apprentissage globales permanentes dans `instructions_consolidated.md` avec nettoyage du compteur d'erreurs.
* **Bouton de synchronisation forcée :** Ajout d'un bouton de synchronisation immédiate à la demande dans les paramètres pour déclencher manuellement la synchronisation du dossier OneDrive local.

### 🤖 Modèles d'IA, Comptes & Furtivité (AI Engine)
* **Gestion Multi-comptes OpenAI & Google Gemini :** Connexion simultanée de plusieurs profils OpenAI et Google Gemini secondaires avec bascule instantanée entre vos différentes clés, comptes et abonnements.
* **Quotas en temps réel :** Visualisation dynamique de votre consommation (crédits / balance restante) sous forme de barres de progression colorées adaptatives dans les options, et pastille live dans le chat.
* **Routage & Résilience Google Antigravity SOTA :** Réparation, de-surcharge réseau (erreur 503), routeurs de secours et transition transparente entre modèles avec résolution dynamique des identifiants d'appels d'outils (tool_call_id).
* **Optimisation de vitesse Gemini :** Streaming et requêtes ultra-rapides pour les modèles Gemini.
* **Incorporation de Claude Opus 4.8 & 4.6 :** Intégration complète de Claude Opus 4.8 (contexte 1M natif) et Claude Opus 4.6 via les abonnements professionnels Google.
* **Système Pending/Steering pour Influencer :** Un vrai système d'interception et de guidage pour orienter, corriger ou ajouter des instructions en temps réel sans blocage du flux de l'IA.
* **Indexation sémantique locale vectorielle :** Indexation et recherche vectorielle haute-performance effectuée localement sur votre machine avec commutateur d'activation directe (BETA) dans le panneau d'options.
* **Intégration de DeepSeek R1 & V3 :** Support complet de **DeepSeek V3** et de **DeepSeek R1** avec capture et rendu en temps réel du bloc de réflexion (*reasoning*) grâce à l'extraction du champ `reasoning_content` dans le chat.
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
* **Outils MCP de diagnostics Chrome avancés :** Intégration de nouveaux outils d'audit (`emulate_experience`, `lighthouse_audit`, `analyze_memory_leaks`) basés sur l'API CDP pour tester les performances, diagnostics Lighthouse et fuites mémoire en local.

---

## 🛠️ Fonctionnalités Fondamentales

### 🔄 Les Trois Modes
1. **Act (Action unique) :** Cycle classique question/action unique.
2. **Goal (Autonomie) :** L'agent boucle de manière autonome jusqu'à la résolution complète.
3. **Plan (Planification) :** L'agent explore le codebase, vous interroge et construit un plan d'action fonctionnel sans jargon technique.

### 💡 Intégration de prompts (`AGENTS.md` & `DESIGN.md`)
* **`AGENTS.md`** : Instructions de développement, conventions de projet et exigences globales injectées au prompt système.
* **`DESIGN.md`** : Système de design (couleurs, typographie, règles UI) injecté pour garantir la fidélité de l'interface.

### 🔗 Fournisseurs IA et Connexion Directe
* Connectez en parallèle : **Anthropic**, **OpenAI**, **Google**, **Kimi**, **Ollama**, et **OpenRouter**.
* **OAuth Direct (Sans surcoût)** : Utilisez vos abonnements grand public/professionnels existants pour vos appels IA, sans payer d'API au compteur.

### 📊 Suivi des Quotas
* Suivi des fenêtres de requêtes pour OpenAI/Codex, pourcentages d'utilisation pour Antigravity/Gemini et crédits restants pour OpenRouter.

### 🧰 Boîte à Outils Intelligente
* **`clean_context`** : L'agent nettoie lui-même son historique de discussion pour éliminer les recherches infructueuses et économiser le contexte sans casser le cache de prompt.
* **`bash` & `bash_input`** : Terminal interactif complet avec gestion de session persistante.
* **Outils Dédiés** (`read`, `glob`, `grep`, `edit_file`, `write_file`) basés sur *ripgrep* pour un signal minimal et structuré.
* **Lazy Loading via MCP & Skills** : Catalogue dynamique qui ne charge les schémas d'outils et de compétences qu'à la demande pour économiser les précieux jetons système.

### 👥 Swarm d'Agents (Peer-to-Peer)
* Collaborez avec 2 à 8 agents spécialisés en simultané.
* Coordination transparente via des rappels automatiques insérés directement dans le prompt système (état du tableau des tâches, messages partagés et modifications récentes).

### 🔍 Gestion du Contexte & Historique
* **Compaction manuelle et automatique** pour résumer l'historique quand la fenêtre de contexte s'approche de la saturation.
* **Rollback & Revert** : Revenez en arrière dans la discussion et restaurez (ou conservez) l'état des fichiers à n'importe quel message précédent grâce aux checkpoints de fichiers.

---

## 📦 Installation et Compilation

### Installation
Téléchargez l'installateur adapté à votre OS dans la section [Releases](https://github.com/Paseru/sinew/releases/latest) :
* **Windows** : `.exe` / `.msi`
* **macOS** : `.dmg`
* **Linux** : `.deb` / `.AppImage`

### Compilation depuis les sources
Nécessite **Node.js 20+** et **Rust 1.80+**.
```powershell
npm install
npm run tauri dev      # Lancer en mode développement
npm run tauri build    # Compiler le paquet de production
```

---

## 📄 Licence
Sous licence [MIT](./LICENSE).
