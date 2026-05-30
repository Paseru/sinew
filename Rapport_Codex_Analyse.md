# 🧬 Rapport d'Analyse Codex : Composants et Idées pour Sinew

Ce rapport présente l'analyse de l'application Codex décompilée (`C:\Users\julie\OneDrive\Documents\Codex-Decompiled`) et identifie les fonctionnalités et l'architecture dont nous pouvons nous inspirer pour Sinew.

---

## 1. 🎛️ Le Tableau de Bord Lumineux (Intégration Clavier Physique)
* **Analogie :** L'assistant utilise les voyants lumineux d'un clavier spécialisé sur votre bureau (les claviers Work Louder) comme les feux tricolores d'un chantier pour indiquer ce qu'il fait.
* **Fonctionnement :**
  * **Bleu clignotant (respiration) :** L'assistant est en train de réfléchir ou de travailler.
  * **Blanc continu :** L'assistant est au repos (veille).
  * **Jaune continu :** L'assistant attend votre feu vert (approbation de commande).
  * **Orange continu :** L'assistant attend une réponse de l'intelligence artificielle.
  * **Rouge continu :** Une erreur s'est produite.
* **Intérêt pour Sinew :** Nous pourrions ajouter une option pour connecter Sinew à des accessoires physiques ou utiliser les lumières standards du clavier (comme le verrouillage majuscule/numérique) pour donner un retour visuel discret sans ouvrir l'application.

---

## 2. 📂 Le Staging des Outils (Évite le blocage des fichiers)
* **Analogie :** Au lieu de lancer les outils directement depuis la boîte d'installation (ce qui peut être bloqué par l'ordinateur comme un intrus), l'application copie les outils dans un tiroir temporaire propre (le dossier de travail temporaire sous AppData) en vérifiant leur signature.
* **Fonctionnement :** Les fichiers exécutables comme le moteur de recherche rapide (Ripgrep) ou le lanceur de commandes sont d'abord recopiés dans un sous-dossier sécurisé identifié par une empreinte unique calculée à partir de leur contenu.
* **Intérêt pour Sinew :** Cette méthode élimine les erreurs d'accès aux fichiers verrouillés sous Windows et assure que les binaires natifs s'exécutent de façon isolée et fiable.

---

## 🛡️ 3. La Garderie Sécurisée (Niveaux d'autorisation et Sandbox)
* **Analogie :** L'assistant travaille dans une salle de jeu fermée. Selon la confiance accordée, on lui ouvre plus ou moins de jouets (accès réseau, modification de fichiers) et on place un surveillant à la porte.
* **Fonctionnement :** L'application propose 5 modes distincts de sécurité :
  1. **Lecture seule :** L'assistant regarde mais ne peut rien modifier.
  2. **Automatique / Granulaire :** L'assistant peut écrire dans le dossier de projet, mais demande votre accord pour toute commande sensible.
  3. **Inspecteur Gardien :** Un second assistant virtuel (un sous-agent inspecteur) valide et filtre les actions de l'assistant principal avant de vous les présenter.
  4. **Accès Total (Danger) :** L'assistant a carte blanche et agit de manière 100% autonome.
* **Mécanismes techniques sous le capot (Windows) :**
  * **La Relocalisation Tactique (Contournement MSIX/WindowsApps) :** 
    * *Métaphore :* Copier ses outils d'un coffre scellé vers un établi personnel pour pouvoir s'en servir.
    * *Détail :* Les outils natifs (`codex.exe`, `codex-command-runner.exe`, etc.) sont extraits du dossier protégé d'installation (`WindowsApps`) et copiés dans `LOCALAPPDATA` de l'utilisateur pour contourner les blocages d'écriture et de droits.
  * **Le Bocal Réseau Hermétique (Windows Sandbox & WFP) :**
    * *Métaphore :* Une bulle de quarantaine étanche surveillée par un garde-barrière réseau.
    * *Détail :* L'application configure Windows Sandbox avec des filtres réseau persistants de bas niveau (Windows Filtering Platform) via des transactions systeme (`FwpmFilterAdd0`, `FwpmProviderAdd0`), interdisant tout accès local ou Internet depuis le bocal.
  * **Les Menottes Système et la Loupe (Command Runner local) :**
    * *Métaphore :* Des menottes limitant les mouvements (Capability SIDs) et une loupe qui ne montre qu'un tiroir spécifique (jonction NTFS `.codex.sandbox`) pour masquer le reste de l'ordinateur.
    * *Détail :* Les commandes locales tournent sous un profil restreint (AppContainer) et accèdent uniquement à un lien virtuel sécurisé contenant le projet.
  * **Mappage des Permissions :**
    * *Détail :* Les configurations de sécurité de l'utilisateur sont directement traduites en profils d'exécution (`readOnly` -> `read-only`, `acceptEdits` -> `workspace-write`, `dangerFullAccess` -> `danger-full-access`).
* **Intérêt pour Sinew :** Mettre en place un mode "Inspecteur Gardien" où une IA plus petite et rapide vérifie les commandes générées pour éviter les bêtises avant de demander la validation humaine.

---

## 4. 🌐 La Bulle d'affichage étanche (Sécurité Web)
* **Analogie :** Pour ouvrir des pages internet sans contaminer le reste de la maison, l'assistant utilise un écran séparé étanche qui bloque tout échange de clés de sécurité ou de mots de passe vers l'extérieur.
* **Fonctionnement :** L'affichage des pages web utilise des règles strictes qui interdisent le chargement de scripts non autorisés et isolent les formulaires.
* **Intérêt pour Sinew :** Renforcer la sécurité de notre navigateur interne (Chrome Bridge) en bloquant les tentatives d'accès aux fichiers sensibles de l'utilisateur lors de visites sur des sites web inconnus.

---

## 5. 🔌 Les Mini-Applications visuelles (MCP Apps & Sandboxed Views)
* **Analogie :** D'habitude, les outils externes (MCP) n'envoient que du texte brut. Ici, l'assistant peut afficher de vrais écrans et des pages interactives fournies par ces outils dans des cadres sécurisés (des "mini-écrans" étanches).
* **Fonctionnement :** 
  * Les serveurs MCP peuvent exposer des vues graphiques. La route `/mcp-app/:pluginId/:server/:toolName` affiche ces pages dans une balise `<webview>` d'Electron isolée par partition réseau.
  * La sécurité réseau (CSP) limite l'accès de ces mini-apps aux seuls domaines autorisés. La communication se fait par un protocole d'échange de messages sécurisé (MessagePorts).
  * Un bouton discret permet d'ouvrir les outils de développement (DevTools) de ce cadre isolé pour aider les créateurs.
* **Intérêt pour Sinew :** Permettre aux outils MCP de Sinew de dessiner de jolies interfaces utilisateur interactives (pour de la visualisation de données, des graphiques ou des formulaires complexes) en toute sécurité.

---

## 6. 📅 Le Planificateur d'Automatisations (Background Scheduler & RRule)
* **Analogie :** Un carnet de bord où l'on programme des tâches régulières à accomplir par l'assistant (comme un réveil-matin ou un calendrier de corvées).
* **Fonctionnement :**
  * L'utilisateur planifie une tâche récurrente (toutes les heures, tous les jours à une heure fixe, ou les jours de semaine).
  * Il spécifie la consigne (ex: "générer les notes de mise à jour"), le dossier de travail, le modèle d'IA et son niveau de réflexion.
  * Les planifications sont stockées dans une base de données locale (SQLite) et gérées via un traducteur de règles de récurrence (RRule/CRON).
* **Intérêt pour Sinew :** Offrir à l'utilisateur un onglet "Automations" pour programmer des tâches régulières autonomes en arrière-plan (revues de code périodiques, vérifications de sécurité régulières, etc.).

---

## 7. 🛠️ Le Brouillon de Travail et l'Auto-Réparation (Git Worktrees & Bouton Auto-Fix)
* **Analogie :** Un plan de travail jetable (espace de brouillon) où l'assistant peut faire des essais. Si les outils de compilation ou d'installation tombent en panne, un bouton magique permet à un micro-assistant de réparer lui-même les dégâts sans vous déranger.
* **Fonctionnement détaillé du bouton "Auto-réparer" :**
  1. **Détection de panne :** Lorsqu'une commande exécutée via le terminal (`bash.rs`) échoue ou qu'un build renvoie des erreurs d'analyse, l'interface utilisateur de Sinew affiche un bouton **"Auto-réparer la configuration"** à côté du rapport d'erreur.
  2. **Lancement du micro-agent de réparation :** Cliquer sur ce bouton démarre un sous-agent (`subagent.rs` en tâche de fond) dédié uniquement à la résolution de cet échec précis.
  3. **Boucle d'auto-correction (SOTA) :** 
     - L'agent lit les journaux d'erreurs (stderr) et utilise l'outil `read_lints` pour obtenir les diagnostics précis.
     - Il formule des corrections (comme réparer des dépendances obsolètes dans `package.json`, corriger des options incompatibles dans `tsconfig.json` ou ajuster des chemins d'importation cassés dans le code) et applique ces changements via `edit_file`.
     - Il réexécute la commande de build dans le répertoire de travail pour vérifier si le problème est résolu.
  4. **Validation finale :** Une fois le code d'erreur ramené à 0 (succès de compilation), l'agent présente un résumé clair des fichiers modifiés et propose à l'utilisateur d'appliquer définitivement ces corrections.
* **Intérêt pour Sinew :** Cette fonctionnalité rendra l'assistant autonome face aux erreurs de configuration typiques (erreurs TypeScript, dépendances manquantes, scripts npm cassés), évitant à l'utilisateur de devoir chercher la solution à la main.

---

## 8. ⏳ Le Régulateur de Débit de Texte (Delta Buffering Queue)
* **Analogie :** Au lieu d'essayer de lire chaque lettre au fur et à mesure qu'elle arrive (ce qui ferait clignoter et ralentir l'écran), on accumule les lettres dans un petit entonnoir pour les afficher par groupes réguliers toutes les 50 millisecondes.
* **Fonctionnement :** Le texte provenant de la console (ou de la pensée de l'IA) est placé dans une file d'attente d'affichage temporaire pour économiser les ressources de l'ordinateur et garder l'interface fluide.
* **Intérêt pour Sinew :** Améliorer la fluidité du chat et de la console interactive de Sinew lors des sorties de texte massives ou rapides.

---

## 9. 🖥️ Le Pilotage Total de l'Ordinateur (Computer Use / OS Control)
* **Analogie :** L'assistant dispose d'yeux (capture d'écran) et de mains virtuelles (clic et clavier) pour manipuler d'autres applications à votre place (comme ouvrir le Bloc-notes, naviguer sur Chrome ou compiler dans un IDE).
* **Fonctionnement :** 
  * Un panneau de contrôle permet d'autoriser l'assistant à piloter soit "N'importe quelle application", soit "Google Chrome uniquement".
  * Il effectue des captures d'écran régulières et simule des événements physiques de souris et de touches pour exécuter les instructions de l'utilisateur de manière interactive.
* **Intérêt pour Sinew :** Re-développer à notre façon (SOTA) une compétence de pilotage du système en utilisant des outils de capture d'écran légers et des déclencheurs de touches sécurisés, intégrés directement sous forme de bouton d'autorisation dans Sinew.

---

## 10. 📱 La Télécommande par Téléphone (Remote Control / Companion App)
* **Analogie :** Un écran déporté sur votre smartphone pour surveiller, parler ou donner votre accord à l'assistant à distance, comme une télécommande de télévision intelligente.
* **Fonctionnement :**
  * Association sécurisée par QR code et connexion locale par ondes réseau (WebSockets).
  * L'écran du téléphone affiche en temps réel l'état d'esprit de l'assistant (réfléchit, attend, en erreur).
  * Il permet de valider les commandes sensibles (les "feux tricolores" de sécurité) ou de dicter des instructions au micro depuis le canapé.
* **Intérêt pour Sinew :** Développer un compagnon mobile Web simple et sécurisé pour garder le contrôle de l'agent sans rester collé devant son écran d'ordinateur.

