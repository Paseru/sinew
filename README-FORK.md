# 🛠️ Sinew — Notes de Version & Ajustements Apportés

Ce document détaille l'ensemble des fonctionnalités et options personnalisées intégrées à ton fork de **Sinew**.

---

## 📌 Détail des ajouts et options de ton Fork

Tes options de personnalisation, de connexion et de navigation sont centralisées pour optimiser ton confort au quotidien :

### 1. 🌐 Traduction Intégrale en Français
* **Localisation** : `Settings > Options` (Option *Langue*).
* **Bénéfice** : Traduit instantanément toute l'interface (menus, paramètres, infobulles, retours d'outils) en français.

### 2. 🤖 Mode "Power User"
* **Localisation** : `Settings > Options` (Option *Mode Power User*).
* **Bénéfice** :
  * **Zéro jargon** : L'IA formule des réponses concises, simples et directement orientées vers l'action.
  * **Gestion Git automatique** : L'IA prend en charge la maintenance de ton dépôt en arrière-plan (vérification, pull, commit, push) sans que tu n'aies à manipuler de commandes Git.

### 3. 🧠 Réflexion Compacte (Compact Reasoning)
* **Localisation** : `Settings > Options` (Option *Réflexion compacte*).
* **Bénéfice** : Masque par défaut les longs blocs de réflexion détaillés (les pensées de l'IA) et affiche uniquement les réponses finales pour une lecture plus rapide.

### 4. 🔄 Synchronisation Multi-PC
* **Localisation** : `Settings > Options` (Option *Synchronisation Multi-PC*).
* **Bénéfice** : Synchronise automatiquement tes conversations et configurations de manière sécurisée entre tes différents ordinateurs via OneDrive.

### 5. 👥 Multi-comptes OpenAI (OAuth & Business Token)
* **Localisation** : `Settings > Providers` (Paramètres > Fournisseurs).
* **Bénéfice** : Possibilité de connecter un deuxième compte OpenAI (« OpenAI 2 ») en collant directement un Business Access Token. C'est parfait pour basculer facilement entre des abonnements ou comptes différents, éviter d'atteindre les limites de requêtes (rate limits) et optimiser la rapidité sans déconnexion permanente.

### 6. 🌐 Sinew Chrome Bridge (Contrôle du Navigateur)
* **Localisation** : `Settings > MCP` (Serveurs MCP) + extension Chrome dédiée.
* **Bénéfice** : Connecte en direct ton navigateur Google Chrome à Sinew via un serveur MCP local pré-configuré (*Sinew Chrome*). Permet à l'assistant IA d'interagir en temps réel avec tes onglets ouverts, de lire des pages web ou de réaliser des actions automatisées directement sur ton navigateur de manière sécurisée (intégration *browser-use*).

### 7. ⚡ Panneau de Diagnostic SOTA (State of the Art)
* **Localisation** : `Settings > Options` (Option *Diagnostic Système SOTA*).
* **Bénéfice** : Permet de tester instantanément en temps réel l'état, le chemin et la version des outils système indispensables (Git, Node, Rust/Cargo, Python, Ripgrep).

### 8. 📌 Question Collante (Sticky Question)
* **Localisation** : Intégration automatique dans l'interface de chat.
* **Bénéfice** : Lors du défilement des longs historiques de discussion, la dernière question posée reste visible et fixée en haut. Un simple clic dessus te ramène instantanément et de manière fluide à la question.
