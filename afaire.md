# À Faire (Améliorations du projet)

## 1. Performance visuelle (Mouvement de la souris)
- **Fichier** : `src/App.tsx`
- **Problème** : L'effet de survol (lueur) écoute les mouvements de la souris sur toute la fenêtre (`window`) à chaque milliseconde. C'est très lourd pour le processeur.
- **Solution** : Utiliser un effet de survol natif (`:hover` en CSS) ou attacher cet événement uniquement aux composants directement concernés.

## 2. Organisation du moteur (Tri des outils)
- **Fichier** : `crates/sinew-app/src/agent/tool_dispatch.rs`
- **Problème** : Le système utilise une très longue chaîne de conditions `if / else if` pour distribuer les outils de l'agent.
- **Solution** : Remplacer par un système de tri natif (le `match` de Rust), beaucoup plus lisible, propre et légèrement plus rapide.

## 3. Sécurité des fondations (esbuild / Vite)
- **Fichier** : `package.json` / `package-lock.json`
- **Problème** : Une alerte de sécurité modérée a été détectée sur les outils de construction locaux (`npm audit`).
- **Solution** : Lancer un `npm audit fix` ou mettre à jour les paquets pour nettoyer la dette technique et assainir les fondations.

## 4. Rangement de la mémoire (Paramètres locaux)
- **Fichier** : Fichiers React de l'interface (Front-end)
- **Problème** : La lecture et l'écriture des paramètres dans la mémoire locale (localStorage) sont dispersées avec de multiples blocs de sécurité (`try/catch`).
- **Solution** : Créer un "tiroir" centralisé (une fonction ou *hook* React) pour regrouper toute la gestion des paramètres en un seul endroit.