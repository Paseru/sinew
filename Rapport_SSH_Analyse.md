# 🧬 Rapport d'Analyse SSH : Antigravity, Codexx et Cursor

Ce rapport détaille comment l'accès à distance (SSH) est géré dans les trois applications décompilées, analysé par nos quatre perspectives d'agents.

---

## 1. 🔍 L'Analyse de Sinew (Structure & Architecture globale)
*   **Antigravity** : Aucun support de connexion à distance n'est présent. L'application est un outil purement local, comparable à un outil de bureau traditionnel fonctionnant uniquement sur votre ordinateur sans pouvoir franchir ses murs.
*   **Codexx** : Architecture robuste de type "pont à distance". Codexx recherche et lit vos raccourcis de connexion (le fichier de configuration SSH) pour découvrir vos serveurs. Si le moteur Codexx n'est pas présent sur la machine distante, il envoie automatiquement les fichiers nécessaires pour "construire le chalet" là-bas, puis établit un tunnel de communication bidirectionnel.
*   **Cursor** : S'appuie sur la structure standard de VS Code pour les connexions distantes, mais ajoute son propre système de "traducteur de requêtes" (`cursor-resolver`) pour connecter son assistant d'écriture intelligent à son propre serveur dans le nuage.

---

## 2. 🐚 L'Analyse de Claude Code (Exécution des commandes & Terminal)
*   **Codexx** : Pour exécuter des ordres sur la machine distante, Codexx lance la commande système standard `ssh` en tâche de fond. Pour les sessions interactives, il force l'ouverture d'un terminal interactif virtuel (en utilisant l'option `ssh -tt "TERM=dumb bash"`), garantissant que le dialogue entre le terminal local et distant ne se brouille pas.
*   **Cursor** : Utilise également des terminaux virtuels et gère le retour visuel en temps réel via des flux de données partagés, mais se concentre sur l'exécution sécurisée et standard des commandes du terminal VS Code.

---

## 3. 📝 L'Analyse de Cursor Composer (Synchronisation des fichiers & Sécurité)
*   **Cursor** : Intègre un "détecteur de fuites". Pendant l'écriture, Cursor scanne activement vos fichiers pour repérer les signatures de vos clés de sécurité privées (les fichiers commençant par `BEGIN OPENSSH PRIVATE KEY`) afin de bloquer leur envoi accidentel au cerveau d'intelligence artificielle.
*   **Codexx** : Utilise un gestionnaire de fichiers à distance (SFTP) pour lire et modifier les fichiers directement à travers son tunnel, mais ne permet pas le transfert direct de fichiers d'un serveur distant à un autre serveur distant sans passer par votre machine.

---

## 4. 🚀 L'Analyse d'Antigravity (Approche Locale & Automatisation de Navigateur)
*   **Antigravity** : Son architecture se concentre à 100% sur le contrôle et le pilotage direct de votre navigateur web local (Chrome) et de vos outils système locaux. Il n'a aucun composant ni bibliothèque pour initier ou gérer des connexions à distance.
