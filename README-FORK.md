# 🛠️ Sinew — Notes de Version & Ajustements Apportés

<p align="center">
  <b>Adaptation, traduction et ajustements spécifiques pour l'environnement de développement IA Sinew.</b>
  <br/>
  <i>Ce document détaille les ajouts, les corrections de bugs et les options pratiques intégrés à cette version pour faciliter la prise en main, notamment sous Windows.</i>
</p>

---

## 📌 Détail des ajouts et corrections apportés

Cette version intègre quelques modifications pratiques destinées à faciliter la compatibilité et le confort d'utilisation au quotidien.

### 1. 🌐 Traduction Intégrale en Français
* **Fonctionnement** : Un sélecteur de langue dynamique est disponible dans le panneau `Settings > About`.
* **Bénéfice** : Traduit instantanément toute l'interface (menus, paramètres, infobulles, retours d'outils) pour un confort d'utilisation optimal sans barrière de la langue.

### 2. 📌 Question Collante (Sticky Question)
* **Fonctionnement** : Lors du défilement des longs historiques de discussion, la dernière question posée reste visible et fixée en haut de la vue de discussion.
* **Bénéfice** : Un simple clic sur cette bannière ramène instantanément l'utilisateur en haut du chat de manière fluide, éliminant les défilements manuels interminables.

### 3. ⚡ Panneau de Diagnostic SOTA (State of the Art)
* **Fonctionnement** : Accessible depuis `Settings > About`, ce module graphique interroge l'outil de diagnostic en un clic.
* **Bénéfice** : Permet de tester instantanément l'état des outils système requis (Git, Node, Rust/Cargo, Python, Ripgrep). Tout dysfonctionnement est clairement signalé avec son chemin et sa version pour un dépannage immédiat.

### 4. 🤖 Mode "Power User" (Activation en un clic)
* **Fonctionnement** : Activé depuis `Settings > About > Power User Mode`.
* **Bénéfice** : 
  * **Zéro jargon** : L'IA formule des réponses concises, simples et directement orientées vers l'action.
  * **Gestion Git simplifiée** : L'IA prend en charge la maintenance de votre dépôt en arrière-plan (vérification, pull, push) à votre demande sans que vous ayez à manipuler de commandes Git.

### 5. 🪟 Optimisations Windows et Fiabilité
* **Détection Git** : Amélioration substantielle de la localisation de l'exécutable `git.exe` sur l'environnement Windows.
* **Build Propre** : Nettoyage des avertissements (warnings) de compilation spécifiques à macOS ou Linux qui encombraient inutilement les logs sous Windows.
