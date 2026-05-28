# Capture MITM — trancher IdempotentSSE vs agent.v1

Objectif : voir **quel chemin Cursor IDE 3.5 utilise vraiment** pour Composer 2.5.

> **Note (28 mai 2026)** : les projets OAuth communautaires ([cursor-oauth-opencode](https://github.com/jaredboynton/cursor-oauth-opencode), [pi-cursor-provider](https://github.com/ndraiman/pi-cursor-provider)) passent par **`https://api2.cursor.sh/agent.v1.AgentService/Run`** avec **`application/connect+proto`**, **sans** `x-idempotent-encryption-key`.  
> Sinew devrait probablement migrer vers ce chemin ; le MITM sert surtout à **confirmer** que l’IDE n’utilise plus IdempotentSSE.



## Scripts Windows (`scripts/mitm/`)

| Script | Role |
|--------|------|
| `scripts/mitm/install-mitmproxy.ps1` | Installe mitmproxy si absent (`winget` puis `pip`) |
| `scripts/mitm/start-mitmweb.ps1` | Demarre mitmweb (proxy **8080**, UI **8081**), affiche les URLs |
| `scripts/mitm/check-mitm.ps1` | Verifie les ports et un GET HTTP sur l'UI |

Demarrage rapide :

```powershell
cd C:\Dev\Sinew
pwsh -File scripts\mitm\install-mitmproxy.ps1   # une fois
pwsh -File scripts\mitm\start-mitmweb.ps1       # laisser la fenetre ouverte
pwsh -File scripts\mitm\check-mitm.ps1           # controle
```

Guide pas a pas : `scripts/mitm/README.md`.

### Depannage Windows courant

- **`ERR_CONNECTION_REFUSED` sur http://127.0.0.1:8081** : mitmweb n'est pas en cours d'execution → lancer `start-mitmweb.ps1` avant d'ouvrir l'UI.
- **Cursor / navigateur sans Internet** : proxy systeme actif (`127.0.0.1:8080`) alors que mitmweb est arrete → relancer mitmweb ou **desactiver le proxy**.
- **TLS / connexion HTTPS refusee** : certificat racine mitmproxy non installe → Options mitmweb *Install mitmproxy CA* ou importer `%USERPROFILE%\.mitmproxy\mitmproxy-ca-cert.cer`.
- **Port 8080 ou 8081 deja utilise** : ancienne instance mitm → `check-mitm.ps1`, fermer le processus concerne.
- **`mitmweb` introuvable** : `where mitmweb` vide → `install-mitmproxy.ps1` ou `winget install mitmproxy.mitmproxy -e`.

Voir aussi : `scripts/AGENT-SPIKE-DESIGN.md`, `scripts/probe_agent_run.py`.
## Prérequis

- [mitmproxy](https://mitmproxy.org/) installé (`mitmweb` recommandé)
- Cursor IDE 3.5.x installé
- Compte Cursor connecté dans l’IDE

## Étapes

1. Installer le certificat mitmproxy dans Windows (Trusted Root) :
   - Lancer `mitmweb`, menu **Options → Install mitmproxy CA**
   - Ou exporter `%USERPROFILE%\.mitmproxy\mitmproxy-ca-cert.cer` et l’importer manuellement

2. Configurer le proxy système Windows (Paramètres → Réseau → Proxy) :
   - `127.0.0.1:8080` (port par défaut mitmproxy)

3. Lancer `pwsh -File scripts\mitm\start-mitmweb.ps1` (ou `mitmweb`) et ouvrir l’UI http://127.0.0.1:8081

4. Ouvrir **Cursor IDE**, envoyer un message **Composer 2.5** (mode Agent)

5. Dans mitmweb, filtrer :
   - Host : `api2.cursor.sh` (principal), `agent.api5.cursor.sh` (diag / routage)
   - Path : `agent.v1.AgentService/Run`, `RunSSE`, `GetUsableModels`
   - Path legacy : `Idempotent`, `StreamUnified` (si encore présent)

6. Pour chaque requête POST pertinente, noter :
   - URL complète (host + path)
   - `content-type` (`connect+proto` vs `connect+json`)
   - Headers `x-*` (surtout idempotent / blob **si** IdempotentSSE)
   - Premier frame body (hex des 200 premiers octets)

7. Comparer avec le dump Sinew :
   ```powershell
   cd C:\Dev\Sinew
   cargo test -p sinew-cursor dump_outgoing_composer_request_for_diff -- --nocapture
   ```
   Fichier : `%TEMP%\sinew-composer-request-dump.json`

## Questions binaires à trancher

| Observation MITM | Action Sinew |
|------------------|--------------|
| Surtout `agent.v1/Run` ou `RunSSE` sur api2 | **Priorité** : spike `agent/` (voir AGENT-SPIKE-DESIGN.md) |
| IdempotentSSE + `x-idempotent-encryption-key` capturé | Implémenter format dans `encryption.rs` |
| IdempotentSSE absent | Stopper investigation idempotent ; agent.v1 only |
| Seulement agent.api5 (host différent) | Tester même path sur api2 d’abord (communauté) |

## Revérification après implémentation

```powershell
python C:\Dev\Sinew\scripts\verify_all.py
```

Succès = ligne `sinew both url+hex` avec du texte streamé (pas ReadTimeout).

