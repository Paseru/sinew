# Règles d'architecture & Performance des Fournisseurs (Providers)

Ce document décrit la règle d'or à suivre impérativement lors de l'ajout ou de la modification de fournisseurs de modèles d'IA (providers) dans Sinew.

---

## ⚠️ Pas de requêtes réseau actives lors de la vérification de l'état (status)

Les commandes Tauri comme `get_*_provider_status` (par exemple `get_openai_provider_status`, `get_deepseek_provider_status`, etc.) ne doivent **jamais** effectuer de requêtes réseau actives (comme valider la clé API via un appel réseau externe) à chaque appel de statut.

### 🔴 Pourquoi c'est interdit ?
1. **Appels fréquents :** L'interface utilisateur appelle ces fonctions de statut fréquemment au démarrage, lors des re-renders de l'interface des paramètres, et lors des changements de focus de l'application.
2. **Fuites et Blocages :** Faire des requêtes réseau synchrones ou asynchrones répétées ici entraîne des ralentissements majeurs de l'UI, des risques de blocages complets de l'application (si le service tiers subit une panne ou si l'utilisateur est hors ligne), et des fuites de mémoire (accumulation de sockets et de requêtes en attente dans tokio/reqwest).
3. **Spamming des API :** Cela engendre du spam des serveurs d'API tiers et expose l'utilisateur à des blocages pour cause de taux limite de requêtes dépassé (*rate-limiting*).

### 🟢 La bonne pratique (à suivre)
À l'instar d'OpenAI, Anthropic et Google :
- **Vérification locale :** Chargez simplement l'état d'authentification enregistré localement sur le disque (`auth.connected`).
- **Retour instantané :** Si l'utilisateur est marqué connecté localement, retournez `"connected"`, sinon `"disconnected"`.
- **Validation unique :** Les requêtes de validation réseau ne doivent être effectuées **qu'une seule fois** via la commande dédiée (`validate_*_api_key`), au moment exact où l'utilisateur soumet ou modifie sa clé d'accès dans l'interface de connexion.
