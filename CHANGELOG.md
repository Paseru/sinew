# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased] - 2026-05-29 15:33:18

### Added
- **Rapport d'analyse des outils (`RAPPORT_OUTILS.md`)** : Création d'un rapport complet sur l'état des outils système, de l'indexeur et de l'intégration du navigateur Sinew pour répondre aux interrogations de l'utilisateur.

## [Unreleased] - 2026-05-29 15:31:31

### Fixed
- **Settings Pane (`src/components/SettingsPane.tsx`)** : Correction de la persistence de l'option « Exposer tous les outils au démarrage » (`autoLoad`) pour les serveurs MCP. L'option était omise lors de la sérialisation des paramètres en JSON (`settingsToJson`), ce qui entraînait sa réinitialisation à chaque rechargement ou modification de la configuration. Ajout de la sérialisation de `autoLoad` dans le JSON exporté.

## [Unreleased] - 2026-05-29 15:28:08

### Fixed
- **Styles CSS (`src/styles.css`)** : Correction du problème d'affichage du menu déroulant (popover) de sélection des modèles en mode/thème IA. Remplacement de `overflow: hidden` par `overflow: visible !important` pour la boîte de composition `.composer__box` sous le sélecteur `html[data-theme="ai"]`, évitant ainsi le masquage ou le rognage des options du menu au-delà des bordures du conteneur.
