#Requires -Version 5.1
<# Installe mitmproxy / mitmweb si absent (Windows). #>
$ErrorActionPreference = 'Stop'

function Test-MitmWeb {
    if (Get-Command mitmweb -ErrorAction SilentlyContinue) { return $true }
    if (Get-Command python -ErrorAction SilentlyContinue) {
        & python -m mitmweb --version 2>$null | Out-Null
        if ($LASTEXITCODE -eq 0) { return $true }
    }
    return $false
}

if (Test-MitmWeb) {
    Write-Host 'mitmweb deja disponible.' -ForegroundColor Green
    if (Get-Command mitmweb -ErrorAction SilentlyContinue) { mitmweb --version } else { python -m mitmweb --version }
    exit 0
}

Write-Host 'mitmweb introuvable. Installation...' -ForegroundColor Yellow
if (Get-Command winget -ErrorAction SilentlyContinue) {
    winget install mitmproxy.mitmproxy -e --accept-package-agreements --accept-source-agreements
    if (($LASTEXITCODE -eq 0) -and (Test-MitmWeb)) { Write-Host 'Installe via winget.' -ForegroundColor Green; exit 0 }
}
if (Get-Command python -ErrorAction SilentlyContinue) {
    python -m pip install --upgrade mitmproxy
    if (($LASTEXITCODE -eq 0) -and (Test-MitmWeb)) { Write-Host 'Installe via pip.' -ForegroundColor Green; exit 0 }
}
Write-Host 'Echec. Manuel: winget install mitmproxy.mitmproxy -e  OU  python -m pip install --upgrade mitmproxy' -ForegroundColor Red
exit 1
