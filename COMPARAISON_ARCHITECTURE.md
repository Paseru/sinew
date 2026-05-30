# Comparaison de l'Architecture Cursor Décompilée vs Sinew Actuel

Ce document compare les 5 concepts clés identifiés dans le code de Cursor (décompilé) avec ce qui est déjà présent dans **Sinew**, et évalue ce qui est le plus pertinent à adapter ou améliorer.

---

## 1. L'Espace Fantôme (Shadow Workspace)
*   **Ce que fait Cursor :**
    *   Utilise un démon gRPC local de validation (`ShadowWorkspaceService`).
    *   Crée un snapshot instantané via la plomberie Git (`GIT_INDEX_FILE` temporaire et `git write-tree`) sans polluer le dossier de travail physique.
    *   Exclusion LSP (`cursor-worktree-textmate`) pour éviter que les analyseurs du projet n'analysent le dossier fantôme temporaire.
    *   Relecture à la volée en mémoire (`GetLintsForChange`) pour valider les erreurs de compilation/syntaxe avant écriture.
*   **Ce qu'a déjà Sinew :**
    *   Des commandes de création/suppression de Git Worktrees physiques (`git_create_worktree_command` dans `src-tauri/src/git.rs`).
    *   Pas de démon gRPC local, pas de delta-linting virtuel (les fichiers doivent être modifiés sur le disque pour être vérifiés par `ReadLintsTool`), et pas d'exclusions LSP automatiques sur ces répertoires temporaires.
*   **Le verdict pour Sinew (Ce qui est le mieux) :**
    *   **Conserver l'approche actuelle** pour la gestion de projet classique (les worktrees physiques permettent à l'utilisateur de voir ce que fait l'agent s'il le souhaite).
    *   **Amélioration recommandée :** Ajouter le filtrage d'exclusion dans le gestionnaire d'arbre de fichiers et l'éditeur de Sinew pour ignorer automatiquement les répertoires temporaires dans `.sinew/worktrees`, évitant ainsi d'encombrer l'interface utilisateur ou de lancer des serveurs de langage LSP en arrière-plan (qui consomment beaucoup de processeur).

---

## 2. L'Indexation et le Filtrage (Retrieval & Ignore)
*   **Ce que fait Cursor :**
    *   Synchronisation incrémentale par arbre de Merkle (via `@anysphere/file-service` en Rust).
    *   Regroupement et détection de similarités par Simhash.
    *   Respecte strictement les fichiers d'exclusion `.cursorignore` et `.cursorindexingignore`.
    *   Chiffrement local des chemins de fichiers.
*   **Ce qu'a déjà Sinew :**
    *   Un indexeur incrémental haute performance en Rust (`crates/sinew-index`) basé sur SQLite FTS5 (recherche plein texte) et Rayon pour le multi-threading.
    *   Ignore les répertoires de développement classiques codés en dur (`node_modules`, `.git`, `.sinew`, `target`, `dist`, etc.).
    *   **Ne lit pas** les fichiers de configuration d'ignoration locaux comme `.gitignore`, `.cursorignore` ou `.sinewignore`.
*   **Le verdict pour Sinew (Ce qui est le mieux) :**
    *   **Le Merkle Tree / Simhash complet n'est pas nécessaire** car l'application Sinew fonctionne de manière 100% locale sans serveur distant pour la recherche de code. L'mtime et la taille de fichier suffisent amplement à maintenir la base SQLite à jour de façon instantanée.
    *   **Amélioration indispensable (Mieux pour nous) :** Lire et respecter dynamiquement les fichiers `.gitignore`, `.cursorignore` et `.sinewignore`. Actuellement, l'index de Sinew se retrouve pollué par des fichiers volumineux (build artifacts, données générées) qui devraient être ignorés, ce qui ralentit la recherche sémantique.

---

## 3. L'Automatisation de Navigateur (MCP & Étiquetage Visuel)
*   **Ce que fait Cursor :**
    *   Délégation de l'affichage via une WebView intégrée et commandes CDP (Chrome DevTools Protocol).
    *   Génération d'un snapshot d'interface avec des étiquettes temporaires simples (`@ref1`, `@ref2`) injectées sur les éléments cliquables. L'IA clique sur ces étiquettes au lieu d'utiliser des sélecteurs CSS complexes.
    *   Gestion explicite des bloqueurs de clic (ex: modales, popups de cookies) avec remontée de messages d'erreur clairs pour que l'IA s'adapte.
    *   Redirection des gros historiques de logs vers des fichiers locaux pour éviter d'inonder le contexte de discussion de l'IA.
*   **Ce qu'a déjà Sinew :**
    *   Un pont de navigation Chrome complet (`sinew-chrome-bridge` réécrit en Rust) et des outils MCP (`navigate`, `screenshot`, `click_selector`, `type_selector`).
    *   Utilise des sélecteurs CSS bruts ou des clics par recherche textuelle (qui se cassent facilement sur les sites Web dynamiques).
*   **Le verdict pour Sinew (Ce qui est le mieux) :**
    *   **L'étiquetage visuel par références (`@ref1`) est nettement supérieur** et beaucoup plus stable que les sélecteurs CSS classiques.
    *   **Amélioration recommandée :** Intégrer l'injection de scripts JavaScript d'étiquetage lors de la capture d'écran de `sinew-chrome-bridge`, pour permettre à notre assistant de piloter le navigateur avec des références fiables et simples. De plus, rediriger les longues erreurs de console vers des fichiers de logs temporaires pour préserver les limites de tokens de l'IA.

---

## 4. La Boucle de l'Agent et les Garde-fous (Agent Loop & Safety)
*   **Ce que fait Cursor :**
    *   Démon d'arrière-plan découplé (`cursor-agent-worker`) qui survit au rechargement de l'éditeur principal.
    *   Boucle orchestrée (`runTurnLoop`) avec injection forcée de réflexion (`Reflect`) et de rappels système en cas de blocage ou d'échecs répétés d'un outil.
*   **Ce qu'a déjà Sinew :**
    *   La boucle s'exécute directement dans le processus de l'application (Tauri).
    *   Il intègre un système d'agents en équipe (`crates/sinew-app/src/team.rs`), mais pas de mécanisme anti-boucle forcée en cas de répétitions de commande par une IA mono-agent.
*   **Le verdict pour Sinew (Ce qui est le mieux) :**
    *   **Conserver l'exécution dans Tauri** (mieux pour la simplicité de distribution et d'installation sans démon externe).
    *   **Amélioration recommandée :** Implémenter un système d'injection de rappels d'auto-correction (Forced Reflection) directement dans notre boucle de turn (`run_turn` dans `crates/sinew-app/src/agent/turn.rs`). Si un outil échoue plusieurs fois de suite ou si la boucle dépasse 8 étapes, insérer automatiquement une instruction système forçant le modèle à analyser ses erreurs avant de soumettre une nouvelle action.

---

## 5. La Suggestion de Commit Git (Génération Sémantique)
*   **Ce que fait Cursor :**
    *   Génère des messages de commit basés sur le diff.
    *   Lit les 10 derniers commits de l'utilisateur pour adapter le ton et la langue de rédaction.
*   **Ce qu'a déjà Sinew :**
    *   Sinew propose des outils Git de base dans son panneau de contrôle, mais l'IA génère les commits de façon brute sur demande de l'utilisateur, sans cohérence de style automatique avec le passé.
*   **Le verdict pour Sinew (Ce qui est le mieux) :**
    *   **C'est une excellente fonctionnalité d'ergonomie.**
    *   **Amélioration recommandée :** Intégrer la lecture des 10 derniers messages de validation locaux lors de l'appel à la génération de commit dans le panneau Git de Sinew, afin d'harmoniser le ton suggéré.
