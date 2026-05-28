# Extraction Exhaustive et Critique : Commits 120 à 230 (Tranche @Subagent_120_230)

Cette extraction recense l'ensemble des micro-améliorations, polissages visuels, innovations ergonomiques, optimisations réseau et de sécurité, ainsi que les avancées de furtivité introduites dans le fork de Sinew entre les **commits 120 et 230**.

---

## 1. Excellence Ergonomique & Intégration Native (Premium UX)

* **Actions contextuelles natives en un clic (Clic droit "Exécuter / Run")** :
  * **Sur l'arborescence des fichiers** : Ajout d'une option "Exécuter / Run" dans le menu contextuel du composant `FileTree.tsx`. Les requêtes sont relayées via des appels IPC Tauri à l'API système sous-jacente (`cmd.exe /C` ou le gestionnaire natif de l'OS) pour exécuter directement des scripts (`.bat`, `.py`, `.sh`) ou ouvrir des documents.
  * **Sur les liens de fichiers dans le chat** : Interception des clics droits sur les liens de fichiers rendus par le parser Markdown dans les bulles de discussion (`ChatPane.tsx`, `Markdown.tsx`). Permet à l'utilisateur de lancer directement un script modifié ou créé par l'assistant sans avoir à le chercher manuellement dans l'arbre de gauche.
* **Sélection et copie fluides dans le chat** : Modification globale des règles d'interaction CSS (`src/styles.css`) sur les conteneurs de discussion de l'assistant. L'injection de `-webkit-user-select: text` et `user-select: text` contourne le blocage par défaut de Tauri WebView, permettant la copie d'extraits textuels sans avoir à ouvrir l'éditeur de code.
* **Header de question collant intelligent (Sticky Question)** : Résolution d'un bug de rafraîchissement visuel. Intégration d'un `ResizeObserver` et d'écouteurs d'événements de scroll dans `ChatPane.tsx` pour forcer la mise à jour réactive et sans à-coups de la question épinglée en haut de l'écran lors du défilement.
* **Compteur d'options actives** : Ajout d'un compteur visuel réactif dans l'onglet des options de la barre de configuration (`SettingsPane.tsx`) pour afficher instantanément le nombre de configurations activées.
* **Splash Screen et évitement de flash blanc** :
  * Configuration de la WebView Tauri avec une couleur d'arrière-plan par défaut noire stricte et état initial caché (`visible: false`).
  * Révélation de la fenêtre via `show()` uniquement après l'initialisation du premier écran et le chargement complet des polices de caractères, évitant le flash blanc éblouissant de démarrage.
  * Ajout d'une animation d'accueil premium (écartement et fondu progressif des barres du logo Sinew).
* **Interface de saisie et de file d'attente premium** :
  * Remplacement du bouton d'envoi classique par une pilule stylisée "Influencer" avec flèche vers le haut.
  * Amélioration de la visibilité des bordures lumineuses actives et états bloqués sur la file d'attente des invites.
* **Grille de configuration réactive et tolérante** :
  * Utilisation d'une grille fluide `grid-template-columns: repeat(auto-fit, minmax(250px, 1fr))` limitée à un maximum strict de 3 colonnes pour l'affichage multi-comptes OpenAI secondaires dans `SettingsPane.tsx`. Évite l'étirement disgracieux ou le rognage des champs de clés API.
  * Remplacement des couleurs vives de diagnostic par des teintes de gris neutres et épurées.

---

## 2. Compaction Visuelle & Réduction du Bruit (Visual Compaction)

* **Mode de compaction du raisonnement triphasé** : Extension de l'ancien commutateur booléen de compaction vers une gestion d'état triphasée :
  * *Détaillé* : Affiche l'historique complet de la réflexion technique.
  * *Compact* : Replie le bloc de réflexion en conservant une entête réductible ("Thinking for 5s").
  * *Très compact* : Masque intégralement et instantanément le bloc de réflexion et les cartes d'outils (`ToolCard.tsx`) exécutés avec succès dès que l'IA a terminé son raisonnement. Les erreurs d'exécution d'outils restent visibles pour le diagnostic.
* **Persistance SQLite des modes d'affichage** : Câblage des drapeaux d'état de compaction au sein de la structure des tours de conversation (`TurnRecord`) stockée dans la base SQLite locale. Les conversations rechargées s'affichent proprement dans le mode souhaité, s'affranchissant de l'état en mémoire volatile de React.
* **Container Queries pour les formulaires** : Remplacement complet des `@media` queries par des `@container` queries (`container-type: inline-size`) sur `.settings-pane`. Les grilles d'options s'adaptent de façon fluide à la largeur exacte de leur panneau parent lorsque l'utilisateur le redimensionne en direct via le Splitter.

---

## 3. Furtivité SOTA & Anti-Détection (Stealth & Spoofing)

* **Pont Chrome CDP-first & Stealth** :
  * Remplacement des clics synthétiques DOM JavaScript par l'injection matérielle d'événements de clic (`mousePressed`, `mouseReleased`) via le protocole direct Chrome DevTools Protocol (CDP).
  * Génération de trajectoires physiques réalistes basées sur des courbes de Béziers de degré 3 candidates évaluées en temps réel par un score de coût physique (inertie, accélération, amortissement).
  * Saisie clavier asynchrone organique simulant la frappe humaine en introduisant des micro-délais variables (80ms à 180ms) entre chaque pression de touche.
  * Dessin d'une traînée lumineuse néon clignotante avec dégradés de couleurs visualisant le déplacement du curseur virtuel de l'agent pour rassurer et guider l'utilisateur.
  * Éradication complète de la barre jaune d'avertissement de débogage ("Cette extension est en train de déboguer ce navigateur") grâce à l'abandon de `chrome.debugger` au profit d'injections directes par scripts de contenu isolés.
* **Spoofing d'identité de haut niveau** :
  * Spoofing de l'en-tête `User-Agent` sous l'identité officielle `codex-cli` pour toutes les requêtes ChatGPT Codex, minimisant le risque de blocage ou de détection par OpenAI.
  * Génération dynamique de signatures d'en-tête `User-Agent` adaptées à la plateforme hôte réelle (Windows, macOS, Linux) pour les requêtes Gemini et Antigravity afin d'éviter les anomalies de fingerprinting.
  * Injection de signatures matérielles et de télémétries Cursor IDE authentiques pour sécuriser les tests de l'agent Composer.
* **Accès OAuth transparent sans frottements** :
  * Intégration interactive du flux d'authentification OAuth pour Cursor en un clic dans `SettingsPane.tsx`.
  * Résolution des inconsistances SQLite de VS Code dans `composer.rs` : lecture directe des jetons JWT stockés sous forme de `TEXT` dans la table typée en `BLOB` de `state.vscdb` via un extracteur résilient inspectant l'affinité de type en temps réel (`rusqlite::ValueRef`).

---

## 4. Performance, Resilience Réseau & Architecture Locale

* **Steering (Pilotage à chaud asynchrone)** :
  * Implémentation d'un canal asynchrone thread-safe `steering_rx` dans `TurnContext` (Rust, `turn.rs`).
  * Intégration de la routine `drain_steering_commands` à 5 étapes clés de la boucle d'exécution de l'agent. Si l'utilisateur saisit une correction ou une réorientation pendant que l'agent génère des fichiers, l'instruction est injectée à la volée dans l'historique et force le rebouclage immédiat de l'agent sans interrompre sa tâche en cours ni vider son contexte de jetons.
* **Bufferisation du Stream d'interface (UI Batching)** :
  * Intégration d'un accumulateur de jetons réactif dans `ChatPane.tsx`. L'écriture asynchrone de l'IA est stockée temporairement et injectée dans le state React sous `requestAnimationFrame` toutes les 16ms.
  * Élimine les gels d'interface utilisateur en synchronisant les rafraîchissements React avec le taux de rafraîchissement d'écran physique (60Hz) lors de streams IA à haut débit (> 100 t/s).
* **Multiplexage HTTP/2 et accélération Markdown** :
  * Activation explicite de la fonctionnalité `http2` sur `reqwest` dans `Cargo.toml` pour multiplexer les streams des modèles de raisonnement.
  * Mémoïsation sélective (`React.memo`) des blocs de code Monaco dans `Markdown.tsx` pour éliminer le lag CPU au défilement.
* **Résilience Réseau Auto-Guérisseuse** :
  * Serveur Node.js local capable d'intercepter l'erreur `EADDRINUSE` en créant un canal intermédiaire dual-mode et en redirigeant le trafic en tunnel-forwarding automatique. Fin des crashs de démarrage si le port est temporairement verrouillé par l'OS.
  * Intégration du proxy asynchrone binarisé compilé en Rust `native-host-wrapper.exe` pour l'extension Chrome (communications via `tokio::io::copy`), empêchant l'ouverture ou le clignotement de fenêtres d'invite de commandes Windows noires.
  * Annulation asynchrone sûre (`select!` sur les futures réseaux reliées au token global d'annulation `cancel`) pour le rafraîchissement des catalogues MCP et du stream d'agent, empêchant le gel de l'application en cas de timeout réseau.
* **Indexation locale sémantique et recherche vectorielle** :
  * Création de la crate `sinew-index` pour l'indexation de codebase légère en arrière-plan.
  * Implémentation du découpage intelligent conscient des symboles syntaxiques (Symbol-aware chunking) respectant les limites de classes et de fonctions de code au lieu de simples tranches physiques de caractères.
  * Génération asynchrone des embeddings et synchronisation OneDrive transparente avec SQLite.
  * Injection automatique d'extraits sémantiques ciblés de l'index dans le prompt système de l'agent Cursor Composer 2.5 (`context_injection.rs`).
* **Table Tombstone anti-résurrection SQLite** :
  * Intégration de la table `deleted_conversations` et nettoyage différentiel Cloud/OneDrive lors des merges SQLite distants pour éviter la résurrection intempestive de fils de discussion supprimés.
* **Processus légers et portables** :
  * Résolution dynamique de l'exécutable Python via les variables d'environnement système `PATH` pour s'affranchir de chemins codés en dur.
  * Fallback natif Node.js sans dépendances tierces ni clés d'API requises pour le serveur MCP par défaut.
