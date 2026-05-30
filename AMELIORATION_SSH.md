# 🧬 Plan d'Amélioration SSH "Plus que SOTA"

En analysant le code décompilé de Cursor et de Codexx, voici les trois piliers majeurs que nous mettons en place pour surpasser les standards actuels :

---

## 1. 🛡️ Le Gardien de Clés (Inspiré de Cursor)
*   **Le problème** : L'IA peut lire accidentellement vos fichiers de clés privées de connexion (comme `id_rsa`) s'ils se trouvent dans votre espace de travail.
*   **Notre amélioration** : Un filtre de sécurité bloque tout envoi ou lecture de clés de sécurité. C'est comme un garde-barrière à l'entrée de l'aéroport qui confisque les objets dangereux avant qu'ils ne montent dans l'avion.

---

## 2. ⚡ Le Robinet d'Eau Chaude (Inspiré de Codexx)
*   **Le problème** : À chaque commande envoyée sur le serveur distant, l'outil doit rouvrir une connexion, ce qui prend 2 à 3 secondes d'attente à chaque fois.
*   **Notre amélioration** : Les connexions restent ouvertes en arrière-plan pendant 10 minutes (Connection Pooling). Les commandes s'exécutent instantanément, comme un robinet d'eau chaude qui coule immédiatement sans avoir à attendre que l'eau chauffe.

---

## 3. 📖 L'Annuaire Partagé (Inspiré de Codexx)
*   **Le problème** : Les informations de connexion des serveurs distants sont souvent mélangées aux codes secrets de l'utilisateur.
*   **Notre amélioration** : Nous séparons l'adresse du serveur (partagée dans le projet) de vos clés secrètes personnelles (gardées au chaud dans votre dossier personnel Windows). C'est comme avoir un annuaire public pour trouver le nom du serveur, tout en gardant votre badge d'accès personnel dans votre propre poche.
