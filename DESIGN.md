# 🎨 Charte Graphique & Règles de Design — DESIGN.md

Ce document définit les principes de design, la palette de couleurs, l'organisation visuelle et les composants d'interface utilisateur de **Sinew**.

---

## 1. 🎨 Palette de Couleurs (Thème Sombre Premium)

L'interface de Sinew utilise un thème sombre épuré avec des accents de couleurs spécifiques pour guider l'attention de l'utilisateur :

* **Fond Principal (Background)** : `#0b0b0d` (noir profond) pour un contraste optimal et moins de fatigue visuelle.
* **Panneaux & Cartes (Cards/Panels)** : `#121215` (gris très sombre) pour séparer visuellement les sections de travail.
* **Accent Primaire (Primary Action)** : `#3b82f6` (bleu vif) utilisé pour les boutons d'action principaux (CTA), les liens et les sélections actives.
* **Validation & Succès (Success/Active)** : `#10b981` (vert émeraude) pour les indicateurs de réussite, les états "connecté" des serveurs MCP et les messages validés.
* **Avertissement & Attente (Warnings)** : `#f59e0b` (orange ambré) pour signaler les limitations de quotas, les erreurs non critiques et l'attente d'une action utilisateur.
* **Erreur & Blocage (Errors)** : `#ef4444` (rouge vif) pour les échecs de commandes, les pertes de connexion et les alertes système.

---

## 2. 📐 Grille & Espacements (Layout Grid)

L'interface est structurée sur une grille logique de **8px** (multiples de 4/8/16/24/32) :

* **Marges extérieures (Page Margins)** : `24px` ou `32px` pour aérer les grands blocs.
* **Espacement des cartes (Card Spacing)** : `16px` entre chaque élément interactif pour éviter la surcharge visuelle.
* **Coins arrondis (Border Radius)** :
  * `12px` pour les cartes principales (Welcome, ToolCard, bulles de chat).
  * `8px` pour les boutons, les champs de saisie et les contrôles secondaires.
  * `6px` pour les petits badges d'état et étiquettes.

---

## 3. 🖥️ Composants Clés de l'Interface

### Page d'Accueil (Welcome Screen)
* Centrée, minimaliste et accueillante.
* Un grand logo/titre clair avec une description simple.
* Des boutons d'action volumineux (`height: 64px`) avec des icônes distinctes et colorées pour les tâches principales (Ouvrir un dossier de projet, etc.).

### Panneau de Paramètres (Settings Pane)
* Une structure claire par sections : Modèles de prompts, Fournisseurs d'IA (API Keys/OAuth) et Serveurs MCP.
* L'indicateur d'état des connexions (Pastille verte pour actif, grise pour inactif).
* Les options de configuration avec des curseurs ou boutons de bascule simples.

### Fil de Discussion (Chat Pane)
* **Boîte de saisie extensible (Chat Box)** : Permet d'écrire de longs messages. Une option permet de doubler ses dimensions pour plus de confort.
* **Question Épinglée (Sticky Question)** : La question courante reste figée en haut du fil pendant que l'utilisateur fait défiler les réponses pour garder le contexte à l'esprit.
* **Cartes d'outils (Tool Cards)** : Design compact. Si une commande bash échoue, un bouton "Auto-réparer" de couleur verte apparaît discrètement pour proposer une correction en un clic.

---

## 4. 🎛️ Règles de Design & Expérience Utilisateur

1. **Pas de jargon technique inutile** : Les explications destinées à l'utilisateur doivent être simples et imagées (par exemple, comparer une base de données à un classeur).
2. **Autonomie et clarté** : L'interface doit refléter l'état de l'agent en temps réel sans pour autant encombrer l'écran d'informations de débogage complexes (utilisation de barres d'état discrètes).
3. **Fluidité visuelle** : Transition douce sur les boutons au survol (`transition: all 0.2s ease-in-out`).
