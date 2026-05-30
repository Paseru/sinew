# Rapport de Conception : Découplage du Moteur d'Agent en Démon Persistant Windows (SOTA)

Ce document présente l'architecture de découplage pour transformer le moteur de discussion de **Sinew** en un démon d'arrière-plan persistant sous Windows (semblable au démon `cursor-agent-worker` officiel).

---

## 1. La Problématique Actuelle (Le maillon faible)

Actuellement, l'exécution des boucles de réflexion et de l'exécution des outils de l'agent tourne directement au sein de l'application principale **Tauri**.
* **Symptôme :** Si l'utilisateur ferme accidentellement la fenêtre de l'interface, recharge le projet, ou si le processus graphique Tauri subit un crash, la tâche de fond de l'agent est immédiatement stoppée en plein milieu de son action (comme une modification de fichier à moitié écrite).
* **Risque :** Corruption de l'état des fichiers, perte de progression des agents autonomes lancés sur des tâches longues de plusieurs minutes.

---

## 2. L'Architecture Cible (Le Démon de Chantier)

Pour assurer la continuité des tâches, nous découplons le moteur dans un binaire indépendant : **`sinew-agent-daemon.exe`**.

```
[ Interface Graphique Tauri ]            [ Démon d'arrière-plan ]
        (Fenêtre UI)                     (sinew-agent-daemon.exe)
             |                                      |
             | ----- Lance / Réveille ------------> | (Spawn detached au boot)
             | ----- Commande : run_turn ---------> | (Named Pipe / TCP Local)
             |                                      | === Exécute LLM / Outils ===
             | <---- Flux d'événements (SSE) ------ | (Mise à jour en temps réel)
             |                                      | === Écrit dans SQLite ===
   [ Fermeture UI ]                                 | (Continue de tourner)
             x                                      |
   [ Réouverture UI ]                               |
             | ----- Reconnexion -----------------> | (Récupère l'état et le flux)
```

---

## 3. Les 4 Piliers de l'Implémentation SOTA

### A. Cycle de vie détaché (Detached Process)
* Au démarrage de l'application Tauri, celle-ci vérifie si le démon tourne déjà. Sinon, elle le démarre en mode détaché via l'API Rust `std::process::Command` en configurant le drapeau Windows `CREATE_NO_WINDOW` et en ignorant l'entrée/sortie standard (`Stdio::null()`).
* Le démon gère un fichier PID (`.sinew/daemon.pid`) pour garantir qu'un seul démon s'exécute par utilisateur et s'auto-coupe après 30 minutes d'inactivité complète pour économiser les ressources.

### B. Communication par Canaux Nommés (IPC Windows Named Pipes)
* Sous Windows, la communication entre l'UI Tauri et le Démon se fait via des canaux nommés (Named Pipes sous le protocole `\\.\pipe\sinew-agent-ipc`), ce qui est beaucoup plus rapide et sécurisé qu'un port réseau TCP local.
* Les requêtes de commande (démarrage de turn, interruption) et les réponses sont sérialisées sous forme de messages JSON légers.

### C. Persistance dans SQLite (Registre central de chantier)
* Le démon et Tauri partagent l'accès au même fichier SQLite central (`.sinew/conversations.db`).
* À chaque fois que l'agent exécute un outil ou reçoit un retour LLM, le démon écrit directement l'état mis à jour dans SQLite. Si la fenêtre est fermée, aucune information n'est perdue.

### D. Reconnexion et Rejeu (Re-attach Stream)
* En cas de réouverture de la fenêtre, l'UI envoie une requête de reconnexion pour la discussion active.
* Le démon renvoie l'historique récent des événements de turn en cours et rebranche le flux d'affichage en temps réel, permettant à l'utilisateur de retrouver sa discussion exactement là où elle en est.

---

## 4. Plan de Livraison Étape par Étape

1. **Étape 1 : Création du binaire Démon autonome**
   * Configurer un nouveau crate Rust `crates/sinew-agent-daemon` dans le workspace.
   * Y copier le runner de turn et l'intégration SQLite.
2. **Étape 2 : Implémentation du serveur IPC (Named Pipes)**
   * Utiliser le crate `tokio::net::windows::named_pipe` pour gérer les connexions entrantes de l'UI.
3. **Étape 3 : Redirection des commandes Tauri**
   * Modifier `src-tauri/src/turns.rs` pour qu'il n'exécute plus `run_turn` localement, mais délègue la requête au Named Pipe du démon en arrière-plan.
4. **Étape 4 : Gestion du cycle de vie et reconnexion dans l'UI**
   * Ajouter le code de spawn du démon au démarrage de Tauri et la fonction de reconnexion dans le store de discussion.
