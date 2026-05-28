# Capture MITM — débloquer `x-idempotent-encryption-key`

Objectif : copier **exactement** ce que Cursor IDE envoie lors d’un message Composer 2.5.

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

3. Lancer `mitmweb` et ouvrir l’UI http://127.0.0.1:8081

4. Ouvrir **Cursor IDE**, envoyer un message **Composer 2.5** (mode Agent)

5. Dans mitmweb, filtrer :
   - Host : `api2.cursor.sh`, `agent.api5.cursor.sh`
   - Path contenant : `Idempotent`, `StreamUnified`, `agent.v1`

6. Pour chaque requête POST pertinente, noter :
   - URL complète
   - Tous les headers `x-*` (surtout `x-idempotent-encryption-key`, `x-blob-encryption-key`, `x-idempotency-key`)
   - Longueur et préfixe du header idempotent (ne pas coller de secrets dans un repo public)
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
| Header `x-idempotent-encryption-key` présent sur api2 IdempotentSSE | Implémenter le même format dans `encryption.rs` |
| Aucun IdempotentSSE ; seulement `agent.api5` + `agent.v1` | Planifier migration agent.api5 (NAL) |
| IdempotentSSE sans ce header (blob seulement) | Vérifier si le token IDE ≠ OAuth Sinew |

## Revérification après implémentation

```powershell
python C:\Dev\Sinew\scripts\verify_all.py
```

Succès = ligne `sinew both url+hex` avec du texte streamé (pas ReadTimeout).
