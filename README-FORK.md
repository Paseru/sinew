# 🛠️ Sinew — Notes de Version & Ajustements Apportés

Ce document détaille l'ensemble des fonctionnalités et des options personnalisées intégrées dans ce fork de **Sinew**.

---

## 📌 Détail des ajouts et options du Fork

Les options de personnalisation, de connexion et de navigation sont centralisées pour optimiser le confort d'utilisation au quotidien :

### 1. 🌐 Traduction Intégrale en Français
* **Localisation** : `Settings > Options` (Option *Langue*).
* **Bénéfice** : Traduit instantanément toute l'interface (menus, paramètres, infobulles, retours d'outils) en français pour une navigation plus accessible.

### 2. 🤖 Mode "Power User"
* **Localisation** : `Settings > Options` (Option *Mode Power User*).
* **Bénéfice** :
  * **Zéro jargon** : L'IA formule des réponses concises, simples et directement orientées vers l'action.
  * **Gestion Git automatique** : L'IA prend en charge la maintenance du dépôt en arrière-plan (vérification, pull, commit, push) de manière automatisée, sans nécessiter de manipuler des commandes Git complexes.

### 3. 🧠 Réflexion Compacte (Compact Reasoning)
* **Localisation** : `Settings > Options` (Option *Réflexion compacte*).
* **Bénéfice** : Masque par défaut les longs blocs de réflexion détaillés (les étapes de réflexion de l'IA) et affiche uniquement les réponses finales pour une lecture plus rapide et directe.

### 4. 🔄 Synchronisation Multi-PC
* **Localisation** : `Settings > Options` (Option *Synchronisation Multi-PC*).
* **Bénéfice** : Synchronise automatiquement les conversations et configurations de manière sécurisée entre différents ordinateurs via un espace OneDrive.

### 5. 👥 Multi-comptes OpenAI (OAuth & Business Token)
* **Localisation** : `Settings > Providers` (Paramètres > Fournisseurs).
* **Bénéfice** : Permet de connecter plusieurs comptes OpenAI secondaires (« OpenAI 2 », « OpenAI 3 », etc.) en cliquant sur le bouton « + » et en renseignant un *Business Access Token* pour chacun d'eux. C'est idéal pour basculer facilement et instantanément entre plusieurs abonnements ou comptes différents, éviter d'atteindre les limites de requêtes (rate limits) et optimiser les coûts et la rapidité sans aucune déconnexion.

### 6. 🌐 Sinew Chrome Bridge (Contrôle du Navigateur)
* **Localisation** : `Settings > MCP` (Serveurs MCP) + extension Chrome dédiée.
* **Bénéfice** : Connecte en direct le navigateur Google Chrome à Sinew via un serveur MCP local pré-configuré (*Sinew Chrome*). Permet à l'assistant IA d'interagir en temps réel avec les onglets ouverts, de lire des pages web ou d'exécuter des actions automatisées directement sur le navigateur de manière sécurisée (intégration *browser-use*).

### 7. ⚡ Panneau de Diagnostic SOTA (State of the Art)
* **Localisation** : `Settings > Options` (Option *Diagnostic Système SOTA*).
* **Bénéfice** : Permet de tester instantanément en temps réel l'état, le chemin et la version des outils système indispensables (Git, Node, Rust/Cargo, Python, Ripgrep).

### 8. 📌 Question Collante (Sticky Question)
* **Localisation** : Intégration automatique dans l'interface de chat.
* **Bénéfice** : Lors du défilement des longs historiques de discussion, la dernière question posée reste visible et fixée en haut de l'écran. Un simple clic dessus permet de remonter instantanément et de manière fluide à la question.
