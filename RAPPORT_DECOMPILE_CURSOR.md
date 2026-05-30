# Rapport d'Analyse : Concepts Architecturaux Clés de Cursor (SOTA)

Ce rapport présente l'analyse des extensions décompilées de Cursor et détaille comment adapter ces concepts de pointe au sein de **Sinew**.

---

## 1. L'Espace de Travail Fantôme (Shadow Workspace)
* **Le Concept :** C'est un *laboratoire d'essai* en arrière-plan.
* **Fonctionnement dans Cursor :** Un serveur local autonome (`aiserver.v1.ShadowWorkspaceService`) s'exécute sur une prise de communication (socket/named pipe). Il maintient une copie temporaire et propre des fichiers pour lancer des vérifications et des compilations silencieuses (comme des relectures de code) sans toucher aux fichiers ouverts de l'utilisateur.
* **Intégration SOTA pour Sinew :** 
  * Créer un espace temporaire caché dans lequel Sinew copie les modifications en cours.
  * Lancer les outils de diagnostic en tâche de fond sur cet espace pour remonter les alertes de relecture (linters) sans interrompre la saisie de l'utilisateur.

---

## 2. Système d'Indexation et de Recherche (Retrieval & Merkle)
* **Le Concept :** Une *bibliothèque avec un catalogue intelligent par fiches cartonnées*.
* **Fonctionnement dans Cursor :** 
  * **Arbre généalogique (Merkle Tree) :** Chaque dossier et fichier possède une signature unique calculée à partir de son contenu. L'application compare les branches de cet arbre avec le serveur central pour envoyer uniquement les pages modifiées, économisant temps et connexion réseau.
  * **Empreintes digitales (Simhash) :** Un vecteur de similarité permet de regrouper et retrouver instantanément des morceaux de code identiques ou proches.
  * **Fichiers de tri :** Un fichier `.cursorindexingignore` permet d'exclure les dossiers trop lourds ou inutiles du catalogue.
* **Intégration SOTA pour Sinew :**
  * Implémenter un système d'empreinte Merkle local pour ne synchroniser ou réindexer que ce qui a changé.
  * Adopter le format de fichier standardisé `.sinewignore` ou `.cursorindexingignore` pour le respect des règles d'exclusion de l'utilisateur.

---

## 3. Automatisation de Navigateur (MCP & Contrôle Web)
* **Le Concept :** Un *œil déporté dans le navigateur*.
* **Fonctionnement dans Cursor :** L'assistant pilote un onglet de navigation intégré via le protocole de débogage de Chrome (CDP). Il utilise des outils dédiés (`browser_click`, `browser_type`, `browser_snapshot`) qui analysent la page :
  * Si un bouton est masqué par un bandeau ou une fenêtre surgissante (modal), l'assistant reçoit un avertissement clair ("bouton bloqué par un élément visuel, fermez d'abord la fenêtre") plutôt que de cliquer dans le vide.
  * Des captures d'écran et des résumés simplifiés de la page sont envoyés à l'intelligence artificielle pour l'aider à s'orienter.
* **Intégration SOTA pour Sinew :**
  * Connecter le serveur MCP Chrome existant de Sinew à une interface visuelle dédiée (Webview).
  * Enrichir les retours d'erreurs d'interaction (clics bloqués, formulaires invisibles) avec des conseils de résolution automatiques pour l'IA.

---

## 4. Boucle de l'Agent et Sécurité (Agent Loop & Approvals)
* **Le Concept :** Un *assistant de chantier* sous haute surveillance.
* **Fonctionnement dans Cursor :** 
  * **Le moteur en arrière-plan (Daemon Worker) :** Un programme indépendant tourne silencieusement pour exécuter les tâches complexes.
  * **La cabine de commande (Pseudoterminal) :** L'agent lance des commandes dans des consoles virtuelles isolées mais visibles par l'utilisateur si nécessaire.
  * **Le droit de regard (Approvals) :** L'agent doit obligatoirement obtenir le feu vert de l'utilisateur (fenêtre de décision) avant d'écrire un fichier, de le supprimer ou d'exécuter une commande système sensible.
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
