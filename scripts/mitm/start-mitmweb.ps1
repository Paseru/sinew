#Requires -Version 5.1
<# Demarre mitmweb (proxy 8080, UI 8081). Garde la fenetre ouverte en cas d'erreur. #>
$ErrorActionPreference = 'Stop'
$ProxyPort = 8080
$WebPort = 8081

function Test-PortInUse([int]$Port) {
    $c = Get-NetTCPConnection -State Listen -LocalPort $Port -ErrorAction SilentlyContinue
    return [bool]$c
}

function Resolve-MitmWeb {
    if (Get-Command mitmweb -ErrorAction SilentlyContinue) {
        return @{ File = (Get-Command mitmweb).Source; Args = @() }
    }
    if (Get-Command python -ErrorAction SilentlyContinue) {
        & python -m mitmweb --version 2>$null | Out-Null
        if ($LASTEXITCODE -eq 0) {
            return @{ File = (Get-Command python).Source; Args = @('-m', 'mitmweb') }
        }
    }
    return $null
}

function Wait-Exit {
    Write-Host "`nAppuyez sur Entree pour fermer..." -ForegroundColor DarkGray
    Read-Host | Out-Null
}

Write-Host "=== Demarrage mitmweb ===" -ForegroundColor Cyan
Write-Host "Proxy systeme Windows : 127.0.0.1:$ProxyPort"
Write-Host "Interface web         : http://127.0.0.1:$WebPort/"
Write-Host "Certificat CA         : %USERPROFILE%\.mitmproxy\mitmproxy-ca-cert.cer"
Write-Host "(Installer le certificat dans Autorites de certification racines de confiance)"

$bin = Resolve-MitmWeb
if (-not $bin) {
    Write-Host "mitmweb introuvable. Executez: pwsh -File scripts\mitm\install-mitmproxy.ps1" -ForegroundColor Red
    Wait-Exit
    exit 1
}

foreach ($port in @($ProxyPort, $WebPort)) {
    if (Test-PortInUse $port) {
        Write-Host "Port $port deja utilise (mitmweb deja lance ?)." -ForegroundColor Yellow
        Get-NetTCPConnection -State Listen -LocalPort $port -ErrorAction SilentlyContinue | Format-Table LocalAddress, LocalPort, OwningProcess
    }
}

$mitmArgs = @('--listen-port', "$ProxyPort", '--web-port', "$WebPort") + $bin.Args
Write-Host "Commande: $($bin.File) $($mitmArgs -join ' ')"

try {
    & $bin.File @mitmArgs
    $code = $LASTEXITCODE
    if ($code -and $code -ne 0) {
        Write-Host "mitmweb a quitte avec le code $code" -ForegroundColor Red
        Wait-Exit
        exit $code
    }
} catch {
    Write-Host "Erreur: $($_.Exception.Message)" -ForegroundColor Red
    Wait-Exit
    exit 1
}
