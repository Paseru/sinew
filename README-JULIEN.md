# Sinew — Fork de Julien

Optimisations et ajouts personnalisés pour un usage simple, ultra-rapide et entièrement automatisé (sans jargon).

## Ajouts & Optimisations (Non présents dans l'original)

- **Dernière question sticky** : La dernière question posée reste fixée en haut de la fenêtre du chat lors du défilement. Un clic sur la bannière permet de remonter automatiquement et fluidement à la question.
- **Option de Langue (Français/Anglais)** : Option ajoutée dans `Settings > About` pour basculer l'interface en français de manière dynamique.
- **Mode Power User (Non-coder)** : Automatisation complète de la technique en arrière-plan (Git, commits, pushes, builds). L'assistant communique de manière ultra-concise et simplifiée.
- **Multi-comptes OpenAI** : Gestion et connexion multi-comptes automatisées avec des raccourcis intelligents pour optimiser la rapidité et les coûts.
- **Sinew Chrome Bridge** : Intégration d'une extension Chrome et d'un installeur automatique pour connecter le navigateur à Sinew.
- **Diagnostic SOTA (check_sota)** : Intégration d'un bouton et d'un tableau de bord de diagnostic visuel en temps réel dans Settings > About (et outil d'agent IA) pour vérifier instantanément si toutes les dépendances de pointe indispensables (ripgrep/rg, git, python, cargo, node, npm) sont installées et prêtes à l'emploi.
- **Auto-enregistrement MCP** : Détection et configuration automatiques des serveurs de connaissances (MCP) directement dans la base de données de l'application.
- **Sauvegarde des logs** : Archivage automatique de l'activité dans `desktop-app.log`.

## Corrections spécifiques (Bug fixes Windows)

- **Correction de compilation Windows** : Masquage des warnings d'imports inutilisés (liés aux menus macOS/Linux) lors de la compilation sur Windows.

## En cours de développement 🚀

- **Optimisation du pont Chrome** : Stabilisation du lanceur unifié en arrière-plan (`run_sinew_bridge.bat`).
- **Enrichissement de la traduction** : Couverture continue et progressive des boîtes de dialogue et menus secondaires.
