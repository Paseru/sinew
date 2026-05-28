#Requires -Version 5.1
<# Verifie que mitmweb ecoute sur 8080/8081 et que l'UI HTTP repond. #>
$ErrorActionPreference = 'Continue'
$ProxyPort = 8080
$WebPort = 8081

function Get-Listeners([int]$Port) {
    Get-NetTCPConnection -State Listen -LocalPort $Port -ErrorAction SilentlyContinue |
        Select-Object LocalAddress, LocalPort, OwningProcess
}

Write-Host "=== mitmproxy / mitmweb binaire ===" -ForegroundColor Cyan
if (Get-Command mitmweb -ErrorAction SilentlyContinue) {
    Write-Host "mitmweb: $( (Get-Command mitmweb).Source )"
    mitmweb --version
} elseif (Get-Command python -ErrorAction SilentlyContinue) {
    python -m mitmweb --version 2>$null
    if ($LASTEXITCODE -eq 0) { Write-Host "mitmweb via: python -m mitmweb" }
    else { Write-Host "mitmweb introuvable (pip/winget: scripts\mitm\install-mitmproxy.ps1)" -ForegroundColor Yellow }
} else {
    Write-Host "mitmweb introuvable" -ForegroundColor Yellow
}

Write-Host "`n=== Ecoute TCP $ProxyPort (proxy) / $WebPort (UI) ===" -ForegroundColor Cyan
$l8080 = Get-Listeners $ProxyPort
$l8081 = Get-Listeners $WebPort
if ($l8080) { $l8080 | Format-Table -AutoSize } else { Write-Host "Port $ProxyPort : RIEN (proxy mitm inactif)" -ForegroundColor Red }
if ($l8081) { $l8081 | Format-Table -AutoSize } else { Write-Host "Port $WebPort : RIEN (mitmweb UI inactive -> ERR_CONNECTION_REFUSED)" -ForegroundColor Red }

Write-Host "`n=== HTTP GET http://127.0.0.1:$WebPort/ ===" -ForegroundColor Cyan
try {
    $r = Invoke-WebRequest -Uri "http://127.0.0.1:$WebPort/" -UseBasicParsing -TimeoutSec 5
    Write-Host "OK HTTP $($r.StatusCode)" -ForegroundColor Green
} catch {
    Write-Host "Echec: $($_.Exception.Message)" -ForegroundColor Red
    Write-Host "Lancez: pwsh -File scripts\mitm\start-mitmweb.ps1" -ForegroundColor Yellow
}

$ok = ($null -ne $l8080) -and ($null -ne $l8081)
if (-not $ok) { exit 1 } else { exit 0 }
