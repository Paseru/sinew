# Spike `agent.v1` — Composer standalone sans `x-idempotent-encryption-key`

> Dernière analyse : **28 mai 2026**  
> Contexte : IdempotentSSE (`aiserver.v1` + connect+json) bloqué sur header inconnu.  
> Piste recommandée : **`agent.v1.AgentService`** sur **`api2.cursor.sh`**.

---

## Ce que disent Cursor IDE + projets communautaires

| Source | Host | Endpoint principal | Content-Type | OAuth PKCE | Header idempotent |
|--------|------|-------------------|--------------|------------|-------------------|
| Sinew (actuel) | `api2.cursor.sh` | `…/StreamUnifiedChatWithToolsIdempotentSSE` | `connect+json` | Oui | **Requis → bloqué** |
| [cursor-oauth-opencode](https://github.com/jaredboynton/cursor-oauth-opencode) | `api2.cursor.sh` | `/agent.v1.AgentService/Run` | `connect+proto` | Oui | **Absent** |
| [pi-cursor-provider](https://github.com/ndraiman/pi-cursor-provider) | `api2.cursor.sh` | idem + `GetUsableModels` | `connect+proto` | Oui | **Absent** |
| [eisbaw/cursor_api_demo](https://github.com/eisbaw/cursor_api_demo) | `api2.cursor.sh` | `…/StreamUnifiedChatWithTools` (bidi) | `connect+proto` | Token **SQLite IDE** | Absent |
| Cursor IDE 3.5 workbench | `agent.api5` (diag) + `api2` | `AgentService.run` / `runSSE` / `runPoll` | `connect+proto` | via IDE | Absent dans JS |

**Correction importante :** les proxies OAuth récents n’utilisent **pas** `agent.api5` comme host principal — ils passent par **`https://api2.cursor.sh`**.  
`agent.api5.cursor.sh` apparaît surtout en diagnostic réseau IDE ; certains clients mentionnent un routage migratoire, mais le chemin documenté en open source reste `api2`.

---

## Protos / méthodes utiles (bundle Cursor 3.5)

Service `agent.v1.AgentService` (workbench + always-local) :

| RPC | Kind | Usage |
|-----|------|--------|
| `Run` | BiDi streaming | Conversation agent (chemin principal communautaire) |
| `RunSSE` | Server streaming | Lecture réponses (souvent couplé à BidiAppend) |
| `RunPoll` | Server streaming | Variante poll |
| `GetUsableModels` | Unary | Liste modèles (`composer-2`, `composer-2.5`, …) |

Pas de champ `idempotent_encryption_key` dans `StreamUnifiedChatRequestWithToolsIdempotent` côté proto — le secret Sinew est **HTTP-only** sur un endpoint legacy.

---

## Design minimal Sinew (`sinew-cursor`)

### Nouveaux fichiers (spike, pas prod complète)

```
crates/sinew-cursor/src/
  agent/
    mod.rs           # façade + sélection transport
    client.rs        # HTTP/2 POST Run / RunSSE
    connect_proto.rs # framing Connect identique à connect.rs mais bytes proto
    models.rs        # GetUsableModels (validation OAuth)
    stream.rs        # parse AgentClientMessage / chunks → ComposerEvent
  client.rs          # router: idempotent vs agent (env)
```

### Sélection transport

```rust
// env SINEW_CURSOR_TRANSPORT=idempotent pour forcer l’ancien chemin (défaut: agent)
```

- **`idempotent`** : code actuel (conservé jusqu’à déblocage MITM éventuel).
- **`agent`** : nouveau chemin ; pas de `encryption.rs` header idempotent.

### Phase 0 — script Python (sans Rust)

`scripts/probe_agent_run.py` :

1. Lire token OAuth Sinew (`cursor-composer-auth.json`).
2. `POST https://api2.cursor.sh/agent.v1.AgentService/GetUsableModels` (body vide ou `{}` encadré Connect).
3. Si 200 → OAuth compatible agent.v1.
4. (Optionnel) POST minimal `/Run` avec proto minimal emprunté à opencode.

### Phase 1 — Rust minimal

- Réutiliser `identity.rs` (checksum, client-key) — déjà OK OAuth.
- Client reqwest **HTTP/2** + `application/connect+proto`.
- Implémenter **uniquement** :
  - `GetUsableModels` (test live)
  - `RunSSE` ou `Run` bidi minimal
  - Mapper **texte** → `StreamEvent::TextDelta` (pas tools au début).

### Phase 2 — Parité Sinew

- Rejouer tools Sinew via MCP / messages agent (comme opencode : rejeter tools natifs Cursor, exposer les nôtres).
- Sessions / checkpoints (pi-cursor-provider : état en mémoire + fingerprint historique).
- `Run` + reprise tool calls (bidi ouvert).

### Dépendances possibles

- **Option A (rapide spike)** : encoder proto à la main / copier schémas depuis [cursor-oauth-opencode/src/proto](https://github.com/jaredboynton/cursor-oauth-opencode/tree/main/src/proto) (buf/prost).
- **Option B (maintenance)** : extraire `agent.v1` du bundle JS (types déjà présents) → générer Rust une fois.

---

## MITM vs agent.v1 — recommandation

| Piste | Effort | Probabilité succès Composer 2.5 OAuth |
|-------|--------|--------------------------------------|
| MITM IdempotentSSE | 1 session utilisateur | **Faible** si IDE n’appelle plus IdempotentSSE |
| agent.v1 sur api2 | 1–2 semaines dev | **Élevée** (déjà prouvé par 2+ projets OAuth) |

**Recommandation : lancer agent.v1 en parallèle**, garder MITM comme **confirmation** (30 min) que l’IDE n’utilise plus le header idempotent.

---

## Ce que l’UTILISATEUR doit faire vs le code

### Utilisateur (manuel)

1. **MITM (optionnel, 30 min)** — voir `CAPTURE-MITM.md` : confirmer si IdempotentSSE apparaît encore.
2. **Compte test** : OAuth Sinew connecté (`cursor-composer-auth.json` valide).
3. **Valider spike** : lancer `python scripts/probe_agent_run.py` quand prêt et coller la sortie.

### Code (sans utilisateur)

- `probe_agent_run.py` + tests Rust `GetUsableModels`.
- Module `agent/` + flag transport.
- Protos empruntés / générés depuis opencode ou bundle.
- Probes CI locales (`verify_all.py` reste pour idempotent).

---

## Prochaines actions concrètes (ordre)

1. ~~`probe_agent_run.py`~~ — `GetUsableModels` OK (200).
2. ~~`SINEW_CURSOR_TRANSPORT=agent`~~ — branché via `scripts/agent-bridge` + `crates/sinew-cursor/src/agent/`.
3. `cd scripts/agent-bridge && npm install` puis test `run-stream.mjs` (voir README).
4. ~~`npm install` manuel~~ — auto au build + au lancement Sinew.
5. Historique multi-tours + checkpoint persisté (`cursor-agent-conversations.json` par `cache_key`).
6. Outils Read/Write/Delete/Grep/Bash via MCP + exec natifs read/ls/write.
7. HTTP/2 inline (un seul processus Node/tsx, plus de sous-processus h2-bridge).
8. ~~Tokens usage (`tokenDelta`)~~ — `StreamEvent::Usage` en direct + `MessageStop` (barre contexte Sinew).
9. ~~Edit search-replace~~ — `old_string` / `new_string` dans `agent/tools.rs`.
10. Outils Composer visibles dans le chat (`composer_bridge` meta, pas de double exécution).
11. Bridge 100 % Rust — **phase 2+** : exec bidirectionnel, MCP, LS `shallowLayout`, pool HTTP/2 partagé (`h2_client.rs`), retry 429/5xx (`retry.rs`), test live `test_live_rust_agent_bridge`. Reste : Rust par défaut, un processus sans Node.
12. ~~Fermeture stream~~ — idle 2,5s après texte, `stepCompleted`/`turnEnded`, cap 120s, `test-live.ps1` timeout 90s.
5. MITM optionnel ; tools/sessions agent (phase 2).
