# Capture MITM (Cursor Composer)

Scripts pour lancer **mitmweb** et eviter `ERR_CONNECTION_REFUSED` sur http://127.0.0.1:8081 (UI arretee).

## Installation (une fois)

```powershell
cd C:\Dev\Sinew
pwsh -File scripts\mitm\install-mitmproxy.ps1
```

Manuel si besoin :

- `winget install mitmproxy.mitmproxy -e`
- ou `python -m pip install --upgrade mitmproxy`

## Demarrage

1. **Verifier** : `pwsh -File scripts\mitm\check-mitm.ps1`
2. **Lancer mitmweb** (fenetre dediee, laisser ouverte) : `pwsh -File scripts\mitm\start-mitmweb.ps1`
3. Ouvrir l'UI : http://127.0.0.1:8081/

## Certificat HTTPS (obligatoire)

1. Dans mitmweb : **Options → Install mitmproxy CA** (Windows).
2. Ou importer `%USERPROFILE%\.mitmproxy\mitmproxy-ca-cert.cer` dans **Autorites de certification racines de confiance** (certmgr / certlm).
3. Redemarrer Cursor apres installation du certificat.

## Proxy systeme Windows

1. Parametres → Reseau et Internet → Proxy → **Utiliser un serveur proxy** : ON
2. Adresse : `127.0.0.1`, Port : `8080`
3. **Apres la capture** : desactiver le proxy (sinon plus de reseau sans mitmweb).

## Capturer Composer

1. mitmweb actif + certificat + proxy systeme ON
2. Cursor IDE : envoyer un message **Composer / Agent**
3. Dans mitmweb, filtrer `api2.cursor.sh`, chemins `agent.v1`, `RunSSE`, `Idempotent`, etc.

Voir aussi `scripts\CAPTURE-MITM.md` pour l'analyse des requetes.

## Depannage

| Symptome | Cause probable | Action |
|----------|----------------|--------|
| `ERR_CONNECTION_REFUSED` sur :8081 | mitmweb non lance | `start-mitmweb.ps1` |
| Cursor sans reseau | Proxy ON sans mitmweb | Desactiver proxy ou relancer mitmweb |
| HTTPS echoue dans Cursor | Certificat CA non installe | Installer CA mitmproxy |
| Port 8080/8081 deja pris | Autre processus | `check-mitm.ps1`, tuer l'ancien mitmweb |
| `mitmweb` introuvable | Non installe | `install-mitmproxy.ps1` |

