# Rapport d'Analyse : Concepts Architecturaux Clés de Cursor (SOTA)

Ce rapport présente l'analyse des extensions décompilées de Cursor et détaille comment adapter ces concepts de pointe au sein de **Sinew**.

---

## 1. L'Espace de Travail Fantôme (Shadow Workspace)
* **Le Concept :** C'est un *laboratoire d'essai* en arrière-plan.
* **Fonctionnement dans Cursor :** Un serveur local gRPC autonome (`aiserver.v1.ShadowWorkspaceService`) s'exécute en arrière-plan et communique via une prise de communication locale (IPC/socket ou named pipe sous Windows : `socketPath`).
  * Il maintient une copie temporaire et propre des fichiers pour exécuter des vérifications, des compilations silencieuses et des diagnostics.
  * La fonction clé `GetLintsForChange` reçoit les modifications de fichiers (contenu initial, lignes et colonnes impactées) et retourne immédiatement les alertes de relecture (`Lint`) ainsi que les suggestions de corrections rapides (`QuickFix`).
* **Intégration SOTA pour Sinew :** 
  * Créer un espace temporaire caché dans lequel Sinew copie les modifications en cours.
  * Lancer les outils de diagnostic en tâche de fond sur cet espace pour remonter les alertes de relecture (linters) sans interrompre la saisie de l'utilisateur.

---

## 2. Système d'Indexation et de Recherche (Retrieval & Merkle)
* **Le Concept :** Une *bibliothèque avec un catalogue intelligent par fiches cartonnées*.
* **Fonctionnement dans Cursor :** 
  * **Arbre généalogique (Merkle Tree) :** Chaque dossier et fichier possède une signature unique calculée à partir de son contenu. L'application utilise un protocole d'échange (`SyncMerkleSubtreeV2Request`) et compare les branches de cet arbre avec le serveur central (`getSubtreeHash(".")`) pour envoyer uniquement les pages modifiées, économisant temps et connexion réseau.
  * **Empreintes digitales (Simhash) :** Un vecteur de similarité de longueur fixe permet de regrouper et de détecter instantanément des morceaux de code identiques ou proches (détection de doublons, recherche de contexte global).
  * **Fichiers de tri :** Un fichier `.cursorindexingignore` permet d'exclure les dossiers trop lourds ou inutiles du catalogue.
  * **Recherche locale :** Un service de recherche local (`Cursor Grep Service`) pilote directement l'outil ultra-rapide `ripgrep` combiné avec des requêtes sémantiques.
* **Intégration SOTA pour Sinew :**
  * Implémenter un système d'empreinte Merkle local pour ne synchroniser ou réindexer que ce qui a changé.
  * Adopter le format de fichier standardisé `.sinewignore` ou `.cursorindexingignore` pour le respect des règles d'exclusion de l'utilisateur.

---

## 3. Automatisation de Navigateur (MCP & Contrôle Web)
* **Le Concept :** Un *œil déporté dans le navigateur*.
* **Fonctionnement dans Cursor :** 
  * **Serveur MCP `cursor-ide-browser` :** L'assistant pilote un onglet de navigation intégré via le protocole de débogage de Chrome (CDP).
  * **Délégation Légère :** Plutôt que de lancer des moteurs lourds (comme Puppeteer ou Playwright autonomes) exigeant de gros binaires, l'extension délégue le rendu graphique et la navigation au moteur interne de l'éditeur en appelant des commandes VS Code (`cursor.browserView.navigate`, `cursor.browserView.newTab`, `cursor.browserView.sendCDPCommand`).
  * **Correctif de refresh OAuth concurrent :** Cursor applique un correctif (patch) à la bibliothèque `@modelcontextprotocol/sdk` pour coordonner les rechargements de jetons d'accès. Grâce à `prepareForRefresh` et au renvoi de l'erreur `SiblingAlreadyRefreshedError`, les instances concurrentes s'alignent sur le jeton fraîchement mis à jour au lieu de lancer des autorisations redondantes.
  * **Outils d'interaction :** Des outils dédiés (`browser_click`, `browser_type`, `browser_snapshot`) analysent la page. Si un bouton est masqué par un bandeau ou une fenêtre surgissante (modal), l'assistant reçoit un avertissement clair ("bouton bloqué par un élément visuel, fermez d'abord la fenêtre") plutôt que de cliquer dans le vide.
* **Intégration SOTA pour Sinew :**
  * Connecter le serveur MCP Chrome existant de Sinew à une interface visuelle dédiée (Webview).
  * Adopter la délégation de commandes d'affichage légères pour éviter les instanciations de navigateurs superflues.
  * Enrichir les retours d'erreurs d'interaction (clics bloqués, formulaires invisibles) avec des conseils de résolution automatiques pour l'IA.

---

## 4. Boucle de l'Agent et Sécurité (Agent Loop & Approvals)
* **Le Concept :** Un *assistant de chantier* sous haute surveillance.
* **Fonctionnement dans Cursor :** 
  * **Le moteur en arrière-plan (Daemon Worker) :** Un programme indépendant (`cursor-agent-worker` / `cursor-agent-exec`) tourne silencieusement pour exécuter les tâches complexes.
  * **Sérialisation Protobuf :** Les communications et appels d'outils (`ShellToolCall`) transitent sous forme de flux Protobuf optimisés.
  * **La cabine de commande (Pseudoterminal) :** L'agent lance des commandes dans des consoles virtuelles isolées mais visibles par l'utilisateur si nécessaire.
  * **Le droit de regard (Approvals) :** L'agent doit obligatoirement obtenir le feu vert de l'utilisateur (fenêtre de décision) avant d'écrire un fichier, de le supprimer ou d'exécuter une commande système sensible (`runCommand`, validation d'autorisations).
* **Intégration SOTA pour Sinew :**
  * Renforcer l'isolation des tâches en arrière-plan via des consoles virtuelles.
  * Mettre en place un système de validation explicite (popups ou boutons de chat) pour toutes les actions modifiant le disque ou le réseau.

---

## 5. Génération Automatique de Commits (Diff & Commits)
* **Le Concept :** Un *secrétaire de chantier* qui tient le journal de bord.
* **Fonctionnement dans Cursor :** Un module dédié (`cursor-commits`) utilise un assistant en tâche de fond (thread séparé) pour calculer la différence précise entre l'ancien et le nouveau code. Il rassemble ces informations (diff de lignes, contexte de la branche, notes de travail) et demande à l'intelligence artificielle de rédiger un mémo clair et compréhensible sans jargon technique.
* **Intégration SOTA pour Sinew :**
  * Exécuter les comparaisons de fichiers dans un sous-programme pour ne pas ralentir l'interface utilisateur.
  * Utiliser un modèle de rédaction de messages de validation (commits) automatiques rédigés en français simple.
