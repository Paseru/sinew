# Rapport d'Analyse : Concepts Architecturaux Clés de Cursor (SOTA)

Ce rapport présente une analyse approfondie des extensions décompilées de Cursor (`cursor-agent-exec`, `cursor-agent-worker`, `cursor-commits`, `cursor-shadow-workspace`, `cursor-retrieval`, `cursor-browser-automation`, `cursor-mcp`) et détaille comment adapter ces concepts de pointe au sein de **Sinew**.

---

## 1. L'Espace de Travail Fantôme (Shadow Workspace)
* **La Métaphore :** La *feuille de calque d'architecte*. Au lieu de dessiner directement sur les plans originaux, l'architecte pose un calque transparent (l'espace fantôme) pour esquisser des modifications. Si le calque est validé (pas d'erreurs de construction), les lignes sont reportées sur le plan réel.
* **Fonctionnement dans Cursor :**
  * **Architecture Client/Serveur IPC locale :** Un serveur gRPC autonome (`aiserver.v1.ShadowWorkspaceService`) tourne en tâche de fond et communique via un canal de communication local (Unix Socket ou Named Pipe sous Windows : `socketPath`). L'éditeur fournit l'implémentation du service à l'extension via le fournisseur de serveur de l'extension host (`e.cursor.registerShadowServerProvider`), tandis que les processus secondaires s'y connectent via le fournisseur de client (`e.cursor.registerShadowClientProvider`).
  * **Capture de clichés par plomberie Git (Git Plumbing Snapshot) :** Pour copier l'espace de travail de l'utilisateur vers l'agent sans salir son dossier, Cursor utilise les commandes bas niveau de Git (`git read-tree HEAD`, `git add -A -- .`, `git write-tree`, `git commit-tree`, `git update-ref`). En utilisant un fichier d'index temporaire via la variable d'environnement `GIT_INDEX_FILE`, il fige l'état de l'arbre sans altérer l'index actif de l'utilisateur, puis pousse cet instantané vers une branche volante (ex : `cursor/cloud-agent-...`).
  * **Configuration TextMate isolée pour les dossiers de travail :** Les espaces de travail temporaires de l'agent sont placés dans `.cursor/worktrees/`. Pour éviter que les analyseurs de code standards (les serveurs de langage LSP de TypeScript, Go, Rust, Python, etc.) ne démarrent sur ces dossiers et ne saturent le processeur ou ne polluent l'éditeur avec des erreurs en double, Cursor désactive l'analyse sémantique sur ces répertoires et applique uniquement une coloration syntaxique TextMate via l'extension `cursor-worktree-textmate`.
  * **Vérification de code à la volée (Delta Linting) :** La fonction centrale `GetLintsForChange` reçoit en entrée un descriptif de modification virtuelle (`GetLintsForChangeRequest.File` avec `{ relative_workspace_path, initial_content, final_content }`). Sans jamais écrire le fichier sur le disque physique de l'utilisateur, le serveur lance les outils de relecture (linters) et renvoie un rapport complet des erreurs (`GetLintsForChangeResponse.Lint`) et des suggestions d'auto-correction (`QuickFix.Edit`).
* **Intégration SOTA pour Sinew :**
  * Implémenter un démon de diagnostic en arrière-plan qui écoute sur un canal nommé (Named Pipe Windows).
  * Isoler les répertoires de travail temporaires dans `.sinew/worktrees/` et s'assurer que les linters/LSP de l'éditeur de Sinew les ignorent pour éviter les blocages.
  * Utiliser la plomberie Git avec un index temporaire (`GIT_INDEX_FILE`) pour capturer l'état modifié des fichiers sans modifier l'index de travail de l'utilisateur.

---

## 2. Système d'Indexation et de Recherche (Retrieval & Merkle)
* **La Métaphore :** Le *registre de signatures et détecteur de similitudes*. Pour vérifier si des livres d'une bibliothèque ont été modifiés, on ne relit pas chaque page. On compare juste le code-barres unique de chaque carton de livres (les nœuds d'un arbre de Merkle).
* **Fonctionnement dans Cursor :**
  * **Comparaison incrémentale par arbre de Merkle :** Le client natif Rust `@anysphere/file-service` génère localement un arbre de hachage de la structure de fichiers (Merkle Tree). Par exemple, avec `SyncMerkleSubtreeV2Request`, le client et le serveur s'échangent les signatures des répertoires pour repérer instantanément et uniquement les fichiers modifiés, évitant des analyses globales coûteuses.
  * **Mise à jour en temps réel via observateur :** Un observateur de fichiers (`FileSystemWatcher`) intercepte chaque ajout, modification ou suppression de fichier et met à jour incrémentiellement l'arbre de Merkle (`merkleClient.onDidChange`, `onDidCreate`, `onDidDelete`) à la volée. L'index reste toujours à jour sans aucun scan lourd à froid.
  * **Regroupement par similarité (Simhash) :** Pour la recherche globale et la détection de doublons, les fichiers sont convertis en signatures numériques condensées de taille fixe (vecteurs simhash calculés via `merkleClient.getSimhash()`). Les documents proches partagent des signatures similaires, ce qui permet de retrouver des exemples de code connexes dans le projet.
  * **Recherche native ultra-rapide (GrepClient) :** L'extension intègre un moteur de recherche natif Rust pilotant l'outil de fouille `crepectl` directement en mémoire, ce qui permet de scanner des millions de lignes de code en quelques millisecondes et de renvoyer les résultats au panneau de recherche VS Code.
  * **Sécurisation des chemins par chiffrement :** Pour protéger la confidentialité du code propriétaire, le système applique un chiffrement local des chemins de fichiers (`pathEncryptionKeySHA256Hash`) avant de les envoyer au serveur d'indexation distant, masquant ainsi la structure interne du projet.
  * **Calculs asynchrones (Worker Threads) :** Les opérations de calcul intensif comme les calculs de différences complexes (diffs de lignes via `computeLinesDiff`) sont déléguées à un thread d'arrière-plan (`worker_threads`) pour ne pas bloquer l'extension host de l'éditeur.
* **Intégration SOTA pour Sinew :**
  * Utiliser un système de signatures (hachage Merkle) pour le catalogue local SQLite afin d'éviter de réindexer les fichiers non modifiés au démarrage.
  * Intégrer un filtre d'exclusion global respectant les fichiers `.sinewignore` ou `.cursorindexingignore`.
  * Offloader les calculs lourds de différences de code dans un worker secondaire.

---

## 3. Automatisation de Navigateur (MCP & Contrôle Web)
* **La Métaphore :** La *télécommande de drone avec caméra*. Plutôt que de lancer une lourde voiture télécommandée autonome contenant son propre groupe électrogène (moteurs Puppeteer ou Playwright complets), l'assistant utilise le drone léger de l'éditeur pour prendre des photos et lui envoyer des consignes de pilotage simples.
* **Fonctionnement dans Cursor :**
  * **Délégation légère de l'affichage (WebView/CDP) :** L'extension `cursor-browser-automation` n'embarque pas de navigateur lourd. Elle communique avec la vue intégrée de l'éditeur (`cursor.browserView.navigate`, `cursor.browserView.sendCDPCommand`) via le protocole standard Chrome DevTools (CDP).
  * **Cibles par étiquettes simplifiées (`browser_snapshot`) :** Avant toute interaction, l'assistant demande une capture de la structure visible de la page. Le navigateur génère un arbre simplifié où chaque élément interactif reçoit une étiquette simple (ex: `@ref1`, `ref=2`). L'assistant clique ou écrit sur ces références, évitant de casser l'action avec des chemins complexes (sélecteurs CSS/XPath fragiles).
  * **Isolation des entrées physiques :** L'extension rejette les commandes CDP d'entrée directe (`Input.*`) car elles peuvent interférer avec le focus global de l'éditeur Electron. Elle utilise à la place des injections de scripts JavaScript ciblées dans le document (`Runtime.evaluate`) pour cliquer et saisir du texte de manière stable.
  * **Gestion fine des échecs d'interaction :** Si le clic échoue car un élément visuel (comme une fenêtre modale ou un bandeau d'acceptation de cookies) bloque le bouton, le navigateur renvoie une erreur descriptive explicite ("le clic va toucher un élément modal, fermez-le d'abord") au lieu de renvoyer une erreur technique muette.
  * **Sauvegarde sur fichier des retours volumineux :** Pour éviter d'inonder le contexte de discussion de l'IA avec des données massives (comme des rapports de performance CDP ou des profils de code dépassant 25 000 caractères), le système écrit ces sorties dans `~/.cursor/browser-logs` et renvoie une référence de fichier local (`log_file`).
  * **Correctif de rafraîchissement OAuth concurrent :** Un correctif est appliqué au SDK MCP pour éviter que plusieurs connexions en parallèle ne demandent le rechargement simultané des jetons d'accès (renvoi de l'erreur `SiblingAlreadyRefreshedError` pour aligner les instances).
* **Intégration SOTA pour Sinew :**
  * Exploiter le pont local de contrôle Chrome existant de Sinew en remplaçant les sélecteurs CSS complexes par un système d'étiquetage temporaire d'éléments interactifs dans le rendu visuel.
  * Rendre les erreurs de navigation interactives (boutons masqués, pages non chargées) intelligentes et explicatives pour que l'IA sache comment s'adapter (ex: faire défiler la page, fermer une popup).
  * Rediriger les gros fichiers de log de console et de profils vers des fichiers temporaires pour économiser la mémoire de contexte du chat.

---

## 4. Boucle de l'Agent et Sécurité (Agent Loop & Approvals)
* **La Métaphore :** Le *robot de chantier avec garde-fou*. Le robot effectue ses tâches pas à pas sous l'œil attentif d'un superviseur qui valide ses demandes d'outils et l'arrête s'il commence à tourner en rond.
* **Fonctionnement dans Cursor :**
  * **Démon d'arrière-plan détaché (Agent Worker Daemon) :** L'exécution locale de l'agent est déléguée à un démon externe (`cursor-agent-worker` ou `agent-cli`). Ce démon est lancé en tâche de fond indépendamment de l'éditeur (`spawn` avec options `{ detached: true, stdio: 'ignore' }`), évitant ainsi d'interrompre l'agent si l'éditeur est rechargé ou planté. Le démon communique avec l'éditeur via http2 sur un socket local (`.sock`) et son cycle de vie est tracé via des fichiers PID.
  * **Délégation à des sous-agents spécialisés (Sub-Agents) :** Plutôt que d'utiliser un modèle unique pour tout faire, la boucle délègue les tâches à des sous-agents spécialisés (`SUBAGENT_TYPE_DEEP_SEARCH` pour la recherche avancée, `SUBAGENT_TYPE_FIX_LINTS` pour corriger les erreurs de compilation, `SUBAGENT_TYPE_SPEC` pour concevoir les plans d'implémentation, et `SUBAGENT_TYPE_TASK` pour les écritures).
  * **Application ultra-rapide des modifications (Fast Apply) :** Pour appliquer des blocs de code volumineux sans lenteur, Cursor utilise un modèle de diff ultra-rapide (`FAST_APPLY_MODEL_TYPE_GPT4O_DIFF` ou `SONNET_35_DIFF`) qui applique de petits correctifs structurés (`apply_agent_diff`) au lieu de réécrire le fichier en entier.
  * **Boucle de contrôle orchestrée (`runTurnLoop`) :** La boucle exécute pas à pas les instructions du modèle jusqu'à une limite fixée (`maxSteps`). À chaque étape, elle rassemble les outils disponibles, injecte les règles personnalisées de l'utilisateur (fichiers `.cursorrules`) et appelle le modèle.
  * **Détection de changement de mode (Nudges) :** Si l'utilisateur ou l'assistant change de mode de travail en cours de route, la boucle le détecte et injecte un message d'orientation invisible dans le fil de discussion pour réaligner le comportement du modèle.
  * **Rappels de relecture et garde-fous (Forced Reflection & Reminders) :** 
    * Si l'assistant enchaîne trop d'étapes sans progresser (ex: plus de 10 étapes d'exécution), la boucle injecte un avertissement système obligatoire : `<system_reminder>You MUST now use the Reflect tool...</system_reminder>`.
    * En cas d'échecs répétés sur un outil, des instructions correctives sont insérées dynamiquement dans le retour d'information de l'outil (`experimental_content`) sans polluer l'historique visuel du chat.
  * **Interactions asynchrones (Ask Question Queue) :** Si l'assistant a besoin d'une décision de l'utilisateur (ex: outil `ask_question`), la boucle se met en pause et écoute le récepteur d'actions (`conversationActionReceiver`). Dès que l'utilisateur répond, la réponse est injectée directement comme un résultat d'outil et la boucle reprend instantanément.
* **Intégration SOTA pour Sinew :**
  * Mettre en place un démon d'arrière-plan découplé pour exécuter les tâches système sans risquer de planter la fenêtre de l'utilisateur.
  * Utiliser un Swarm de sous-agents spécialisés dans Sinew (recherche, linter, écriture) pour paralléliser et optimiser la résolution de problèmes complexes.
  * Adopter une boucle d'auto-correction forcée (Reflect) en cas de blocages ou de boucles de tentatives infinies sur un même outil.

---

## 5. Génération de Commits et Métriques (Diff & Commits)
* **La Métaphore :** Le *secrétaire de rédaction de journal de bord*. Il consulte les notes de la journée, compare le travail fini avec le point de départ, et rédige un résumé clair et lisible en français simple pour l'équipe.
* **Fonctionnement dans Cursor :**
  * **Extraction intelligente de contexte (`GetCurrentIndexAndRecentCommits`) :** L'extension interroge l'API Git de VS Code (`vscode.git`) pour récupérer la liste des fichiers modifiés et calcule leur différence (diff) par rapport au commit de référence `HEAD`. Si le dépôt est vide, elle compare avec la signature vide universelle (`4b825dc642cb6eb9a060e54bf8d69288fbee4904`).
  * **Apprentissage du style de rédaction historique :** La requête de génération `WriteGitCommitMessageRequest` envoie au serveur d'IA les diffs ainsi qu'un tableau contenant les 10 derniers messages de validation (`previous_commit_messages`). Cela permet au modèle de s'approprier automatiquement le style de l'utilisateur (ex : conventionnel, français, descriptif ou court) pour rédiger un message homogène.
  * **Suivi de télémétrie des requêtes :** Pendant l'exécution de l'agent, un module de suivi enregistre la taille des fichiers, le nombre de lignes ajoutées/supprimées et les détails des modifications sous forme de fichiers `metadata.json` afin de mesurer la qualité des actions produites.
* **Intégration SOTA pour Sinew :**
  * Proposer une fonction de rédaction automatique de commit Git dans le panneau de contrôle de Sinew.
  * Récupérer les 10 derniers commits locaux pour adapter automatiquement le ton de rédaction de l'IA à celui de l'utilisateur (ex: commits en français court, impératif, ou descriptif).


