# Rapport d'Audit & Plan de Rationalisation (Refactoring)

Ce rapport consolide l'analyse globale de la base de code (Frontend React & Backend Rust) pour identifier les meilleures opportunités de rationalisation (DRY) et de refactoring.

## Top 3 des Cibles Prioritaires de Refactoring

### 1. Unification des Providers LLM (Backend Rust)
- **Problème** : L'authentification OAuth, les appels réseaux et le formatage des requêtes sont massivement dupliqués à travers 6 crates dédiés (`sinew-anthropic`, `sinew-openai`, `sinew-google`, `sinew-deepseek`, `sinew-kimi`, `sinew-openrouter`) ainsi que dans `providers.rs`.
- **Action de Refactoring** : Créer un crate ou module core pour centraliser le client HTTP, la gestion des tokens OAuth et la standardisation des requêtes/réponses. Chaque crate fournisseur deviendra une simple surcouche de mapping de payload (API-specific).

### 2. Découpage des Composants Monolithiques (Frontend React)
- **Problème** : Les composants principaux sont devenus des monolithes ingérables, notamment `SettingsPane.tsx` (plus de 8000 lignes) et `ChatPane.tsx` (plus de 6500 lignes). L'interface contient également de nombreux composants UI dupliqués (ex: icônes, formatBytes, MenuItem) répartis entre `EditorPane`, `FileTree` et `ImageContextMenu`.
- **Action de Refactoring** : 
  - Extraire les fonctions utilitaires de `ChatPane.tsx` vers un module `chatUtils.ts`.
  - Découper `SettingsPane.tsx` en sous-composants abstraits (ex: `ProviderCard.tsx`, `SystemSettings.tsx`).
  - Centraliser les composants UI partagés dans un dossier `src/components/ui`.

### 3. Standardisation du Pipeline d'Outils et d'Exécution (Backend Rust)
- **Problème** : Le passage d'arguments et le dispatch des outils se fait manuellement dans `tool_dispatch.rs` via des match/if complexes. De plus, la logique d'exécution des tours de l'agent est éclatée entre `turn.rs` et `turns.rs`.
- **Action de Refactoring** : Implémenter un système de `Trait` commun pour l'ensemble des outils de l'agent, avec un registre d'outils dynamique. Consolider le moteur d'exécution en une pipeline d'actions modulaire pour simplifier la boucle principale.
