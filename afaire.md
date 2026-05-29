# À faire — priorités fiables du projet Sinew

> Mis à jour le 2026-05-29 17:17. Les priorités déjà traitées ont été retirées.

## État confirmé

- Git était à jour avec `origin/main` lors de la vérification.
- Le frontend compile : `npm run build` passe.
- Le backend Rust compile : `cargo check --workspace --all-targets` passe.
- Le test terminal ciblé `bash::tests::interactive_session_accepts_input` passe après stabilisation.
- Les tests live Cursor, dépendants d'un compte et du réseau, sont séparés des contrôles courants.
- `cargo clippy` est installé et branché dans le contrôle unique. Il tourne en mode rapport par défaut, avec un mode strict activable via `SINEW_STRICT_CLIPPY=1`.
- Un contrôle unique existe pour les contrôles courants : `npm run check`.
- L'audit npm est propre sur les 3 paquets : racine, `sinew-chrome-bridge`, `scripts/agent-bridge`.
- Les diagnostics éditeur ne signalent aucune erreur.
- `DESIGN.md` est absent alors que le README annonce son injection automatique.

---

## Priorité 1 — Découper les fichiers trop gros

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

## Priorité 2 — Resserrer les zones sensibles

- [ ] Réduire progressivement la portée du protocole asset Tauri actuellement très large : `**/*`.
- [ ] Auditer le terminal intégré.
- [ ] Auditer le pont Chrome local.
- [ ] Auditer l'ouverture de liens externes.
- [ ] Auditer la lecture de fichiers hors espace de travail.

**Pourquoi c'est important :** ce ne sont pas forcément des bugs aujourd'hui, mais ce sont des portes puissantes. Une porte puissante doit être petite, claire et bien surveillée.

---

## Priorité 3 — Garder les dépendances propres

- [ ] Garder l'audit npm à zéro vulnérabilité.
- [ ] Ne pas lancer `npm audit fix` sans alerte réelle : l'audit actuel est propre.
- [ ] Mettre à jour les grosses dépendances par petits lots testés.
- [ ] Surveiller les dépendances Rust en double et supprimer celles qui sont faciles à éviter.

**Pourquoi c'est important :** les dépendances sont comme des pièces importées. Plus elles sont nombreuses et anciennes, plus l'entretien devient lourd.

---

## Priorité 4 — Créer le document de design annoncé

- [ ] Ajouter `DESIGN.md` à la racine.
- [ ] Y documenter les couleurs, tailles, boutons, cartes, champs et règles visuelles principales.

**Pourquoi c'est important :** le README annonce ce fichier, mais il n'existe pas. Le créer donnera une boussole claire pour toutes les futures modifications d'interface.

## Ordre conseillé

1. Découper `SettingsPane.tsx` puis `ChatPane.tsx`.
2. Auditer les zones sensibles.
3. Garder les dépendances propres.
4. Créer `DESIGN.md`.
