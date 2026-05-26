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
* **🧠 Réflexion Compacte (Compact Reasoning) / Mode d'affichage** : 
  * *Option* : **Mode d'affichage** (Trois niveaux : Détaillé / Compact / Très compact).
  * *Bénéfice* : Permet d'ajuster le niveau de visibilité et de détail des étapes de réflexion de l'IA (*thinking blocks*) ainsi que des exécutions d'outils (comme les commandes ou modifications de fichiers) dans l'interface de discussion :
    * **Détaillé** : Visibilité maximale. Les longs blocs de réflexion de l'IA restent affichés en entier une fois terminés, et toutes les modifications de fichiers s'affichent sans filtre.
    * **Compact** : Vue équilibrée. Les blocs de réflexion se replient automatiquement après la génération pour n'afficher qu'un bandeau résumé (ex: *Thinking (5.2s)*).
    * **Très compact** : Focus total. Les étapes de réflexion sont visibles en temps réel lors de l'écriture puis **disparaissent complètement** une fois le message final généré, évitant toute surcharge visuelle.
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
* **🚀 Lancement et Exécution des Fichiers directement depuis les liens du Chat** :
  * *Bénéfice* : Permet de faire un clic droit sur n'importe quel lien de fichier ou chemin généré par l'IA dans vos bulles de discussion du chat (comme `installers/Sinew_0.1.25_x64-setup.exe`) pour afficher un menu contextuel et l'exécuter directement (**"Execute / Run"**), l'ouvrir dans l'éditeur ou révéler son dossier d'origine dans l'Explorateur Windows. C'est la solution ultime pour contourner le blocage des liens locaux !
* **⚡ Bouton « Influencer » dans la File d'Attente (Prompt Queue)** :
  - *Bénéfice* : Permet d'injecter et de soumettre instantanément un prompt mis en attente pour orienter, influencer et guider le flux de la discussion en cours. Ce bouton est représenté par une pilule distinctive affichant le texte « **Influencer** » accompagné d'une flèche vers le haut (`solar:arrow-up-bold`), remplaçant l'ancienne flèche simple à droite peu visible pour une interaction beaucoup plus claire et intuitive.

---

### 5. Au démarrage & Gestion des Sessions (Écran d'accueil)

* **📂 Utilisation sans dossier (Mode Bac à sable / Sandbox / Sans projet)** :
  * *Bénéfice* : Permet de lancer Sinew d'un simple clic en mode **« Sans dossier »** (ou Sandbox) depuis l'écran d'accueil sans devoir sélectionner ou créer un dossier projet. C'est parfait pour interagir rapidement avec l'IA, lui confier des tâches générales, exécuter des commandes système ou exploiter le pont Google Chrome (MCP) de façon ultra-légère. L'espace de travail s'affiche alors sous le nom de **« Sans dossier »** (géré dans un répertoire sécurisé `.sinew-sandbox` dans votre répertoire utilisateur).



