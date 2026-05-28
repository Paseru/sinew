# Extraction Exhaustive et Critique : Commits 30 à 90 (Tranche @Subagent_30_90)

Cette extraction recense l'ensemble des micro-améliorations, polissages visuels, innovations ergonomiques, optimisations réseau, performances front-end/back-end, et choix d'architecture introduits dans le fork de Sinew entre les **commits 30 et 90** (incluant les Pull Requests PR-2, PR-4, PR-5, PR-11 et PR-12).

---

## 1. Excellence Ergonomique, Design Premium & Intégration Native (Premium UX)

* **En-tête de question collante avec accentuation colorée (Sticky Question)** :
  * Intégration d'un conteneur à positionnement sticky (`position: sticky`, `top: 0`) dans `ChatPane.tsx` qui cible le dernier message envoyé par l'utilisateur.
  * Conception visuelle haut de gamme : support complet de l'affichage multiligne sans débordement de grille, ombre de démarcation pour le contraste lors du défilement, et ajout d'une bordure verticale lavande épurée (*lavender accent border*) sur le côté gauche.
  * **Bénéfice :** Permet à l'utilisateur de conserver un point d'ancrage contextuel immédiat lors de la lecture de longues réponses de l'IA sans avoir à remonter le fil.
* **Panneau d'apparence étendu et hautement personnalisable (Appearance Settings)** :
  * Création d'une section "Appearance" complète dans le panneau des options (`SettingsPane.tsx`) permettant de configurer finement chaque pixel de l'interface.
  * Sélecteur de thèmes harmonieux (System / Dark / Cream Light) avec sélecteurs de couleurs d'accentuation personnalisées.
  * Ajustement indépendant de la taille de police pour Monaco Editor, le Chat, et le Terminal.
  * Toggles de configuration Monaco : affichage des espaces invisibles, retour à la ligne automatique, configuration du style et du clignotement du curseur.
  * Export et import en un clic de l'intégralité des configurations utilisateur sous forme de fichier JSON local pour une portabilité totale des préférences.
* **Menu contextuel sur les onglets de l'éditeur de code** :
  * Interception des clics droits sur la barre d'onglets de `EditorPane.tsx` pour offrir des commandes de gestion de fichiers instantanées.
  * Actions proposées : "Fermer l'onglet" (`Ctrl+F4`), "Fermer les autres onglets", "Fermer tous les onglets à droite", "Copier le chemin relatif/absolu du fichier" et "Révéler dans l'Explorateur Windows / Finder macOS".
* **Localisation française complète à chaud & Guide d'onboarding** :
  * Traduction intégrale de l'interface utilisateur en français via un dictionnaire de runtime optimisé `frRuntime.ts` assurant des performances de chargement maximales sans dépendances i18n externes.
  * Ajout d'une machine à états d'onboarding interactive (`WorkspaceOnboarding.tsx`) guidant l'utilisateur pas-à-pas pour connecter ses clés API et ses serveurs d'outils MCP.
* **Ergonomie système et navigation clavier** :
  * Raccourci natif `CmdOrCtrl+W` pour la fermeture instantanée des onglets actifs de l'éditeur.
  * Bouton "Connecter un fournisseur" de l'accueil redirigeant directement vers l'onglet des paramètres via l'événement global `sinew:open-settings-section`.
  * Activation du zoom natif de la WebView Tauri via la directive `core:webview:allow-set-webview-zoom`.
  * Correction du bug de dénomination de conversation garantissant que les fils importés ou créés avec le nom temporaire "New chat" se renomment automatiquement dès le premier message.

---

## 2. Métamorphose de l'Éditeur & Algorithmes de Modification SOTA

* **Moteur d'édition par blocs avec 8 couches de correspondance floue (Fuzzy Matching)** :
  * Remplacement complet de l'ancien outil fragile `apply_patch` (suppression de 1431 lignes de code) par le duo ultra-performant `edit_file` et `write_file` basé sur des blocs `{ oldContent, newContent }`.
  * Implémentation d'une cascade algorithmique déterministe à 8 niveaux dans `edit.rs` pour appliquer avec succès les modifications même si le modèle commet des imprécisions de formatage :
    1. *Exact match* : Recherche textuelle brute et stricte.
    2. *Line-trimmed match* : Correspondance en ignorant les espaces de début et de fin de ligne.
    3. *Block-anchor match* : Pour les blocs longs (>3 lignes), ancrage rigide sur la première et la dernière ligne et comparaison du corps via la distance de Levenshtein pondérée (seuil >= 0.3).
    4. *Whitespace-normalized match* : Condensation des espaces multiples consécutifs en un seul.
    5. *Indentation-flexible match* : Alignement automatique et dynamique des marges gauches.
    6. *Escape-normalized match* : Nettoyage des caractères d'échappement invalides générés par les LLM.
    7. *Trimmed-boundary match* : Tolérance sur les délimitations supérieures et inférieures du bloc.
    8. *Context-aware match* : Validation si plus de 50% des lignes internes coïncident.
* **Parsing tolérant et autocorrectif pour LLM (Lenient Parsing)** :
  * Système de nettoyage en amont des patches générés par les modèles plus petits : retrait automatique des préfixes indésirables sur les balises de début `*** Begin Patch`, normalisation Unicode des apostrophes courbes ou espaces insécables, et correction automatique de l'omission de préfixes `+` pour les ajouts de lignes.
* **Sécurité d'écriture & Argument `replaceAll`** :
  * Préservation rigoureuse de la marque d'ordre des octets (UTF-8 BOM) et du type de fin de ligne (LF ou CRLF).
  * Détection d'intersection d'octets physiques pour empêcher la collision de modifications simultanées sur un même fichier.
  * Introduction de l'argument optionnel `replaceAll` pour remplacer globalement toutes les instances non superposées d'un bloc textuel recherché s'il s'avère non unique dans le document.

---

## 3. Souveraineté Réseau, WebSocket Temps Réel & Résilience

* **Transport temps réel par WebSockets pour OpenAI** :
  * Intégration d'un client WebSocket natif (`websocket.rs`) basé sur `tokio-tungstenite` et `rustls-tls-webpki-roots`.
  * Normalisation et unification de la logique d'analyse et de streaming réseau pour les flux SSE et WS dans `responses_stream.rs`.
  * Système de failover automatique réactif : basculement silencieux et immédiat vers le transport SSE en cas d'erreur de connexion ou de coupure du canal WebSocket.
* **Détection active de coupures et gestion des files d'attente (Queued Prompts)** :
  * Ajout de détecteurs de fermeture silencieuse de flux sans événement terminal de fin sur tous les connecteurs (Anthropic, Google, Kimi, OpenAI, OpenRouter) couplés à un timeout inactif de 300 secondes.
  * Implémentation d'un système de reconnexion automatique (2 tentatives de retry) avec backoff lors de l'établissement du flux SSE si la connexion flanche avant la production du premier token.
  * Structure de file d'attente visible via l'interface `TodoStrip.tsx` permettant d'empiler des tâches en attente pendant que l'IA génère du code, avec un bouton "Send now" pour accélérer l'envoi d'une tâche spécifique.
* **Compaction active des flux (Replay Buffer)** :
  * Intégration du tampon `replay_events` dans `ActiveTurnRecord` pour fusionner les deltas successifs de texte, de réflexion et d'outils en direct, éliminant les gels ou ralentissements d'interface React lors du rechargement de discussions volumineuses.
* **Furtivité système Windows et logging hybride** :
  * Injection systématique du flag système Windows `CREATE_NO_WINDOW` lors du lancement de serveurs MCP et de processus de recherche (ripgrep) pour éliminer les flashs inesthétiques d'invites de commandes.
  * Redirection globale des flux stdout/stderr et traces internes vers un fichier physique permanent `desktop-app.log` pour faciliter les diagnostics de stabilité hors-ligne.
  * Amélioration de la résilience du spawn PowerShell pour gérer les chemins contenant des espaces et des guillemets (PR-2).

---

## 4. Furtivité, MCP, Pont Navigateur & Automatisation Web SOTA

* **Pont de contrôle Chrome natif (Chrome Bridge & MCP)** :
  * Création d'une suite complète de navigation autonome dans `sinew-chrome-bridge/` intégrant une extension Chrome et un serveur WebSocket de messagerie native en Node.js.
  * L'IA dispose d'outils avancés pour piloter le navigateur (clics physiques, saisie, snapshots DOM, captures d'écran interactives).
* **Furtivité d'interaction réaliste (Anti-Détection)** :
  * Script injecté `sinew_cursor.js` simulant des déplacements de souris hyper-réalistes le long de courbes de Béziers de degré 3 évaluées par score de coût physique (inertie, accélération) avant d'engager le clic, contournant efficacement les détections de robots.
  * Saisie clavier asynchrone émulant la frappe humaine avec des micro-délais variables (80ms à 180ms) entre chaque pression de touche.
* **Déploiement et auto-configuration simplifiés** :
  * Script PowerShell `register.ps1` résolvant dynamiquement le chemin absolu de l'exécutable `node.exe` et l'injectant dans `native_host.bat` pour éviter les erreurs de variable d'environnement `PATH` sur Windows.
  * Enregistrement robuste dans la ruche utilisateur `HKCU` du Registre Windows pour éviter de requérir des droits d'administrateur.
  * Génération d'un lanceur de pont unifié `run_sinew_bridge.bat` fusionnant le Native Host et le serveur WebSocket pour éviter les collisions de ports et le spawn de multiples terminaux.
  * Script d'arrière-plan Python `add_to_sinew.py` se connectant à SQLite pour inscrire automatiquement les serveurs MCP WebSocket dès la fin de l'installation de l'extension.

---

## 5. Sécurisation, Multi-Comptes & Synchronisation Multi-PC

* **Authentification sécurisée PKCE OAuth** :
  * Sécurisation des flux OAuth pour Google/Antigravity via PKCE (Proof Key for Code Exchange) s'appuyant sur la crate `sha2` pour empêcher toute interception de jetons.
  * Nettoyage automatique des jetons obsolètes au démarrage de l'application (`purge_legacy_oauth_if_needed()`).
* **Intégration Antigravity & effort cognitif** :
  * Connexion au backend Antigravity / Cloud Code Assist avec normalisation des schémas d'outils pour répondre aux exigences strictes de validation d'API.
  * Contrôle fin du niveau de réflexion cognitif via le paramètre `generationConfig.thinkingConfig.thinkingLevel`.
* **Vérification intègre de l'état du disque (Checkpoints de sauvegarde)** :
  * Migration SQLite vers la version v8 ajoutant le champ `after` dans `TurnFileCheckpoint`.
  * Avant de restaurer un état précédent de l'espace de travail, Sinew compare l'état physique du disque avec l'état `after` enregistré. Si des modifications manuelles ont été faites par le développeur, la restauration est bloquée pour éviter toute perte accidentelle de code.
* **Synchronisation Multi-PC transparente via OneDrive** :
  * Scripts PowerShell intégrés (`sinew-save.ps1`, `sinew-sync.ps1`) et routines de synchronisation natives Rust dans le cycle de vie Tauri (`on_startup` et `on_exit`).
  * Sauvegarde automatique différentielle et fusion de la base SQLite locale sur OneDrive à l'ouverture et à la fermeture de l'application.
  * Toggle utilisateur dédié "Multi-PC Sync" dans l'onglet des paramètres pour activer ou suspendre la synchronisation.
* **Multi-Comptes OpenAI visuel et flexible** :
  * Support de multiples comptes OpenAI configurables sous forme de cartes d'API individuelles dans l'interface graphique avec gestionnaires de modèle, vitesse et bouton de suppression granulaire.
* **Power User Mode & Raccourcis de productivité** :
  * Bouton à bascule pour activer le "Power User Mode", qui simplifie l'interface en masquant le jargon technique et automatise la maintenance de l'espace de travail (check-out, pulls, commits automatiques).
  * Intégration de raccourcis locaux dans `POWERUSER.md` et de scripts de déploiement en un clic (`apply-sinew-fr.ps1`, `sinew-auto.ps1`).
