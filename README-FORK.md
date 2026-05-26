# Documentation du Fork de Julien — Sinew Custom

Ce fichier liste les optimisations, corrections et scripts personnalisés de ton fork de **Sinew**.

---

## ✨ Les modifications et ajouts de ton Fork

Ces fonctionnalités personnalisées améliorent l'expérience de Sinew par rapport au projet officiel :

1. **Option de Langue (Français/Anglais)** : Une option dynamique dans `Settings > About` pour traduire toute l'interface en français.
2. **Dernière question collante (Sticky Question)** : La dernière question posée reste fixée en haut du chat pendant le défilement. Un clic dessus te ramène en haut de manière fluide.
3. **Panneau de Diagnostic SOTA** : Bouton et panneau de contrôle en direct dans `Settings > About` (avec l'outil IA `check_sota`) pour tester si tes dépendances système (Git, Node, Rust/Cargo, Python, Ripgrep) fonctionnent parfaitement.
4. **Mode "Power User Mode" (Bouton d'activation)** :
   * **C'est quoi ?** C'est le bouton situé dans `Settings > About` > *Power User Mode*.
   * **Son rôle** : Il sert à activer/désactiver l'automatisation en arrière-plan et à dire à l'IA d'utiliser des réponses simples, concises et directes (sans jargon de développeur).
5. **Corrections Windows (Bug fixes)** :
   * Détection améliorée de `git.exe`.
   * Suppression des avertissements de compilation (warnings) inutiles liés à macOS/Linux sur Windows.

---

## 🛠️ Scripts d'automatisation (PowerShell)

Pour ne plus te soucier de Git manuellement, lance ces scripts depuis ta console :

* **`.\scripts\sinew-sync.ps1`** : Synchronise ton fork avec la version officielle de Sinew. A faire au début.
* **`.\scripts\sinew-save.ps1`** : Sauvegarde simplement ton travail et l'envoie sur ton GitHub.
* **`.\scripts\sinew-build-save.ps1`** : Compile l'interface et l'envoie sur GitHub (recommandé après modification).
* **`.\scripts\sinew-build-save.ps1 -FullApp`** : Compile l'application installable complète (.exe) pour Windows et l'enregistre sur GitHub.
