# 🛠️ Sinew — Notes de Version & Ajustements Apportés

Ce document détaille l'ensemble des fonctionnalités et des options personnalisées intégrées dans ce fork de **Sinew**.

---

## 📌 Fonctionnalités par panneau de configuration

Pour faciliter la navigation, les ajouts ont été classés selon leur emplacement dans l'application :

### 1. Dans le panneau `Settings > Options` (Paramètres > Options)

Ce nouvel onglet regroupe les préférences globales de l'utilisateur :

* **🌐 Interface en Français (Traduction progressive)** : 
  * *Option* : **Langue** (English / Français).
  * *Bénéfice* : Traduit dynamiquement les éléments clés de l'interface utilisateur (boutons, menus principaux, paramètres, dialogues et infobulles) en français pour une navigation plus confortable. Les termes purement techniques (sorties de terminal, code ou diagnostics) restent en anglais pour maintenir la cohérence de développement.
* **🤖 Mode "Power User"** : 
  * *Option* : **Mode Power User** (Activé / Désactivé).
  * *Bénéfice* : 
    * **Zéro jargon** : L'IA formule des réponses concises, simples et directement orientées vers l'action.
    * **Gestion Git automatique** : L'IA prend en charge la maintenance du dépôt en arrière-plan (vérification, pull, commit, push) de manière automatisée, sans nécessiter de manipuler des commandes Git complexes.
* **🧠 Réflexion Compacte (Compact Reasoning)** : 
  * *Option* : **Réflexion compacte** (Activé / Désactivé).
  * *Bénéfice* : Masque par défaut les longs blocs de réflexion détaillés (les étapes de réflexion de l'IA) et affiche uniquement les réponses finales pour une lecture plus rapide et directe.
* **🔄 Synchronisation Multi-PC** : 
  * *Option* : **Synchronisation Multi-PC** (Activé / Désactivé).
  * *Bénéfice* : Synchronise automatiquement les conversations et configurations de manière sécurisée entre différents ordinateurs via un espace OneDrive.
* **⚡ Diagnostic Système SOTA (State of the Art)** : 
  * *Option* : **Diagnostic Système SOTA** (avec bouton *Actualiser*).
  * *Bénéfice* : Permet de tester instantanément en temps réel l'état, le chemin et la version des outils système indispensables (Git, Node, Rust/Cargo, Python, Ripgrep).

---

### 2. Dans le panneau `Settings > Providers` (Paramètres > Fournisseurs)

Ce panneau gère les comptes et modèles connectés à l'application :

* **👥 Multi-comptes OpenAI (OAuth & Business Token)** :
  * *Bénéfice* : Permet de connecter plusieurs comptes OpenAI secondaires (« OpenAI 2 », « OpenAI 3 », etc.) en cliquant sur le bouton « + » et en renseignant un *Business Access Token* pour chacun d'eux. C'est idéal pour basculer facilement et instantanément entre plusieurs abonnements ou comptes différents, éviter d'atteindre les limites de requêtes (rate limits) et optimiser les coûts et la rapidité sans aucune déconnexion.

---

### 3. Dans le panneau `Settings > MCP` (Paramètres > Serveurs MCP)

Ce panneau gère les serveurs de protocole de contexte de modèle :

* **🌐 Sinew Chrome Bridge (Contrôle du Navigateur)** :
  * *Bénéfice* : Connecte en direct le navigateur Google Chrome à Sinew via un serveur MCP local pré-configuré (*Sinew Chrome*). Permet à l'assistant IA d'interagir en temps réel avec les onglets ouverts, de lire des pages web ou d'exécuter des actions automatisées directement sur le navigateur de manière sécurisée (intégration *browser-use*).

---

### 4. Directement dans l'interface de Chat

* **📌 Question Collante (Sticky Question)** :
  * *Bénéfice* : Lors du défilement des longs historiques de discussion, la dernière question posée reste visible et fixée en haut de l'écran. Un simple clic dessus permet de remonter instantanément et de manière fluide à la question.
* **📋 Sélection et Copie de Texte dans le Chat** :
  * *Bénéfice* : Permet de sélectionner et copier librement le texte des messages (réponses de l'assistant, extraits de code, questions et messages système) directement depuis la fenêtre de discussion, une action qui était auparavant bloquée par l'interface.
* **🚀 Option "Execute / Run" au clic droit dans l'Explorateur de Fichiers** :
  * *Bénéfice* : Permet de faire un clic droit sur n'importe quel fichier de l'explorateur (barre latérale gauche) et de cliquer sur **"Execute / Run"** pour l'ouvrir ou le lancer directement sur le système d'exploitation Windows (contournant les restrictions de clic sur les liens locaux du chat). Idéal pour lancer des installateurs (`.exe`, `.msi`), des scripts ou tout document avec son application par défaut.

