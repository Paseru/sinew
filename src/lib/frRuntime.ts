import { getAppLocale } from "./locale";

const FR_RUNTIME_MARK = "sinew-fr-runtime";

const EXACT_TRANSLATIONS: Record<string, string> = {
  "Your personal Agentic IDE": "Votre IDE agentique personnel",
  "Open a folder": "Ouvrir un dossier",
  "Opening…": "Ouverture…",
  "Choose any directory to start a session": "Choisissez un dossier pour démarrer une session",
  Recent: "Récents",
  "Agent running": "Agent en cours d’exécution",
  "No conversations yet.": "Aucune conversation pour le moment.",
  Streaming: "Flux en cours",
  Rename: "Renommer",
  Delete: "Supprimer",
  Settings: "Paramètres",
  "Close tab": "Fermer l’onglet",
  "Nothing open": "Aucun fichier ouvert",
  "This file can’t be edited here.": "Ce fichier ne peut pas être modifié ici.",
  "binary or too large": "binaire ou trop volumineux",
  Search: "Rechercher",
  Clear: "Effacer",
  "Path match": "Correspondance du chemin",
  "No results": "Aucun résultat",
  "No files": "Aucun fichier",
  "Clipboard has no files to paste.": "Le presse-papiers ne contient aucun fichier à coller.",
  "Copy Image": "Copier l’image",
  "Reveal in Finder": "Afficher dans le Finder",
  "Show in Explorer": "Afficher dans l’Explorateur",
  "Show in File Manager": "Afficher dans le gestionnaire de fichiers",
  "Open file": "Ouvrir le fichier",
  "Click to zoom": "Cliquer pour zoomer",
  "Mermaid render error": "Erreur de rendu Mermaid",
  "Rendering Mermaid diagram…": "Rendu du diagramme Mermaid…",
  "Mermaid diagram preview": "Aperçu du diagramme Mermaid",
  "Zoom out": "Dézoomer",
  "Zoom in": "Zoomer",
  "Reset view": "Réinitialiser la vue",
  "Close (Esc)": "Fermer (Échap)",
  "Window controls": "Contrôles de fenêtre",
  Minimize: "Réduire",
  Close: "Fermer",
  "Close terminal": "Fermer le terminal",
  "New terminal": "Nouveau terminal",
  "Close all terminals": "Fermer tous les terminaux",
  "Restore terminal height": "Restaurer la hauteur du terminal",
  "Full height": "Pleine hauteur",
  "Hide terminal": "Masquer le terminal",
  "Switch workspace": "Changer d’espace de travail",
  "New file": "Nouveau fichier",
  "New folder": "Nouveau dossier",
  "click to dismiss": "cliquer pour masquer",
  Conversations: "Conversations",
  "New conversation": "Nouvelle conversation",
  "Show terminal": "Afficher le terminal",
  "Reading repository…": "Lecture du dépôt…",
  "Git isn't available": "Git n’est pas disponible",
  "Not a Git repository": "Ce n’est pas un dépôt Git",
  Pull: "Tirer",
  Push: "Pousser",
  Refresh: "Actualiser",
  Dismiss: "Masquer",
  Changes: "Modifications",
  Worktrees: "Worktrees",
  Branches: "Branches",
  "New worktree": "Nouveau worktree",
  "New branch": "Nouvelle branche",
  Local: "Locales",
  Remote: "Distantes",
  "Pull Request": "Pull request",
  "Open Pull Request": "Ouvrir une pull request",
  "Open in browser": "Ouvrir dans le navigateur",
  "Delete branch?": "Supprimer la branche ?",
  "Force delete branch?": "Forcer la suppression de la branche ?",
  "Force delete": "Forcer la suppression",
  Cancel: "Annuler",
  "Rename branch": "Renommer la branche",
  "Remove worktree": "Supprimer le worktree",
  "Stop the active conversation before switching": "Arrêtez la conversation active avant de changer",
  "Currently open in this window": "Actuellement ouvert dans cette fenêtre",
  "No upstream is tracked — remote sync is unavailable.": "Aucun upstream n’est suivi — la synchronisation distante est indisponible.",
  modified: "modifié",
  "App update": "Mise à jour de l’application",
  "Restart to update": "Redémarrer pour mettre à jour",
  "Update failed": "Échec de la mise à jour",
  "Downloading the update…": "Téléchargement de la mise à jour…",
  "Update downloaded — click to restart and apply": "Mise à jour téléchargée — cliquez pour redémarrer et appliquer",
  "Update required": "Mise à jour requise",
  "Downloading update": "Téléchargement de la mise à jour",
  "Finalizing installation": "Finalisation de l’installation",
  "Update ready": "Mise à jour prête",
  "Keep Sinew open — almost there": "Gardez Sinew ouvert — on y est presque",
  "Restarting…": "Redémarrage…",
  "We couldn't install the update. Sinew can't start until it's done.": "Impossible d’installer la mise à jour. Sinew ne peut pas démarrer tant qu’elle n’est pas terminée.",
  "Finalizing installation…": "Finalisation de l’installation…",
  "Downloading…": "Téléchargement…",
  "Restart now": "Redémarrer maintenant",
  "Quit Sinew": "Quitter Sinew",
  Skip: "Ignorer",
  Retry: "Réessayer",
  "Release notes": "Notes de version",
  About: "À propos",
  Providers: "Fournisseurs",
  Tools: "Outils",
  Skills: "Compétences",
  Agents: "Agents",
  "Settings sections": "Sections des paramètres",
  Saved: "Enregistré",
  Disconnected: "Déconnecté",
  Deleted: "Supprimé",
  Created: "Créé",
  "Waiting for browser confirmation…": "En attente de confirmation dans le navigateur…",
  "Waiting for browser confirmation...": "En attente de confirmation dans le navigateur…",
  "API key": "Clé API",
  "Remove API key": "Supprimer la clé API",
  "Searching…": "Recherche…",
  "No matching model.": "Aucun modèle correspondant.",
  Added: "Ajouté",
  "Remove model": "Supprimer le modèle",
  "Plan mode prompt": "Invite du mode Plan",
  "Image generation": "Génération d’images",
  "Image provider": "Fournisseur d’images",
  "Web search": "Recherche web",
  "Web search provider": "Fournisseur de recherche web",
  "linkup key": "clé LinkUp",
  "No tools": "Aucun outil",
  "Reset Plan mode prompt": "Réinitialiser l’invite du mode Plan",
  "Reset prompt": "Réinitialiser l’invite",
  "Plan mode instructions…": "Instructions du mode Plan…",
  "Reset description": "Réinitialiser la description",
  "MCP servers": "Serveurs MCP",
  Servers: "Serveurs",
  "probing…": "sonde…",
  "No MCP servers configured yet.": "Aucun serveur MCP configuré pour le moment.",
  "Use Advanced config to paste an MCP server block.": "Utilisez la configuration avancée pour coller un bloc de serveur MCP.",
  "Server returned no tools.": "Le serveur n’a renvoyé aucun outil.",
  "Sub-agents": "Sous-agents",
  "New agent": "Nouvel agent",
  "Untitled agent": "Agent sans titre",
  "Agent name": "Nom de l’agent",
  "Delete?": "Supprimer ?",
  "Description seen by the main agent": "Description visible par l’agent principal",
  Model: "Modèle",
  Thinking: "Réflexion",
  "Internal prompt": "Invite interne",
  Add: "Ajouter",
  Rescan: "Rebalayer",
  "No skills yet": "Aucune compétence pour le moment",
  "Create a folder under": "Créez un dossier dans",
  "with a": "avec un fichier",
  "Edit skill content": "Modifier le contenu de la compétence",
  Raw: "Brut",
  "Name is required.": "Le nom est requis.",
  "When should the agent reach for this skill?": "Quand l’agent doit-il utiliser cette compétence ?",
  "Drop files to attach": "Déposez des fichiers à joindre",
  "They’ll be included as context in your next message.": "Ils seront ajoutés comme contexte à votre prochain message.",
  "Back to main chat": "Retour au chat principal",
  Back: "Retour",
  "Say something": "Dites quelque chose",
  "Enter to send · Shift+Enter for newline": "Entrée pour envoyer · Maj+Entrée pour une nouvelle ligne",
  "Remove attachment": "Retirer la pièce jointe",
  "Remove from recents": "Retirer des récents",
  "Use without folder (Sandbox)": "Utiliser sans dossier",
  "Work on general tasks or system files without opening a project": "Travailler sur des tâches générales ou des fichiers du PC sans ouvrir de projet",
  "File mentions": "Mentions de fichiers",
  "Resize composer (double-click to reset)": "Redimensionner la zone de saisie (double-clic pour réinitialiser)",
  "Compaction instruction": "Instruction de compaction",
  "Optional focus, e.g. keep only X…": "Focus optionnel, par ex. ne garder que X…",
  "Compact conversation": "Compacter la conversation",
  "Cancel compaction": "Annuler la compaction",
  "Attach files": "Joindre des fichiers",
  "Connect a provider": "Connecter un fournisseur",
  Mode: "Mode",
  "No models": "Aucun modèle",
  "Compact context": "Compacter le contexte",
  Stop: "Arrêter",
  Send: "Envoyer",
  "Agent Swarm": "Essaim d’agents",
  Context: "Contexte",
  Full: "plein",
  Rollback: "Retour arrière",
  "Revert workspace changes on rollback": "Rétablir les modifications de l’espace de travail lors du retour arrière",
  "Cancel rewind": "Annuler le retour arrière",
  Running: "En cours",
  Error: "Erreur",
  Stopped: "Arrêté",
  Finished: "Terminé",
  Slept: "En veille",
  "Conversation compacted": "Conversation compactée",
  "Keep updating": "Continuer à mettre à jour",
  "Implement the plan": "Implémenter le plan",
  "Implement the plan & clear context": "Implémenter le plan et vider le contexte",
  Normal: "Normal",
  "Single agent works through the plan.": "Un seul agent exécute le plan.",
  "Agent swarm": "Essaim d’agents",
  "Multiple teammates split the work in parallel.": "Plusieurs coéquipiers se partagent le travail en parallèle.",
  "Compacted context": "Contexte compacté",
  "Compacting context…": "Compaction du contexte…",
  "Or type your own answer…": "Ou saisissez votre propre réponse…",
  Read: "Lire",
  "Preparing agents": "Préparation des agents",
  "Agents pending": "Agents en attente",
  "Stop Agent Swarm": "Arrêter l’essaim d’agents",
  "To-dos": "À faire",
  "Edit queued prompt": "Modifier l’invite en file d’attente",
  "Move queued prompt up": "Monter l’invite en file d’attente",
  "Move queued prompt down": "Descendre l’invite en file d’attente",
  "Move up": "Monter",
  "Move down": "Descendre",
  "Remove queued prompt": "Retirer l’invite en file d’attente",
  Remove: "Retirer",
  from: "de",
  teammate: "coéquipier",
  "all agents": "tous les agents",
  Act: "Action",
  Plan: "Plan",
  Goal: "Objectif",
  none: "aucun",
  low: "faible",
  medium: "moyen",
  high: "élevé",
  xhigh: "très élevé",
  max: "max",
  "in progress": "en cours",
  pending: "en attente",
  done: "terminé",
  active: "actif",
  closed: "fermé",
  "Write file": "Écrire un fichier",
  "Clean context": "Nettoyer le contexte",
  "Context compacted": "Contexte compacté",
  "Create image": "Créer une image",
  "Close todo_list": "Fermer todo_list",
  "Update todo_list": "Mettre à jour todo_list",
  Question: "Question",
  Skill: "Compétence",
  "Sub-agent": "Sous-agent",
  "Create team": "Créer une équipe",
  "Agent teammate": "Coéquipier agent",
  "Send team message": "Envoyer un message d’équipe",
  "Create task": "Créer une tâche",
  "Task list": "Liste des tâches",
  "Update task": "Mettre à jour la tâche",
  "Team status": "État de l’équipe",
  "Stop team": "Arrêter l’équipe",
  "Load MCP tool": "Charger l’outil MCP",
  "Search web": "Rechercher sur le web",
  "Fetch URL": "Récupérer l’URL",
  "Running command": "Commande en cours",
  "Interacting with command": "Interaction avec la commande",
  "Preparing edit": "Préparation de la modification",
  "Preparing write": "Préparation de l’écriture",
  "Cleaning context": "Nettoyage du contexte",
  "Finishing goal": "Finalisation de l’objectif",
  "Updating todo_list": "Mise à jour de todo_list",
  "Preparing question": "Préparation de la question",
  "Loading MCP tool": "Chargement de l’outil MCP",
  "Loading skill": "Chargement de la compétence",
  "Preparing web search": "Préparation de la recherche web",
  "Preparing web fetch": "Préparation de la récupération web",
  "Creating image": "Création de l’image",
  "Starting sub-agent": "Démarrage du sous-agent",
  "Starting Agent Swarm": "Démarrage de l’essaim d’agents",
  "Sending team message": "Envoi du message d’équipe",
  "Checking tasks": "Vérification des tâches",
  "Checking team": "Vérification de l’équipe",
  "Stopping team": "Arrêt de l’équipe",
  "Edit file": "Modifier un fichier",
  "Delete this conversation?": "Supprimer cette conversation ?",
  "Power User Mode": "Mode Power User",
  "Compact Reasoning": "Réflexion compacte",
  "Collapse detailed thought blocks by default and show only final answers.": "Masque par défaut les blocs de réflexion détaillés et n'affiche que les réponses finales.",
  "Automate everything (Git & more) behind the scenes, and enable ultra-concise, simplified plain language answers.": "Gère et automatise tout en arrière-plan (Git & autres) et active les réponses ultra-concises et simplifiées.",
  "Multi-PC Sync": "Synchronisation Multi-PC",
  "Automatically synchronize your conversations and configurations between your computers via OneDrive.": "Synchronise automatiquement vos conversations et configurations entre vos ordinateurs via OneDrive.",
  ["Enabled"]: "Activé",
  ["Disabled"]: "Désactivé",
  "Sinew is a flexible AI coding harness. You shape it: tweak the description of every tool, turn the ones you don't need off, and the assistant only sees what you keep.": "Sinew est un harnais de codage IA flexible. Vous le façonnez : ajustez la description de chaque outil, désactivez ceux dont vous n'avez pas besoin, et l'assistant ne verra que ce que vous conservez.",
  "Run it minimal with a couple of tools, or unlock the full set : shell, search, MCP, web, images, sub-agents. Multi-provider by default.": "Lancez-le en mode minimal avec quelques outils, ou débloquez l'ensemble complet : terminal, recherche, MCP, web, images, sous-agents. Multi-fournisseur par défaut.",
  "Language": "Langue",
  "Choose the interface language. Sinew reloads after a change so every panel updates cleanly.": "Choisissez la langue de l'interface. Sinew se recharge après un changement afin que chaque panneau se mette à jour proprement.",
  "Interface language": "Langue de l'interface",
  "Connect model providers for Sinew.": "Connectez des fournisseurs de modèles pour Sinew.",
  "Use OAuth to connect your Claude account for Anthropic models.": "Utilisez l'authentification OAuth pour connecter votre compte Claude pour les modèles Anthropic.",
  "Use OAuth to connect your ChatGPT account for OpenAI models.": "Utilisez l'authentification OAuth pour connecter votre compte ChatGPT pour les modèles OpenAI.",
  "Use OAuth to connect your Google account for Gemini models.": "Utilisez l'authentification OAuth pour connecter votre compte Google pour les modèles Gemini.",
  "Use OAuth to connect your Kimi account for Kimi 2.6.": "Utilisez l'authentification OAuth pour connecter votre compte Kimi pour Kimi 2.6.",
  "Signed in": "Connecté",
  "Connect a secondary OpenAI ChatGPT account.": "Connectez un compte ChatGPT OpenAI secondaire.",
  "Connecting": "Connexion en cours",
  "Connected": "Connecté",
  "Limit reached": "Limite",
  "Needs attention": "Attention requise",
  "Not connected": "Non connecté",
  "Add account": "Ajouter un compte",
  "Remove account": "Supprimer le compte",
  "Disconnect": "Se déconnecter",
  "Disconnecting...": "Déconnexion…",
  "Disconnecting…": "Déconnexion…",
  "Connect": "Se connecter",
  "Key saved": "Clé enregistrée",
  "Hide key": "Masquer la clé",
  "Show key": "Afficher la clé",
  "Type a model name…": "Saisir un nom de modèle…",
  "Save a valid key first": "Enregistrez d'abord une clé valide",
  "Adding…": "Ajout en cours…",
  "Advanced config": "Configuration avancée",
  "Hide config": "Masquer la config",
  "Use OpenAI subscription": "Utiliser l'abonnement OpenAI",
  "Connect OpenAI in Settings → Providers to use your subscription.": "Connectez OpenAI dans Paramètres → Fournisseurs pour utiliser votre abonnement.",
  "Disable OpenAI subscription mode": "Désactiver le mode d'abonnement OpenAI",
  "Enable OpenAI subscription mode": "Activer le mode d'abonnement OpenAI",
  "Gemini API key": "Clé API Gemini",
  "OpenAI API key": "Clé API OpenAI",
  "LinkUp API key": "Clé API LinkUp",
  "Classic": "Classique",
  "Default": "Par défaut",
  "Custom": "Personnalisé",
  "Create focused agents the main agent can call as tools.": "Créez des agents spécialisés que l'agent principal peut appeler comme outils.",
  "No sub-agents yet — click + to start.": "Aucun sous-agent pour le moment — cliquez sur + pour commencer.",
  "Select or create an agent": "Sélectionnez ou créez un agent",
  "Click again to confirm": "Cliquez à nouveau pour confirmer",
  "Delete agent": "Supprimer l'agent",
  "Confirm delete": "Confirmer la suppression",
  "Untitled": "Sans titre",
  "Scanning…": "Balayage…",
  "Scanning...": "Balayage…",
  "Drop SKILL.md files in .agents/skills or ~/.agents/skills.": "Déposez des fichiers SKILL.md dans .agents/skills ou ~/.agents/skills.",
  "No skills match.": "Aucune compétence correspondante.",
  "Nothing to preview": "Rien à afficher",
  "Select a skill": "Sélectionnez une compétence",
  "Delete skill": "Supprimer la compétence",
  "Confirm skill delete": "Confirmer la suppression de la compétence",
  "Deleting...": "Suppression…",
  "Deleting…": "Suppression…",
  "skill-name": "nom-de-la-competence",
  "Also delete the upstream branch": "Supprimer également la branche amont",
  "Also rename the upstream branch": "Renommer également la branche amont",
  "New name": "Nouveau nom",
  "new-name": "nouveau-nom",
  "Remove worktree with uncommitted changes?": "Supprimer le worktree avec des modifications non validées ?",
  "Remove worktree?": "Supprimer le worktree ?",
  "Remove anyway": "Supprimer quand même",
  "has commits that aren't merged into the current branch. Force deleting will": "contient des commits qui ne sont pas fusionnés dans la branche actuelle. Forcer la suppression va",
  "drop those commits": "abandonner ces commits",
  "from this repository — there is no undo.": "de ce dépôt — cette opération est irréversible.",
  "Delete the local branch": "Supprimer la branche locale",
  " The remote-tracking branch stays in place unless you opt in below.": " La branche de suivi à distance reste en place sauf si vous l'indiquez ci-dessous.",
  " No upstream is tracked for this branch.": " Aucun upstream n'est suivi pour cette branche.",
  "has": "possède",
  "uncommitted change": "modification non validée",
  "uncommitted changes": "modifications non validées",
  "at": "à",
  "Removing it will": ". Le supprimer va",
  "discard": "abandonner",
  "local changes — there is no undo.": "les modifications locales — cette opération est irréversible.",
  "Remove the worktree": "Supprimer le worktree",
  "The branch itself stays intact in the repository.": ". La branche elle-même reste intacte dans le dépôt.",
  "workspace": "espace de travail",
  "global": "global",
  "enabled": "activé",
  "off": "désactivé",
  "disabled": "désactivé",
  "No description provided.": "Aucune description fournie.",
  "No probe data yet.": "Aucune donnée de sonde pour le moment.",
  "Probing server…": "Sondage du serveur en cours…",
  "Probing server...": "Sondage du serveur en cours…",
  "Message the agent… (type @ to mention a file)": "Envoyer un message à l’agent… (saisir @ pour mentionner un fichier)",
  "Message the agent... (type @ to mention a file)": "Envoyer un message à l’agent… (saisir @ pour mentionner un fichier)",
  "Queue next prompt...": "Mettre l’invite suivante en file d’attente…",
  "Queue next prompt…": "Mettre l’invite suivante en file d’attente…",
  "Use any OpenRouter model with your own API key.": "Utilisez n’importe quel modèle OpenRouter avec votre propre clé API.",
  "Prompt injected into Plan mode": "Invite injectée en mode Plan",
  "This text is appended to the system prompt only when the conversation is in Plan mode.": "Ce texte est ajouté à l’invite système uniquement lorsque la conversation est en mode Plan.",
  "Click to preview": "Cliquer pour prévisualiser",
  "Mode locked while streaming": "Mode verrouillé pendant le flux",
  "Model locked while streaming": "Modèle verrouillé pendant le flux",
  "Thinking locked while streaming": "Réflexion verrouillée pendant le flux",
  "Fast locked while streaming": "Mode rapide verrouillé pendant le flux",
  "Click to edit from here": "Cliquer pour modifier à partir d’ici",
  "Fast on (uses more tokens)": "Mode rapide activé (consomme plus de tokens)",
  "Fast (uses more tokens)": "Mode rapide (consomme plus de tokens)",
  "Disable Fast": "Désactiver le mode rapide",
  "Enable Fast": "Activer le mode rapide",
  "File changes will be reverted": "Les modifications des fichiers seront rétablies",
  "File changes will be kept": "Les modifications des fichiers seront conservées",
  "Please activate Agent teams in settings.": "Veuillez activer les équipes d’agents dans les paramètres.",
  "Search skills": "Rechercher des compétences",
  "Copied": "Copié",
  "Copy key": "Copier la clé",
  "Agent Swarm tasks": "Tâches de l’essaim d’agents",
  "Queue": "File d’attente",
  "Collapse tasks": "Réduire les tâches",
  "Expand tasks": "Développer les tâches",
  "binary": "binaire",
  "no text diff": "aucun diff texte",
  "Binary file, diff omitted.": "Fichier binaire, diff omis.",
  "No text diff available.": "Aucun diff texte disponible.",
  "Text diff preview unavailable.": "Aperçu du diff texte indisponible.",
  "detached HEAD": "HEAD détachée",
  "git pull": "Tirer (git pull)",
  "git push": "Pousser (git push)",
  "Planning next moves": "Planification des prochaines étapes",
  "Save Image As…": "Enregistrer l’image sous…",
  "Open with Default App": "Ouvrir avec l’application par défaut",
  "Preparing questions": "Préparation des questions",
  "Question unavailable": "Question indisponible",
  "Questions Answered": "Questions répondues",
  "Question Answered": "Question répondue",
  "Send and stop questions": "Envoyer et arrêter les questions",
  "Next": "Suivant",
  "Later": "Plus tard",
  "Install & restart": "Installer et redémarrer",
};

type RegexTranslation = [RegExp, (...matches: string[]) => string];

const REGEX_TRANSLATIONS: RegexTranslation[] = [
  [/^Install update (.+)$/u, (version) => `Installer la mise à jour ${version}`],
  [/^Downloading · (.+)$/u, (progress) => `Téléchargement · ${progress}`],
  [/^Sinew (.+) · you're on (.+)$/u, (version, current) => `Sinew ${version} · version actuelle ${current}`],
  [/^(.+) → (.+)$/u, (from, to) => `${from} → ${to}`],
  [/^Restarting in (\d+)s…$/u, (seconds) => `Redémarrage dans ${seconds} s…`],
  [/^(\d+)% Full$/u, (percent) => `${percent} % rempli`],
  [/^Delete branch (.+)$/u, (name) => `Supprimer la branche ${name}`],
  [/^Remove worktree (.+)$/u, (name) => `Supprimer le worktree ${name}`],
  [/^Remove (.+)$/u, (name) => `Retirer ${name}`],
  [/^Disable (.+)$/u, (name) => `Désactiver ${name}`],
  [/^Enable (.+)$/u, (name) => `Activer ${name}`],
  [/^Delete skill (.+)$/u, (name) => `Supprimer la compétence ${name}`],
  [/^(\d+)\/(\d+) available to the main agent$/u, (enabled, total) => `${enabled}/${total} disponibles pour l'agent principal`],
  [/^(\d+)\/(\d+) available to the agent$/u, (enabled, total) => `${enabled}/${total} disponibles pour l'agent`],
  [/^Search (\d+) skills$/u, (count) => `Rechercher parmi ${count} compétences`],
  [/^(\d+) tools?$/u, (count) => `${count} outil${parseInt(count) > 1 ? "s" : ""}`],
  [/^Connected OpenAI account (.+)\.$/u, (suffix) => `Compte OpenAI connecté ${suffix}.`],
  [/^Reveal (.+) in Finder$/u, (name) => `Afficher ${name} dans le Finder`],
  [/^Switch this window to (.+)$/u, (path) => `Basculer cette fenêtre vers ${path}`],
  [/^Agent Swarm: (.+)$/u, (objective) => `Essaim d’agents : ${objective}`],
  [/^Agent: @(.+)$/u, (agent) => `Agent : @${agent}`],
  [/^Task: #(\d+)$/u, (id) => `Tâche : #${id}`],
  [/^Load (.+) · (.+)$/u, (server, tool) => `Charger ${server} · ${tool}`],
  [/^Search web: (.+)$/u, (query) => `Rechercher sur le web : ${query}`],
  [/^Fetch (.+)$/u, (url) => `Récupérer ${url}`],
  [/^OpenRouter model `(.+)`$/u, (model) => `Modèle OpenRouter « ${model} »`],
  [/^Reset (.+) description$/u, (name) => `Réinitialiser la description de ${name}`],
  [/^Remove model (.+)$/u, (name) => `Supprimer le modèle ${name}`],
  [/^Thought for (.+)s$/u, (seconds) => `Réflexion pendant ${seconds} s`],
  [/^(\d+) uncommitted change$/u, (count) => `${count} modification non validée`],
  [/^(\d+) uncommitted changes$/u, (count) => `${count} modifications non validées`],
  [/^tracks (.+)$/u, (upstream) => `suit ${upstream}`],
  [/^New version (.+)$/u, (version) => `Nouvelle version ${version}`],
  [/^You are on (.+)$/u, (version) => `Version actuelle ${version}`],
  [/^Downloading (\d+)%$/u, (percent) => `Téléchargement ${percent}%`],
  [/^Update (.+) available — click to view and install$/u, (version) => `Mise à jour ${version} disponible — cliquez pour voir et installer`],
  [/^Update failed: (.+)\. Click to retry\.$/u, (message) => `Échec de la mise à jour : ${message}. Cliquez pour réessayer.`],
];

const TEXT_EXCLUDED_SELECTOR = [
  ".md",
  ".monaco-editor",
  ".monaco-diff-editor",
  ".xterm",
  ".terminal-views",
  ".editor-md-preview",
  ".msg__body",
  ".tool-card__body",
  ".tool-card__pre",
  ".plan-writing-card__body",
  ".compaction-summary__body",
  ".team-message__text",
  ".question-tool__label",
  ".updater-lock__notes",
  "code",
  "pre",
  "kbd",
  "samp",
  "textarea",
  "input",
  "select",
  "[contenteditable='true']",
  "[contenteditable='']",
].join(",");

const ATTR_EXCLUDED_SELECTOR = [
  ".md",
  ".monaco-editor",
  ".monaco-diff-editor",
  ".xterm",
  ".terminal-views",
  ".editor-md-preview",
  ".msg__body",
  ".tool-card__body",
  ".tool-card__pre",
  ".plan-writing-card__body",
  ".compaction-summary__body",
  ".team-message__text",
  ".question-tool__label",
  ".updater-lock__notes",
  "code",
  "pre",
  "kbd",
  "samp",
  "[contenteditable='true']",
  "[contenteditable='']",
].join(",");

const TRANSLATABLE_ATTRIBUTES = ["title", "aria-label", "placeholder"] as const;

function enabled(): boolean {
  return getAppLocale() === "fr";
}

function translate(value: string): string {
  if (!value || !/[A-Za-z]/.test(value)) return value;

  const leading = value.match(/^\s*/u)?.[0] ?? "";
  const trailing = value.match(/\s*$/u)?.[0] ?? "";
  const core = value.slice(leading.length, value.length - trailing.length);
  if (!core) return value;

  const exact = EXACT_TRANSLATIONS[core];
  if (exact) return `${leading}${exact}${trailing}`;

  for (const [regex, factory] of REGEX_TRANSLATIONS) {
    const match = core.match(regex);
    if (match) return `${leading}${factory(...match.slice(1))}${trailing}`;
  }

  return value;
}

function shouldSkipTextNode(node: Text): boolean {
  const parent = node.parentElement;
  return !parent || !!parent.closest(TEXT_EXCLUDED_SELECTOR);
}

function shouldSkipAttributes(element: Element): boolean {
  return !!element.closest(ATTR_EXCLUDED_SELECTOR);
}

function translateTextNode(node: Text): void {
  if (shouldSkipTextNode(node)) return;
  const current = node.nodeValue ?? "";
  const next = translate(current);
  if (next !== current) node.nodeValue = next;
}

function translateAttributes(element: Element): void {
  if (shouldSkipAttributes(element)) return;
  for (const attr of TRANSLATABLE_ATTRIBUTES) {
    const current = element.getAttribute(attr);
    if (!current) continue;
    const next = translate(current);
    if (next !== current) element.setAttribute(attr, next);
  }
}

function translateTree(root: Node): void {
  if (root.nodeType === Node.TEXT_NODE) {
    translateTextNode(root as Text);
    return;
  }

  if (root.nodeType !== Node.ELEMENT_NODE && root.nodeType !== Node.DOCUMENT_NODE) {
    return;
  }

  if (root.nodeType === Node.ELEMENT_NODE) {
    translateAttributes(root as Element);
  }

  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT | NodeFilter.SHOW_ELEMENT);
  let node = walker.nextNode();
  while (node) {
    if (node.nodeType === Node.TEXT_NODE) {
      translateTextNode(node as Text);
    } else if (node.nodeType === Node.ELEMENT_NODE) {
      translateAttributes(node as Element);
    }
    node = walker.nextNode();
  }
}

const pending = new Set<Node>();
let scheduled = false;

function schedule(root: Node): void {
  pending.add(root);
  if (scheduled) return;
  scheduled = true;
  window.requestAnimationFrame(() => {
    scheduled = false;
    const roots = Array.from(pending);
    pending.clear();
    for (const node of roots) {
      if (node.isConnected || node.nodeType === Node.DOCUMENT_NODE) {
        translateTree(node);
      }
    }
  });
}

function patchDialogs(): void {
  const originalConfirm = window.confirm.bind(window);
  window.confirm = (message?: string) => originalConfirm(typeof message === "string" ? translate(message) : message);

  const originalAlert = window.alert.bind(window);
  window.alert = (message?: unknown) => originalAlert(typeof message === "string" ? translate(message) : message);
}

function start(): void {
  if (!enabled()) return;
  if (document.documentElement.dataset.frRuntime === FR_RUNTIME_MARK) return;
  document.documentElement.dataset.frRuntime = FR_RUNTIME_MARK;
  document.documentElement.lang = "fr";

  patchDialogs();
  translateTree(document);

  const observer = new MutationObserver((mutations) => {
    for (const mutation of mutations) {
      if (mutation.type === "characterData") {
        schedule(mutation.target);
      } else if (mutation.type === "attributes") {
        schedule(mutation.target);
      } else {
        for (const node of mutation.addedNodes) schedule(node);
      }
    }
  });

  observer.observe(document.documentElement, {
    childList: true,
    subtree: true,
    characterData: true,
    attributes: true,
    attributeFilter: [...TRANSLATABLE_ATTRIBUTES],
  });
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", start, { once: true });
} else {
  start();
}
