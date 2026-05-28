# agent-bridge (Node)

Pont **optionnel** pour `agent.v1` — Sinew utilise par défaut le **bridge Rust** (aucune config).

## Utilisateur final

1. Ouvrir Sinew → **Réglages → Fournisseurs**
2. **Connecter Cursor** (OAuth Google ou GitHub)
3. Utiliser Composer — rien d'autre à installer ni variable d'environnement.

## Développeur

```bash
npm run prepare-agent-bridge   # une fois (ou automatique via beforeBuildCommand / beforeDevCommand)
```

Repli Node automatique si le bridge Rust échoue **et** que `node_modules` est présent (build release ou `prepare-agent-bridge`).

Forcer Node : `SINEW_CURSOR_BRIDGE=node`  
Désactiver le repli : `SINEW_CURSOR_BRIDGE_FALLBACK=0`

Tests :

```powershell
.\scripts\agent-bridge\test-live.ps1
.\scripts\agent-bridge\test-live-rust.ps1
```
