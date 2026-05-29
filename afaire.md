# À faire — priorités fiables du projet Sinew

> Nettoyé le 2026-05-29 16:36. Ce fichier remplace l'ancien rapport : il garde uniquement les constats vérifiés et les actions qui peuvent avoir un vrai impact.

## Synthèse de l'ancien rapport

L'ancien fichier mélangeait des constats sûrs, des hypothèses et des idées secondaires. La version ci-dessous conserve seulement :

- ce qui a été vérifié par les contrôles locaux ;
- ce qui peut bloquer une livraison ;
- ce qui peut réduire fortement les bugs, les risques ou la difficulté de maintenance.

## État confirmé

- Git était à jour avec `origin/main` lors de la vérification.
- Le frontend compile : `npm run build` passe.
- Le backend Rust compile : `cargo check --workspace --all-targets` passe.
- L'audit npm est propre sur les 3 paquets : racine, `sinew-chrome-bridge`, `scripts/agent-bridge`.
- Les diagnostics éditeur ne signalent aucune erreur.
- `cargo clippy` n'est pas installé localement.
- `DESIGN.md` est absent alors que le README annonce son injection automatique.

---

## Priorité 1 — Fiabiliser les contrôles avant livraison

### 1. Corriger le test Rust bloquant

- [ ] Stabiliser `bash::tests::interactive_session_accepts_input` sur Windows.

**Pourquoi c'est important :** la suite de tests peut échouer ou rester bloquée. C'est comme un voyant rouge sur le tableau de bord : tant qu'il reste allumé, une livraison peut cacher une panne.

### 2. Remettre `clippy` dans les contrôles locaux

- [ ] Installer/activer `cargo clippy` localement.
- [ ] Ajouter son lancement dans les contrôles habituels quand il sera disponible.

**Pourquoi c'est important :** `clippy` repère les erreurs discrètes et les mauvaises habitudes avant qu'elles deviennent coûteuses.

### 3. Créer un contrôle unique du projet

- [ ] Ajouter un script simple `check` qui lance au minimum :
  - `npm run build`
  - `cargo check --workspace --all-targets`
  - `cargo test --workspace --no-fail-fast`
  - les 3 audits npm
  - `cargo clippy` quand installé

**Pourquoi c'est important :** un seul bouton de contrôle évite les oublis.

---

## Priorité 2 — Découper les fichiers trop gros

Les fichiers suivants sont confirmés comme très volumineux :

| Fichier | Taille constatée | Action utile |
|---|---:|---|
| `src/components/SettingsPane.tsx` | 7138 lignes | Découper par panneaux : comptes, modèles, outils, apparence, avancé |
| `src/components/chat/ChatPane.tsx` | 6501 lignes | Découper : messages, saisie, pièces jointes, plan, équipe, aperçu image |
| `sinew-chrome-bridge/server.js` | 2788 lignes | Séparer : serveur, sécurité, Chrome, actions, macros, interface locale |
| `src-tauri/src/providers.rs` | 2455 lignes | Séparer : connexion, quotas, modèles, état par fournisseur |
| `src/components/Workspace.tsx` | 2226 lignes | Séparer : disposition, onglets, terminal, fichiers, actions |

**Pourquoi c'est important :** ces fichiers sont des gros classeurs. Les découper en dossiers plus petits rend les changements plus sûrs, plus rapides et plus faciles à relire.

---

## Priorité 3 — Resserrer les zones sensibles

- [ ] Réduire progressivement la portée du protocole asset Tauri actuellement très large : `**/*`.
- [ ] Auditer le terminal intégré.
- [ ] Auditer le pont Chrome local.
- [ ] Auditer l'ouverture de liens externes.
- [ ] Auditer la lecture de fichiers hors espace de travail.

**Pourquoi c'est important :** ce ne sont pas forcément des bugs aujourd'hui, mais ce sont des portes puissantes. Une porte puissante doit être petite, claire et bien surveillée.

---

## Priorité 4 — Garder les dépendances propres

- [ ] Garder l'audit npm à zéro vulnérabilité.
- [ ] Ne pas lancer `npm audit fix` sans alerte réelle : l'audit actuel est propre.
- [ ] Mettre à jour les grosses dépendances par petits lots testés.
- [ ] Surveiller les dépendances Rust en double et supprimer celles qui sont faciles à éviter.

**Pourquoi c'est important :** les dépendances sont comme des pièces importées. Plus elles sont nombreuses et anciennes, plus l'entretien devient lourd.

---

## Priorité 5 — Créer le document de design annoncé

- [ ] Ajouter `DESIGN.md` à la racine.
- [ ] Y documenter les couleurs, tailles, boutons, cartes, champs et règles visuelles principales.

**Pourquoi c'est important :** le README annonce ce fichier, mais il n'existe pas. Le créer donnera une boussole claire pour toutes les futures modifications d'interface.

---

## Points retirés de l'ancien plan

Ces sujets ne sont pas gardés comme priorités, soit parce qu'ils ne sont pas assez prouvés par l'analyse actuelle, soit parce que leur effet est moins direct :

- refonte générale des 7 fournisseurs ;
- refonte complète de `AppError` ;
- rate limiting local ;
- scripts de développement macOS/Linux ;
- remarque sur le `mousemove` visuel ;
- remplacement d'une chaîne `if/else` par un `match` ;
- lancement de `npm audit fix` malgré un audit propre ;
- réorganisation générale du changelog.

## Ordre conseillé

1. Corriger le test Rust bloquant.
2. Ajouter le script `check`.
3. Installer/brancher `clippy`.
4. Découper `SettingsPane.tsx` puis `ChatPane.tsx`.
5. Auditer les zones sensibles.
6. Créer `DESIGN.md`.
