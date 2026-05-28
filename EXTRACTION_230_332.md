# Extraction Technique des Commits 230 à 332+ (Fork Sinew)

Ce rapport recense l'ensemble des micro-améliorations et fonctionnalités techniques issues de l'analyse chronologique de la tranche finale des commits (de 230 à 332+).

---

## 1. Souveraineté Totale & Découpage des Dépendances (gRPC Rust Bridge)
* **Retrait de Node.js au démarrage :** Migration complète du pont de communication Cursor Composer d'un modèle basé sur un sous-processus Node (`agent-bridge`) à une implémentation 100% native Rust (`crates/sinew-cursor/src/agent/bridge.rs`) utilisant hyper HTTP/2.
* **Chargement Dynamique Protobuf :** Utilisation de la crate `prost-reflect` (`proto_pool.rs`) pour le chargement et le décodage dynamique du pool de descripteurs à la compilation, garantissant une compatibilité ascendante sans figer le code Rust à chaque modification de schéma.
* **Exécution en Mémoire :** Intégration du module `exec_handler.rs` pour router et exécuter les commandes d'outils (`read_args`, `ls_args`, `write_args`, `delete_args`) directement en mémoire au sein du binaire Tauri sans spawn de processus Node.js, divisant par dix les temps de chargement et éliminant les erreurs de droits système.

---

## 2. Discrétion Réseau & Furtivité Avancée (Stealth SOTA)
* **Bypass de Télémétrie Cursor :** Chargement et forgeage des sommes de contrôle réelles de la machine physique locale (`macMachineId`) pour s'aligner sur les exigences d'authentification et de validation de l'API Composer, évitant les bannissements ou rejets de requêtes.
* **Keep-Alive persistant de l'Extension Chrome :** Implémentation d'une connexion de port permanente nommée `sinew-keep-alive` pour réveiller le Service Worker MV3 en tâche de fond toutes les 5 secondes, empêchant la mise en veille automatique imposée par Chrome.
* **Extraction OAuth robuste :** Extraction asynchrone sécurisée du jeton OAuth via des appels HTTP/2 proactifs, et gestion de minuteries de gel de flux (*freeze timers*) pour libérer proprement les handles de sockets en arrière-plan.
* **Bypass d'outils unsupported :** La méthode `resolve_tool_call` intercepte et renvoie une réponse vide formatée `composer_unsupported_tool` pour les requêtes non prises en charge de Composer (comme les recherches sémantiques propriétaires), empêchant les plantages de flux.

---

## 3. Optimisation Réseau & Auto-Guérison Système
* **Résolution des fuites mémoire de statut :** Remplacement des requêtes réseau distantes synchrones d'appel d'API de validation de clés par une lecture en cache local asynchrone, éliminant d'immenses fuites mémoire et surcharges CPU dans Tauri.
* **Lancement silencieux batch Windows :** Priorisation des extensions lors de la recherche des dépendances système dans le PATH (`.exe` -> `.cmd` -> `.bat`) et exécution enveloppée via `cmd.exe /C` sous Windows pour éliminer les retards du chargeur Win32 standard et les faux négatifs de version inconnue sur `npm`.
* **Auto-Réparation local en un clic :** Commande Tauri IPC qui orchestre de façon transparente l'enregistrement silencieux des clés de registre par PowerShell. Un bouton bleu "Configurer / Réparer le pont local" permet aux utilisateurs OneDrive multi-PC d'installer le Native Messaging localement sans ouvrir de terminal.
* **Packaging Hybride :** Intégration complète du répertoire `sinew-chrome-bridge` dans les ressources compilées de Tauri, avec résolution dynamique de chemin en production et repli vers le dossier de développement en local.

---

## 4. UI/UX Premium, Ergonomie & Visuels SOTA
* **DeepSeek V3 & Raisonnement R1 :** Conception du crate modulaire `sinew-deepseek` avec streaming asynchrone des flux SSE, validation des clés et extraction à chaud du champ `reasoning_content` de R1 affiché en direct avec des styles premium dans Tauri. Désactivation automatique des outils sur R1 pour éviter les rejets d'envoi.
* **Favicon Status Badging :** Injection dynamique d'iconographie SVG de statut encodée en base64 dans le favicon de l'onglet actif de Chrome (Orange pour actif, Rose pour recording, Teal checkmark pour terminé), offrant un retour d'information visuel direct sans quitter le navigateur.
* **Mouvement Clic Hybride (Teleport & Click) :** Mode de clic hybride combinant la rapidité de la téléportation instantanée du pointeur virtuel à proximité de la cible, suivie d'une transition finale en courbe de Béziers lente dotée d'accélération physique pour déjouer les protections anti-robots des formulaires complexes.
* **Redirection de builds OneDrive :** Scripts automatisant la génération de l'installateur Tauri (.msi / .dmg) et la copie directe sur le Bureau OneDrive pour une portabilité immédiate.
* **Option d'Exposition `autoLoad` :** Interrupteur switch dans l'interface permettant de forcer l'IA à voir et charger immédiatement tous les outils d'un serveur MCP dès son lancement sans devoir attendre une probe explicite.
