# À faire — Analyse du projet Sinew

> Rapport généré le 2026-05-29

## 🟢 Points forts (à préserver)

- Architecture propre : `sinew-core` → crates fournisseurs → `sinew-app` → `src-tauri` → UI React
- 13 crates Rust bien séparées, dépendances centralisées dans le workspace `Cargo.toml`
- CI/CD robuste : release multi-plateforme (Windows, macOS universal, Linux) + audit de sécurité
- Bundle Vite optimisé avec code splitting (Monaco, xterm, mermaid, markdown en chunks séparés)
- Gestion fine des modèles, providers, OAuth, quotas temps réel

---

## 🔴 Problématiques

### 1. Fichiers monolithiques ("god files")

| Fichier | Lignes | Ce qu'il contient |
|---|---|---|
| `src-tauri/src/lib.rs` | 1036 | Toutes les commandes Tauri : OAuth 6 providers, workspace, conversations, terminal, swarm, git, éditeur, fichiers… |
| `crates/sinew-app/src/store.rs` | 1984 | Toute la persistance SQLite : conversations, settings, checkpoints, modèles, plans, goals… |
| `src/lib/ipc.ts` | 909 | Tous les appels IPC frontend → backend dans un seul fichier |

**Conséquences :** impossible de tester unitairement, merges conflictuels fréquents, onboarding difficile.

### 2. Duplication massive entre les 7 fournisseurs

Chaque crate fournisseur (`sinew-anthropic`, `sinew-openai`, `sinew-google`, `sinew-kimi`, `sinew-openrouter`, `sinew-deepseek`, `sinew-cursor`) répète la même structure :

```
auth.rs → client.rs → lib.rs → model_info.rs → stream.rs → wire.rs
```

Seul `wire.rs` diffère réellement. Tout le reste (config, credential loading, HTTP client, user-agent) est dupliqué 7 fois.

**Conséquences :** 7x le code à maintenir, 7x les bugs potentiels, frein à l'ajout d'un nouveau fournisseur.

### 3. Faible couverture de tests

✅ Présents :
- `src-tauri/src/tests.rs` — machine à états du mode Plan
- `crates/sinew-app/src/agent/tests.rs` — clean_context, history

❌ Absents (zones critiques) :
- Auth (OAuth PKCE, device flow Kimi)
- Providers (streaming, retry, erreurs)
- Store SQLite (CRUD conversations, settings)
- Bash PTY (spawn, read, kill)
- Edit / Write (pas d'écrasement sans backup)
- MCP (connexion, outils)
- SubAgent, Team, Git, FileTree, EditorPane…
- Aucun test frontend React

### 4. Pas de formatage Rust standardisé

- Pas de `rustfmt.toml`
- Pas de `.clippy.toml`
- Seule règle lint : `too_many_arguments = "allow"` dans le workspace `Cargo.toml`

### 5. Tooling de développement Windows uniquement

- `launch-sinew-dev.bat` — pas d'équivalent bash
- Scripts MITM en PowerShell uniquement
- Chrome bridge : `.bat` + `.exe` natif

Linux et macOS ne sont que des cibles CI, pas des environnements de dev.

### 6. Gestion des erreurs trop générique

```rust
pub enum AppError {
    Auth(String),
    InvalidRequest(String),
    RateLimit(String),
    ContextLength(String),
    Network(String),
    Decode(String),
    Stream(String),
    RetryableStream { message: String, delay_ms: Option<u64> },
    Unsupported(String),
    Provider(String),
}
```

Aucune information structurée (code HTTP, provider, modèle, délai de retry). Impossible de réagir programmatiquement.

### 7. Pas de file d'attente ni rate-limiting local

Aucun backoff exponentiel, aucune queue de requêtes. Si l'utilisateur enchaîne vite ou si le mode Goal tourne en boucle, risque de 429 sans gestion.

### 8. Pas de `DESIGN.md`

Le README mentionne l'injection automatique de `DESIGN.md` mais le fichier n'existe pas. Pas de tokens CSS standardisés ni de règles UI documentées.

### 9. `CHANGELOG.md` orienté marketing

Le changelog actuel est une page de fonctionnalités/vente, pas un journal technique des modifications. Difficile de tracer l'historique réel.

---

## 🟡 Améliorations proposées

### 🔥 Priorité haute

- [ ] **1. Découper `src-tauri/src/lib.rs`** en modules :
  - `oauth.rs` — toutes les commandes OAuth (6 providers)
  - `workspace_commands.rs` — open, create, delete, import
  - `conversation_commands.rs` — list, load, save, delete, rename
  - `terminal_commands.rs` — spawn, write, resize, kill
  - `file_commands.rs` — read, write, edit, delete, trash, restore
  - `swarm_commands.rs` — team, subagent
  - `git_commands.rs` — status, branches, worktrees, PR

- [ ] **2. Découper `store.rs`** par domaine :
  - `conversation_store.rs` — CRUD conversations
  - `settings_store.rs` — tool settings, MCP settings, skill settings
  - `checkpoint_store.rs` — snapshots et restaurations
  - `model_store.rs` — mode_model_settings, OpenRouter catalog

- [ ] **3. Ajouter `rustfmt.toml`** et lancer `cargo fmt --all`

- [ ] **4. Tests sur les chemins critiques :**
  - Auth OAuth (PKCE generation, code exchange, token storage)
  - Store SQLite (create/read/update/delete conversations)
  - Edit / Write (pas d'écrasement accidentel)
  - Bash PTY (spawn → write → read → kill)

### 🟠 Priorité moyenne

- [ ] **5. Trait commun `ProviderClient`** pour réduire la duplication entre les 7 crates fournisseurs
  - Extraire : config, credential loading, HTTP client builder, user-agent
  - Garder spécifique : wire.rs (format de requête/réponse propre à chaque API)

- [ ] **6. Structurer `AppError`** avec des champs utiles :
  - `provider: Option<String>`
  - `http_status: Option<u16>`
  - `retry_after_secs: Option<u64>`

- [ ] **7. Créer un `DESIGN.md`** avec :
  - Palette de couleurs (dark theme actuel)
  - Tokens CSS (`--sinew-bg`, `--sinew-text`, `--sinew-accent`, etc.)
  - Règles de composants (boutons, cartes, inputs, modales)

- [ ] **8. Scripts de dev cross-platform :**
  - `justfile` (via `just`) ou scripts bash + PowerShell
  - Alternative : `npm run dev` unifié qui détecte l'OS

- [ ] **9. Découper `src/lib/ipc.ts`** par domaine :
  - `ipc/workspace.ts`, `ipc/conversation.ts`, `ipc/providers.ts`, `ipc/git.ts`, `ipc/terminal.ts`

### 🔵 Priorité basse

- [ ] **10. Rate limiter local avec exponential backoff** (`tokio::time::sleep` + jitter)

- [ ] **11. Tests E2E basiques** (Playwright ou Tauri WebDriver) : ouverture app → welcome → ouvrir un workspace → envoyer un message

- [ ] **12. Composants UI partagés** (Button, Card, Input, Modal) au lieu de styles inline dispersés

- [ ] **13. Mettre à jour `CHANGELOG.md`** avec un format standard (Keep a Changelog) et historique réel

---

## 📋 Résumé

| Problème | Sévérité | Remédiation |
|---|---|---|
| God files (`lib.rs`, `store.rs`, `ipc.ts`) | 🔴 Critique | Découpage modulaire (points 1, 2, 9) |
| Duplication 7 fournisseurs | 🔴 Critique | Trait commun `ProviderClient` (point 5) |
| Tests quasi absents | 🟠 Élevée | Tests sur auth, store, I/O (point 4) |
| Pas de formatage standard | 🟠 Élevée | `rustfmt.toml` (point 3) |
| Tooling Windows-only | 🟡 Moyenne | Scripts cross-platform (point 8) |
| Erreurs trop génériques | 🟡 Moyenne | Structurer `AppError` (point 6) |
| Pas de DESIGN.md | 🟡 Moyenne | Créer le fichier (point 7) |
| Pas de rate limiting | 🔵 Basse | Backoff local (point 10) |
| Pas de tests E2E | 🔵 Basse | Playwright (point 11) |
| CHANGELOG marketing | 🔵 Basse | Keep a Changelog (point 13) |

### Points supplémentaires détectés avant l'analyse

- [ ] **14. Performance visuelle** — `src/App.tsx` : l'effet de lueur au survol écoute `mousemove` sur toute la fenêtre. Remplacer par `:hover` CSS ou limiter aux composants concernés.
- [ ] **15. Dispatch des outils** — `crates/sinew-app/src/agent/tool_dispatch.rs` : longue chaîne `if / else if`. Remplacer par un `match` Rust.
- [ ] **16. Audit npm** — Lancer `npm audit fix` pour nettoyer les alertes de sécurité sur les dépendances.
- [ ] **17. Centraliser localStorage** — Regrouper la lecture/écriture des paramètres dans un hook ou module unique plutôt que des blocs `try/catch` dispersés.

---

## 📝 Rapport Simplifié (Synthèse)

- **Bureaux encombrés (Fichiers géants) :** Division des fichiers clés (`lib.rs`, `store.rs`, `ipc.ts`) en petits tiroirs spécialisés et indépendants.
- **Répétitions inutiles (Duplication des fournisseurs) :** Mutualisation des 7 connecteurs d'intelligence artificielle sous un même socle commun.
- **Manque de verrous de sécurité (Absence de tests) :** Mise en place progressive de tests de robustesse automatique sur les outils, sauvegardes et conversations.

