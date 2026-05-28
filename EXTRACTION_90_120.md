# Extraction Technique et Fonctionnelle : Commits 90 à 120 (Tranche @Subagent_90_120)

Ce rapport recense l'ensemble des micro-améliorations, polissages visuels, innovations ergonomiques, optimisations réseau et de sécurité, ainsi que les avancées de furtivité introduites dans le fork de Sinew entre les **commits 90 et 120**.

---

## 1. Furtivité SOTA & Anti-Détection (Stealth & Web Automation)

* **Pont Chrome CDP-first & Stealth :**
  * Remplacement des clics synthétiques JavaScript classiques (vulnérables aux détections anti-bot) par une injection native d'événements de bas niveau (`mousePressed` et `mouseReleased`) directement via le protocole Chrome DevTools Protocol (`Input.dispatchMouseEvent`).
  * Déplacement de souris fluide simulant un comportement humain basé sur des trajectoires physiques réalistes le long de courbes de Béziers multi-candidates évaluées par coût physique (amortissement, vitesse, inertie) avant d'engager le clic, garantissant un focus parfait.
* **Sélection d'onglet intelligente (Tab Reuse) :**
  * Optimisation du sélecteur d'onglets au sein de `mcp_server.js` pour cibler en priorité l'onglet Chrome déjà actif ou le dernier onglet contrôlé. Cela prévient la création inutile d'onglets doublons lors d'appels MCP consécutifs et réduit considérablement l'empreinte mémoire système.
* **Résolution d'URL résiliente (background.js) :**
  * Amélioration du parseur d'URL pour extraire proprement les noms de domaines bruts sans protocole (ex. `google.com` ou `julienpiron.fr`) et y injecter automatiquement le préfixe sécurisé HTTPS, facilitant la navigation autonome pour l'IA.

---

## 2. Robustesse Système, Runtimes Portables & Self-Healing Réseau

* **Binarisation du Native Host en Rust (Zero Popup Windows) :**
  * Création et compilation de la crate Rust `native-host-wrapper` (dans `sinew-chrome-bridge/native-host-wrapper`) configurée comme hôte de messagerie native Chrome (`com.sinew.chrome_bridge.json`).
  * Ce binaire léger en Rust natif utilise `tokio::io::copy` pour relier de façon asynchrone et transparente le protocole Chrome et le serveur Node.js local.
  * **Bénéfice majeur :** Éradication totale des fenêtres de console d'invite de commandes Windows (`cmd.exe` noires) qui s'ouvraient ou clignotaient auparavant en arrière-plan sous Windows lors du lancement du navigateur.
* **Auto-Guérison des conflits réseau (`EADDRINUSE`) :**
  * Implémentation dans `server.js` d'un serveur dual-mode auto-guérisseur. Si le port réseau par défaut (`29002`) est occupé par un processus orphelin ou une autre instance de l'application, le serveur bascule automatiquement sur un port dynamique alternatif et lance un Tunnel Forwarder transparent. La communication inter-processus reste ininterrompue.
* **Résolution dynamique des chemins d'exécution (Zero-configuration PATH) :**
  * Remplacement de tous les chemins d'exécutables Python codés en dur dans `server.js` par un algorithme de détection dynamique qui scanne les variables d'environnement système (`PATH`) pour repérer le runtime installé (Microsoft Store, conda, scripts locaux).
  * Retour par défaut au serveur MCP Node.js natif sans dépendance Python ni clé d'API requise, maximisant la portabilité de l'écosystème chez l'utilisateur final.
* **Stabilité et diagnostics du registre Windows :**
  * Amélioration du script PowerShell d'enregistrement `register.ps1` pour cibler de manière robuste la ruche utilisateur `HKCU\Software\Google\Chrome\NativeMessagingHosts`, simplifiant l'installation multi-PC sans droits administrateur.

---

## 3. Synchronisation Décentralisée SQLite & OneDrive

* **Architecture Tombstone anti-résurrection SQLite :**
  * Création d'une table SQL dédiée `deleted_conversations` stockée au sein de la base de données SQLite locale (`app_state.db`).
  * Enregistrement systématique des identifiants uniques des conversations supprimées accompagnés d'un horodatage précis dans cette table.
  * **Résolution définitive du conflit cloud :** Lors des merges différentiels de synchronisation OneDrive/Tauri multi-PC, les conversations marquées comme supprimées localement sont purgées définitivement du cloud OneDrive au lieu de ressusciter arbitrairement lors du démarrage de l'application sur un autre poste de travail.
* **Persistance SQLite des paramètres MCP :**
  * Enregistrement persistant des configurations d'exécutables et de ports MCP directement dans SQLite via `add_to_sinew.py`, empêchant les collisions au démarrage des agents.

---

## 4. UI/UX Premium, Ergonomie & Visuels SOTA

* **Requêtes de conteneur CSS modernes (Container Queries) :**
  * Introduction de `@container` et de `container-type: inline-size` sur `.settings-pane` (dans `src/styles.css`).
  * Permet un redimensionnement ultra-fluide et réactif à chaud de l'interface des options et formulaires de formulaires d'API en fonction de la largeur réelle du panneau, éliminant les Media Queries rigides par rapport à l'écran.
* **Modes de compaction visuelle triphasés :**
  * Intégration dans `ChatPane.tsx` d'un commutateur de verbosité à trois niveaux :
    * *Détaillé* : Affichage complet.
    * *Compact* : Replie les blocs de réflexion.
    * *Très compact* : Masquage total et instantané de toutes les cartes d'outils terminées avec succès (`ToolCard.tsx`) et des blocs de réflexion de l'IA. Seules les réponses textuelles pures de l'assistant restent affichées. Les erreurs d'outils éventuelles restent visibles pour le diagnostic.
* **Pilule ergonomique "Influencer" :**
  * Remplacement du bouton d'envoi standard par une pilule stylisée "Influencer" avec une flèche vers le haut.
  * Amélioration de la visibilité des bordures lumineuses et états visuels de transition de la file d'attente d'invites utilisateur (Queue).
* **Diagnostics interactifs intégrés :**
  * Intégration de logs diagnostiques interactifs déroulants dans la fenêtre popup de l'extension Chrome (`popup.html` / `popup.js`) permettant à l'utilisateur de vérifier l'état du pont local en un clic.
* **Résolution du rafraîchissement des questions épinglées :**
  * Correction d'un bug de persistance où la question épinglée (Sticky Question) ne se mettait pas à jour lors du nettoyage ou du changement de conversation.
