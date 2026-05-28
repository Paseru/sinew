# Sinew — Fork de julienpiron.fr

Cette version a été optimisée en profondeur pour offrir une expérience utilisateur haut de gamme (SOTA), une autonomie maximale en arrière-plan, et des intégrations d'intelligence artificielle inégalées.

---

## 🖱️ Interface & Confort Visuel

* **Menu clic droit sur les onglets :** Clic droit sur n'importe quel onglet pour fermer l'onglet actif, les autres ou tous les onglets situés à sa droite. Permet également de copier le chemin du fichier ou de le révéler dans l'explorateur système (Finder/Explorer).
* **Sélecteurs de taille de police :** Boutons de réglage réactifs (`+` et `-`) dans les options pour ajuster instantanément la taille du texte de l'éditeur de code Monaco et du chat de l'IA.
* **Traduction française intégrale :** L'interface entière et toutes les actions de l'application s'adaptent automatiquement en français ou en anglais selon vos préférences.
* **Affichage des plans d'action (Planning Board) :** Visualisation dynamique et interactive des prochaines étapes planifiées par le Swarm d'IA directement dans le fil de discussion.
* **Guidage dynamique & Bouton « Influencer » :** Possibilité d'injecter des instructions en cours de route pour orienter l'agent IA sans bloquer son cycle.

---

## 💾 Autonomie, Sauvegarde & Robustesse

* **Mode Sandbox (Démarrage instantané) :** Démarrage immédiat de l'application en un clic sans avoir besoin d'ouvrir de dossier pour tester l'IA ou utiliser les outils MCP de manière isolée.
* **Sauvegarde automatique intelligente (Auto-Save SOTA) :** Enregistrement transparent en arrière-plan 1,5 seconde après l'arrêt de la frappe. Activable ou désactivable d'un simple clic dans vos options.
* **Synchronisation OneDrive Multi-PC :** Synchronisation automatique en tâche de fond de vos configurations, clés de connexion et conversations entre vos ordinateurs.
* **Diagnostic Windows OAuth résilient :** Capture automatique de l'erreur réseau typique sous Windows (code 10013) et conseils immédiats pour débloquer la connexion (WinNAT/HNS).
* **Écran de mises à jour sécurisé :** Verrouillage propre de l'interface pendant l'application des correctifs système pour éviter toute corruption de données.
* **Script de compilation OneDrive (`compil.ps1`) :** Automatisation de la génération de l'application et copie immédiate sur OneDrive pour un déploiement instantané sur vos PC.

---

## 🤖 Intégrations d'IA Avancées & Furtivité

* **Niveaux de réflexion ajustables (Display Mode) :** Choix entre 3 niveaux de détails techniques affichés dans le chat (Détaillé, Compact ou Très compact).
* **Rendu de la pensée DeepSeek R1 :** Affichage fluide et en temps réel de la réflexion interne du modèle R1 en cours de streaming.
* **Pont Cursor Composer 2.5 (agent.v1) :** Moteur haute-performance autonome sur connexions HTTP/2 persistantes gérant toutes les modifications chirurgicales de fichiers, avec installation automatique et invisible en arrière-plan.
* **Abonnement Gemini sans clé API :** Connexion directe avec votre compte Google OAuth pour la génération d'images, sans aucune clé API externe requise.
* **Sélecteur de modèles d'images :** Choix direct du générateur d'images de votre choix (OpenAI ou Gemini).
* **Furtivité & Sécurité WebSocket :** Spoofing d'empreinte réseau avancé pour éliminer tout risque de détection ou de blocage sur les flux de ChatGPT.
* **Badge d'espace de travail d'entreprise :** Détection automatique et affichage de votre profil Team / Enterprise.

---

## 🔌 Écosystème d'Outils & MCP

* **Réparation Chrome en un clic :** Bouton bleu de configuration automatique si le pont Chrome ne répond pas sur un nouveau PC.
* **Empaquetage des ressources Tauri :** Le pont local et l'extension Chrome sont intégrés directement au sein de l'installateur compilé (MSI/EXE).
* **Outils Rust optimisés :** Outils haute-performance (`list_dir` et `delete_file`) intégrés nativement à l'agent de workspace.
* **Laboratoire réseau MITM :** Outils de débogage et d'ingénierie inverse intégrés pour inspecter le trafic chiffré des outils IA.
