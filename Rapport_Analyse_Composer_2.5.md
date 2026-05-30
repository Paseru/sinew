# Rapport d'Analyse : Support de Composer 2.5 Standalone dans Sinew

Ce rapport présente l'analyse des fichiers d'investigation et détaille la stratégie pour faire fonctionner **Composer 2.5** dans l'application Sinew sans nécessiter l'ouverture de l'application officielle Cursor.

---

## 1. Le problème actuel (Le robinet bloqué)

Lorsque Sinew tente de communiquer avec les serveurs de Cursor en direct (grâce à votre connexion autonome OAuth), la connexion s'établit avec succès (statut 200), mais **les réponses textuelles ne coulent jamais**. L'écran de l'utilisateur reste figé sur « Planification des prochaines étapes... » et finit par s'arrêter après un long moment d'attente (timeout).

**La cause du blocage :**
Pour des raisons de sécurité, le serveur exige une clé spéciale appelée `x-idempotent-encryption-key` (un cadenas de sécurité). 
* Si cette clé est envoyée dans un format classique ou hexadécimal, le serveur la rejette immédiatement comme invalide.
* Si elle est envoyée sous forme de texte encodé en base64url, le serveur accepte la connexion mais reste suspendu en attente d'autre chose. Le format exact attendu par le serveur reste à ce jour mystérieux et introuvable dans le code visible de l'application officielle.

---

## 2. Inventaire des outils d'investigation (La boîte à outils)

Le dossier analysé contient deux guides d'enquête (`cursor-composer-standalone.md` et `cursor-composer-probes-reference.md`), associés à plusieurs sondes automatiques situées dans le dossier `scripts/` de Sinew :
1. `probe_composer_validate.py` et `probe_composer_matrix.py` : Testent différentes combinaisons de clés de sécurité et de serveurs pour identifier laquelle débloque la situation.
2. `probe_idempotent_key_formats.py` : Varie les formats d'encodage de la clé de sécurité.
3. `probe_full_sinew_payload.py` : Rejoue une requête complète enregistrée pour voir si la taille du message influence le blocage.
4. `probe_composer_hosts.py` : Compare l'ancien guichet de réception (`api2.cursor.sh`) avec le nouveau guichet moderne (`agent.api5.cursor.sh`).

---

## 3. Évaluation des voies de résolution (Le choix du chemin)

Deux options principales s'offrent à nous pour rétablir la communication :

* **Option A : Découvrir la clé secrète (Écoute réseau)**
  * *Méthode :* Installer un outil d'écoute réseau (comme mitmproxy) sur l'ordinateur, envoyer un message depuis l'application Cursor officielle, et intercepter les paquets pour copier/coller la clé exacte et son format.
  * *Verdict :* Utile uniquement si le serveur utilise toujours ce vieux protocole.

* **Option B : Utiliser la nouvelle ligne de livraison express (Migration NAL / agent.api5)**
  * *Méthode :* Abandonner l'ancien guichet et passer par le nouveau canal de communication express (`agent.api5.cursor.sh`). C'est le chemin moderne utilisé par les versions récentes de Cursor.
  * *Verdict :* **Fortement recommandé**. Bonne nouvelle : Sinew possède déjà un « pont » de connexion autonome (en Rust et en Node) conçu pour ce nouveau canal. Ce pont est activé par défaut et gère le format de communication moderne.

---

## 4. Plan d'action recommandé

1. **Valider le pont express autonome :** S'assurer que le pont de connexion moderne de Sinew (Rust par défaut) est fonctionnel avec votre compte connecté.
2. **Améliorer les alertes de blocage :** Modifier les tests et l'interface de Sinew pour qu'ils affichent immédiatement une erreur claire en cas de non-réponse du serveur, au lieu de laisser l'utilisateur attendre indéfiniment.
3. **Écoute réseau de secours :** Si le pont express rencontre des limites, réaliser une écoute réseau ponctuelle pour capturer les en-têtes de l'application officielle et corriger la clé de sécurité.
