# 🚀 Sinew Custom — Documentation du Fork Optimisé

<p align="center">
  <b>Une version améliorée, francisée et hautement automatisée de l'environnement de développement IA Sinew.</b>
  <br/>
  <i>Ce fork intègre des fonctionnalités exclusives conçues pour éliminer la friction technique et offrir une expérience utilisateur fluide et sans effort sur Windows.</i>
</p>

---

## 🌟 Fonctionnalités exclusives et améliorations

Ce fork enrichit l'application officielle **Sinew** avec plusieurs optimisations majeures pour simplifier l'ergonomie, automatiser la maintenance et fiabiliser l'environnement de travail.

### 1. 🌐 Traduction Intégrale en Français
* **Fonctionnement** : Un sélecteur de langue dynamique est disponible dans le panneau `Settings > About`.
* **Bénéfice** : Traduit instantanément toute l'interface (menus, paramètres, infobulles, retours d'outils) pour un confort d'utilisation optimal sans barrière de la langue.

### 2. 📌 Question Collante (Sticky Question)
* **Fonctionnement** : Lors du défilement des longs historiques de discussion, la dernière question posée reste visible et fixée en haut de la vue de discussion.
* **Bénéfice** : Un simple clic sur cette bannière ramène instantanément l'utilisateur en haut du chat de manière fluide, éliminant les défilements manuels interminables.

### 3. ⚡ Panneau de Diagnostic SOTA (State of the Art)
* **Fonctionnement** : Accessible depuis `Settings > About`, ce module graphique interroge l'outil de diagnostic en un clic.
* **Bénéfice** : Permet de tester instantanément l'état des outils système requis (Git, Node, Rust/Cargo, Python, Ripgrep). Tout dysfonctionnement est clairement signalé avec son chemin et sa version pour un dépannage immédiat.

### 4. 🤖 Mode "Power User" & Automatisation Totale
* **Fonctionnement** : Une fois activé dans `Settings > About`, ce mode injecte des instructions de comportement avancées pour l'IA.
* **Bénéfice** :
  * **Zéro jargon** : L'IA formule des réponses concises, simples et directement orientées vers l'action, sans jargon technique superflu.
  * **Gestion Git & Synchronisation Invisible** : L'IA gère la maintenance de votre dépôt en arrière-plan sans aucune intervention manuelle. Elle vérifie si le projet est à jour, applique les mises à jour et sauvegarde les modifications.

### 5. ☁️ Synchronisation Multi-PC & Presse-papiers Partagé (OneDrive)
* **Fonctionnement** : Les modules de synchronisation en arrière-plan exploitent un espace sécurisé sous OneDrive (`OneDrive/Documents/Sinew`).
* **Bénéfice** :
  * **Conversations Synchronisées** : La base de données locale SQLite et l'historique complet des sessions de chat sont automatiquement synchronisés entre vos différents PC (Maison, Travail, etc.).
  * **Presse-papiers Universel** : Le contenu de votre presse-papiers est partagé instantanément entre vos ordinateurs de manière sécurisée.

### 6. 🪟 Optimisations Windows et Fiabilité du Build
* **Détection Git** : Amélioration substantielle de la localisation de l'exécutable `git.exe` sur l'environnement Windows.
* **Build Propre** : Nettoyage des avertissements (warnings) de compilation spécifiques à macOS ou Linux qui encombraient inutilement les logs sous Windows.

---

## ⚙️ Fonctionnement de l'automatisation en arrière-plan

Pour garantir une expérience sans friction, **toute la maintenance technique est déléguée à l'IA**. L'utilisateur n'a jamais besoin d'ouvrir une console ou de saisir des commandes Git.

### 🔄 Le moteur d'automatisation sous le capot
Le répertoire `/scripts` contient les scripts PowerShell qui orchestrent cette fluidité. L'IA les exécute d'elle-même en arrière-plan selon les besoins :

* **`sinew-sync.ps1`** : Synchronise en arrière-plan le projet avec la version officielle de Sinew, gère les fusions de code (rebase), résout les conflits temporaires et rapatrie l'état de la base de données SQLite et du presse-papiers depuis OneDrive.
* **`sinew-save.ps1`** : Enregistre, versionne et pousse les modifications locales sur le dépôt distant GitHub tout en exportant la dernière version de la base de données et du presse-papiers vers OneDrive.
* **`sinew-build-save.ps1`** : Assure la compilation propre de l'interface front-end ou de l'application Windows (.exe installable) avant de publier les résultats.
