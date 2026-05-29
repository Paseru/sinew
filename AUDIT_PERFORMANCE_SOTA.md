# ⚡ Rapport d'Audit de Performance et de Qualité SOTA — Sinew

Ce rapport présente l'analyse rigoureuse et exhaustive de la performance, de la qualité du code et des optimisations de type SOTA (State-of-the-Art) au sein du projet **Sinew** (Rust, React/TypeScript, Tauri, et Chrome Bridge).

---

## 📊 Synthèse des Points Forts (SOTA Optimizations existantes)

Le projet intègre déjà plusieurs mécanismes avancés de gestion des performances qui se situent au meilleur niveau de l'état de l'art actuel.

### 1. Traitement Fluide des Flux LLM via `requestAnimationFrame` (60 FPS)
* **Emplacement :** `src/components/chat/ChatPane.tsx`
* **Mécanisme :** Les morceaux de texte (`text_chunk`) et de réflexion (`thinking_chunk`) arrivant en streaming ne déclenchent pas immédiatement un rendu React individuel. Les signaux sont fusionnés dans une file d'attente temporaire et vidés (flushed) en synchronisation avec le cycle de rafraîchissement natif de l'écran grâce à `window.requestAnimationFrame`.
* **Impact :** Élimine complètement les gels d'interface (UI stuttering) même lors de réponses LLM à très haut débit, maintenant un défilement et une réactivité impeccables.

### 2. Découpage Intelligent du Bundle Frontend (Vite & Rollup Manual Chunks)
* **Emplacement :** `vite.config.ts`
* **Mécanisme :** Extraction des composants extrêmement lourds comme l'éditeur Monaco (`monaco`), le terminal (`xterm`), le moteur de rendu `mermaid` et les outils de Markdown dans des fichiers Javascript séparés (chunks).
* **Impact :** Le bundle principal de démarrage reste léger et rapide à charger, améliorant considérablement le temps de premier affichage de l'application.

### 3. Compaction Sémantique de l'Historique (Handoff Summary)
* **Emplacement :** `crates/sinew-app/src/compact.rs`
* **Mécanisme :** Au lieu de tronquer brutalement les messages passés, Sinew demande au modèle de générer un résumé de passation fonctionnel (handoff summary). Le système élimine tous les jetons transitoires (appels d'outils, logs système, réponses intermédiaires) et conserve uniquement le résumé structuré et les derniers messages de l'utilisateur (limite de 80 000 caractères).
* **Impact :** Réduction drastique du coût en jetons, baisse de la latence de traitement des invites et maintien d'une mémoire sémantique à long terme pour les grands projets.

---

## 🔍 Opportunités d'Optimisation Majeures (Pistes d'Amélioration)

Bien que l'application soit de grande qualité, plusieurs goulots d'étranglement de performance et de qualité SOTA ont été identifiés.

### 1. Absence de Profil d'Optimisation Compilateur pour la Release Rust
* **Observation :** Aucun fichier `Cargo.toml` (à la racine ou dans `src-tauri/`) ne configure le profil `[profile.release]`.
* **Problème :** Par défaut, Rust compile en mode release avec des paramètres de base. Un exécutable de bureau Tauri sans LTO (Link-Time Optimization) et sans découpage de symboles de débogage est inutilement lourd et moins véloce.
* **Recommandation SOTA :** Ajouter la configuration suivante dans le `Cargo.toml` racine :
  ```toml
  [profile.release]
  opt-level = 3        # Optimisation maximale du code
  lto = true           # Link-Time Optimization globale entre crates
  codegen-units = 1    # Génération de code unifiée pour de meilleures optimisations
  panic = "abort"      # Supprime la surcharge du déroulement de pile
  strip = true         # Retire les symboles de débogage (gains de 10 à 30 Mo sur le binaire)
  ```

### 2. Surcharge I/O de SQLite et Absence du mode WAL
* **Observation :** Dans `crates/sinew-index/src/store.rs` et `crates/sinew-app/src/store.rs`, une nouvelle connexion physique SQLite (`Connection::open`) est ouverte et fermée à chaque micro-action (vérification d'empreinte, liste des fichiers, indexation, recherche). De plus, aucun pragma de performance n'est configuré.
* **Problème :** Ouvrir des fichiers répétitivement sur le disque génère une surcharge système colossale (appels système coûteux), particulièrement sous Windows avec le verrouillage de fichiers. De plus, le mode de journalisation par défaut bloque les lectures lors des écritures.
* **Recommandation SOTA :**
  1. **Réutilisation des Connexions :** Conserver une connexion unique ouverte pendant la durée de vie du service au lieu de la réouvrir.
  2. **Pragmas d'optimisation :** Lors de l'ouverture de la connexion, exécuter les pragmas suivants :
     ```sql
     PRAGMA journal_mode = WAL;      -- Permet des lectures/écritures simultanées ultra-rapides
     PRAGMA synchronous = NORMAL;    -- Excellent compromis de sécurité et rapidité avec le WAL
     PRAGMA cache_size = -2000;      -- Alloue ~2 Mo de cache mémoire pour les pages SQLite
     PRAGMA temp_store = MEMORY;     -- Stocke les tables temporaires en RAM
     ```

### 3. Blocage de l'Exécuteur Asynchrone (Threadpool Starvation)
* **Observation :** Les outils d'accès disque de Sinew (`ReadTool` dans `read.rs`, `WriteFileTool` dans `write.rs`, `EditFileTool` dans `edit.rs`) effectuent des lectures/écritures de fichiers synchrones directes (`fs::read`, `fs::write`, `fs::metadata`) au sein de fonctions asynchrones exécutées directement par le pool d'exécuteurs Tokio.
* **Problème :** Bloquer un thread travailleur Tokio avec des entrées/sorties disque synchrones empêche l'exécuteur de traiter les autres tâches asynchrones en cours. Sous forte charge ou avec de gros fichiers, cela peut provoquer des freezes imperceptibles de l'interface ou des hausses de latence.
* **Recommandation SOTA :** Encapsuler toutes les opérations synchrones lourdes dans un conteneur dédié de blocage fourni par Tokio :
  ```rust
  let content = tokio::task::spawn_blocking(move || {
      std::fs::read_to_string(path)
  }).await??;
  ```
  Cela permet de décharger l'I/O bloquante sur un pool de threads secondaires prévu à cet effet, préservant la fluidité totale de la boucle d'événements asynchrones principale.

---

## 🏆 Conclusions de l'Audit de Performance

| Secteur Audité | Statut SOTA | Impact sur l'Expérience | Priorité d'Amélioration |
| :--- | :--- | :--- | :--- |
| **Gestion du Stream UI** | ⭐ Excellent (SOTA) | Défilement fluide, aucune saccade à l'écran | Minimale (Déjà optimisé) |
| **Bundling & Chargement** | ⭐ Très Bon (SOTA) | Temps de chargement initial réduit | Minimale (Déjà optimisé) |
| **Compaction de Contexte** | ⭐ Excellent (SOTA) | Économie de tokens, conservation sémantique | Minimale (Déjà optimisé) |
| **Compilation Rust (Release)**| ⚠️ Améliorable | Taille de l'exécutable + Vitesse d'exécution | **Haute** (Facile et gros impact) |
| **Accès SQLite (Indexation)** | ⚠️ Améliorable | Lenteur potentielle sur gros répertoires | **Haute** (Latence d'indexation) |
| **I/O Asynchrone (Tokio)** | ⚠️ Améliorable | Risque de blocage du thread pool sous charge | Moyenne (Sécurité PTY/Fichiers) |

Les optimisations identifiées au niveau de la compilation Rust et de la configuration de SQLite apporteraient un gain immédiat en réactivité et réduiraient significativement la taille de l'exécutable final de Sinew.
