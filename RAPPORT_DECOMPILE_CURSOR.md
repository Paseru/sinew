# Rapport d'Analyse : Concepts Architecturaux Clés de Cursor (SOTA)

Ce rapport présente une analyse approfondie des extensions décompilées de Cursor (`cursor-agent-exec`, `cursor-agent-worker`, `cursor-commits`, `cursor-shadow-workspace`, `cursor-retrieval`, `cursor-browser-automation`, `cursor-mcp`) et détaille comment adapter ces concepts de pointe au sein de **Sinew**.

---

## 1. L'Espace de Travail Fantôme (Shadow Workspace)
* **La Métaphore :** La *feuille de calque d'architecte*. Au lieu de dessiner directement sur les plans originaux, l'architecte pose un calque transparent (l'espace fantôme) pour esquisser des modifications. Si le calque est validé (pas d'erreurs de construction), les lignes sont reportées sur le plan réel.
* **Fonctionnement dans Cursor :**
  * **Architecture Client/Serveur IPC locale :** Un serveur gRPC autonome (`aiserver.v1.ShadowWorkspaceService`) tourne en tâche de fond et communique via un canal de communication local (Unix Socket ou Named Pipe sous Windows : `socketPath`).
  * **Vérification de code à la volée (Delta Linting) :** La fonction centrale `GetLintsForChange` reçoit en entrée un descriptif de modification virtuelle (`GetLintsForChangeRequest.File` avec `{ relative_workspace_path, initial_content, final_content }`). Sans jamais écrire le fichier sur le disque physique de l'utilisateur, le serveur lance les outils de relecture (linters) et renvoie un rapport complet des erreurs (`GetLintsForChangeResponse.Lint`) et des suggestions d'auto-correction (`QuickFix.Edit`).
* **Intégration SOTA pour Sinew :**
  * Implémenter un démon de diagnostic en arrière-plan qui écoute sur un canal nommé (Named Pipe Windows).
  * Permettre à l'intelligence artificielle de tester ses modifications de code dans un fichier virtuel temporaire et de recevoir immédiatement les diagnostics du compilateur avant de valider l'écriture réelle sur le disque.

---

## 2. Système d'Indexation et de Recherche (Retrieval & Merkle)
* **La Métaphore :** Le *registre de signatures et détecteur de similitudes*. Pour vérifier si des livres d'une bibliothèque ont été modifiés, on ne relit pas chaque page. On compare juste le code-barres unique de chaque carton de livres (les nœuds d'un arbre de Merkle).
* **Fonctionnement dans Cursor :**
  * **Comparaison incrémentale par arbre de Merkle :** Le client natif Rust `@anysphere/file-service` génère localement un arbre de hachage de la structure de fichiers (Merkle Tree). Par exemple, avec `SyncMerkleSubtreeV2Request`, le client et le serveur s'échangent les signatures des répertoires pour repérer instantanément et uniquement les fichiers modifiés, évitant des analyses globales coûteuses.
  * **Regroupement par similarité (Simhash) :** Pour la recherche globale et la détection de doublons, les fichiers sont convertis en signatures numériques condensées de taille fixe. Les documents proches partagent des signatures similaires, ce qui permet de retrouver des exemples de code connexes dans le projet.
  * **Recherche native ultra-rapide (GrepClient) :** L'extension intègre un moteur de recherche natif Rust pilotant l'outil de fouille `ripgrep` directement en mémoire, ce qui permet de scanner des millions de lignes de code en quelques millisecondes.
* **Intégration SOTA pour Sinew :**
  * Utiliser un système de signatures (hachage Merkle) pour le catalogue local SQLite afin d'éviter de réindexer les fichiers non modifiés au démarrage.
  * Intégrer un filtre d'exclusion global respectant les fichiers `.sinewignore` ou `.cursorindexingignore`.

---

## 3. Automatisation de Navigateur (MCP & Contrôle Web)
* **La Métaphore :** La *télécommande de drone avec caméra*. Plutôt que de lancer une lourde voiture télécommandée autonome contenant son propre groupe électrogène (moteurs Puppeteer ou Playwright complets), l'assistant utilise le drone léger de l'éditeur pour prendre des photos et lui envoyer des consignes de pilotage simples.
* **Fonctionnement dans Cursor :**
  * **Délégation légère de l'affichage (WebView/CDP) :** L'extension `cursor-browser-automation` n'embarque pas de navigateur lourd. Elle communique avec la vue intégrée de l'éditeur (`cursor.browserView.navigate`, `cursor.browserView.sendCDPCommand`) via le protocole standard Chrome DevTools (CDP).
  * **Cibles par étiquettes simplifiées (`browser_snapshot`) :** Avant toute interaction, l'assistant demande une capture de la structure visible de la page. Le navigateur génère un arbre simplifié où chaque élément interactif reçoit une étiquette simple (ex: `@ref1`, `ref=2`). L'assistant clique ou écrit sur ces références, évitant de casser l'action avec des chemins complexes (sélecteurs CSS/XPath fragiles).
  * **Gestion fine des échecs d'interaction :** Si le clic échoue car un élément visuel (comme une fenêtre modale ou un bandeau d'acceptation de cookies) bloque le bouton, le navigateur renvoie une erreur descriptive explicite ("le clic va toucher un élément modal, fermez-le d'abord") au lieu de renvoyer une erreur technique muette.
  * **Correctif de rafraîchissement OAuth concurrent :** Un correctif est appliqué au SDK MCP pour éviter que plusieurs connexions en parallèle ne demandent le rechargement simultané des jetons d'accès (renvoi de l'erreur `SiblingAlreadyRefreshedError` pour aligner les instances).
* **Intégration SOTA pour Sinew :**
  * Exploiter le pont local de contrôle Chrome existant de Sinew en remplaçant les sélecteurs CSS complexes par un système d'étiquetage temporaire d'éléments interactifs dans le rendu visuel.
  * Rendre les erreurs de navigation interactives (boutons masqués, pages non chargées) intelligentes et explicatives pour que l'IA sache comment s'adapter (ex: faire défiler la page, fermer une popup).

---

## 4. Boucle de l'Agent et Sécurité (Agent Loop & Approvals)
* **La Métaphore :** Le *robot de chantier avec garde-fou*. Le robot effectue ses tâches pas à pas sous l'œil attentif d'un superviseur qui valide ses demandes d'outils et l'arrête s'il commence à tourner en rond.
* **Fonctionnement dans Cursor :**
  * **Boucle de contrôle orchestrée (`runTurnLoop`) :** La boucle exécute pas à pas les instructions du modèle jusqu'à une limite fixée (`maxSteps`). À chaque étape, elle rassemble les outils disponibles, injecte les règles personnalisées de l'utilisateur (fichiers `.cursorrules`) et appelle le modèle.
  * **Détection de changement de mode (Nudges) :** Si l'utilisateur ou l'assistant change de mode de travail en cours de route, la boucle le détecte et injecte un message d'orientation invisible dans le fil de discussion pour réaligner le comportement du modèle.
  * **Rappels de relecture et garde-fous (Forced Reflection & Reminders) :** 
    * Si l'assistant enchaîne trop d'étapes sans progresser (ex: plus de 10 étapes d'exécution), la boucle injecte un avertissement système obligatoire : `<system_reminder>You MUST now use the Reflect tool...</system_reminder>`.
    * En cas d'échecs répétés sur un outil, des instructions correctives sont insérées dynamiquement dans le retour d'information de l'outil (`experimental_content`) sans polluer l'historique visuel du chat.
  * **Interactions asynchrones (Ask Question Queue) :** Si l'assistant a besoin d'une décision de l'utilisateur (ex: outil `ask_question`), la boucle se met en pause et écoute le récepteur d'actions (`conversationActionReceiver`). Dès que l'utilisateur répond, la réponse est injectée directement comme un résultat d'outil et la boucle reprend instantanément.
* **Intégration SOTA pour Sinew :**
  * Mettre en place un régulateur de boucle dans Sinew pour surveiller le nombre d'étapes d'exécution d'affilée et forcer une phase de réflexion (Reflect) en cas de dérive ou d'erreurs répétées.
  * Injecter les alertes et les conseils d'auto-correction directement dans le retour de l'outil fautif plutôt que de créer un nouveau tour de discussion utilisateur verbeux.

---

## 5. Génération de Commits et Métriques (Diff & Commits)
* **La Métaphore :** Le *secrétaire de rédaction de journal de bord*. Il consulte les notes de la journée, compare le travail fini avec le point de départ, et rédige un résumé clair et lisible en français simple pour l'équipe.
* **Fonctionnement dans Cursor :**
  * **Extraction intelligente de contexte (`GetCurrentIndexAndRecentCommits`) :** L'extension interroge l'API Git de VS Code (`vscode.git`) pour récupérer la liste des fichiers modifiés et calcule leur différence (diff) par rapport au commit de référence `HEAD`. Si le dépôt est vide, elle compare avec la signature vide universelle (`4b825dc642cb6eb9a060e54bf8d69288fbee4904`).
  * **Apprentissage du style de rédaction historique :** Elle charge les 10 derniers messages de validation (`s.log({maxEntries: 10})`) du projet. Ces anciens messages et les différences de code sont envoyés au modèle pour qu'il rédige le nouveau message de validation dans le même ton et style d'écriture que l'utilisateur.
  * **Suivi de télémétrie des requêtes :** Pendant l'exécution de l'agent, un module de suivi enregistre la taille des fichiers, le nombre de lignes ajoutées/supprimées et les détails des modifications sous forme de fichiers `metadata.json` afin de mesurer la qualité des actions produites.
* **Intégration SOTA pour Sinew :**
  * Proposer une fonction de rédaction automatique de commit Git dans le panneau de contrôle de Sinew.
  * Récupérer les 10 derniers commits locaux pour adapter automatiquement le ton de rédaction de l'IA à celui de l'utilisateur (ex: commits en français court, impératif, ou descriptif).

