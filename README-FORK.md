# 🛠️ Sinew — Notes de Version & Ajustements Apportés

Ce document détaille les ajouts, fonctionnalités et scripts d'automatisation intégrés à ton fork de **Sinew**.

---

## 📌 Détail des ajouts de ton Fork

Cette version intègre des modifications pratiques destinées à faciliter la compatibilité et le confort d'utilisation au quotidien :

### 1. 🌐 Traduction Intégrale en Français
* **Fonctionnement** : Un sélecteur de langue dynamique est disponible dans le nouveau panneau `Settings > Options`.
* **Bénéfice** : Traduit instantanément toute l'interface (menus, paramètres, infobulles, retours d'outils) en français.

### 2. 📌 Question Collante (Sticky Question)
* **Fonctionnement** : Lors du défilement des longs historiques de discussion, la dernière question posée reste visible et fixée en haut.
* **Bénéfice** : Un simple clic sur cette bannière ramène instantanément en haut du chat de manière fluide.

### 3. ⚡ Panneau de Diagnostic SOTA (State of the Art)
* **Fonctionnement** : Accessible depuis `Settings > Options`, ce module graphique interroge l'outil de diagnostic en temps réel.
* **Bénéfice** : Permet de tester instantanément l'état et la version des outils système requis (Git, Node, Rust/Cargo, Python, Ripgrep).

### 🤖 4. Mode "Power User" (Activation en un clic)
* **Fonctionnement** : Activé depuis `Settings > Options > Power User Mode`.
* **Bénéfice** :
  * **Zéro jargon** : L'IA formule des réponses concises, simples et directement orientées vers l'action.
  * **Gestion Git simplifiée** : L'IA prend en charge toute la maintenance de ton dépôt en arrière-plan (vérification, pull, commit, push) sans que tu n'aies à manipuler de commandes Git.

---

## 🛠️ Scripts d'automatisation (PowerShell)

Pour ne plus te soucier de Git manuellement, lance ces scripts depuis ta console PowerShell :

* **`.\scripts\sinew-sync.ps1`** : Synchronise ton fork avec la version officielle de Sinew. À faire au début de ta session de travail.
* **`.\scripts\sinew-save.ps1`** : Sauvegarde simplement ton travail et l'envoie sur ton dépôt GitHub.
* **`.\scripts\sinew-build-save.ps1`** : Compile l'interface et l'envoie sur GitHub (recommandé après modification).
* **`.\scripts\sinew-build-save.ps1 -FullApp`** : Compile l'application installable complète (.msi / .exe) pour Windows et l'enregistre sur GitHub.
