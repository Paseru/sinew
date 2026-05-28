# Agent bridge (agent.v1 Run)

Pont Node pour Sinew : encode une requête `agent.v1.AgentService/Run` en protobuf et stream les deltas texte.

## Installation

```powershell
cd C:\Dev\Sinew\scripts\agent-bridge
npm install
```

Télécharge `vendor/agent_pb.ts` depuis [cursor-oauth-opencode](https://github.com/jaredboynton/cursor-oauth-opencode).

Requiert **`npx tsx`** (dépendance npm) pour exécuter `vendor/agent_pb.ts`.

Corrections critiques du bridge :
- réponses `exec` enveloppées dans `AgentClientMessage` (pas du protobuf `ExecClientMessage` nu) ;
- `conversationState.turns` = références blob (SHA-256), pas octets inline ;
- message utilisateur courant uniquement via `userMessageAction`.

## Test manuel

```powershell
$token = (Get-Content "$env:LOCALAPPDATA\Hyrak\sinew\data\cursor-composer-auth.json" | ConvertFrom-Json).tokens.accessToken
$line = @{ accessToken=$token; modelId="composer-2.5"; systemPrompt="You are Composer"; userText="Say OK" } | ConvertTo-Json -Compress
.\test-live.ps1
# ou :
$line | npx tsx run-stream.mjs
```

## Transport dans Sinew

Par défaut Sinew utilise déjà `agent.v1` (aucune variable requise).

Pour forcer l’ancien chemin IdempotentSSE (cassé) :

```powershell
$env:SINEW_CURSOR_TRANSPORT = "idempotent"
```

Test live :

```powershell
cd C:\Dev\Sinew
cargo test -p sinew-cursor test_live_composer_request -- --nocapture
```
