# Preuve mesuree : distillation locale vs lecture brute
# Compare les jetons qu'une IA cloud consommerait AVEC vs SANS le distillateur local.
$ErrorActionPreference = "Stop"
$target = "C:\dev\sinew\src\components\SettingsPane.tsx"
$question = "Where is the semantic embeddings toggle defined, what is its localStorage key, and which IPC function does it call?"

$raw = Get-Content -Raw -Path $target
$rawChars = $raw.Length
$rawTokens = [math]::Round($rawChars / 4)

# Charge le 3B en memoire et le garde resident (keep_alive: -1)
$prompt = @"
You are a code-context distiller for an AI coding agent. A teammate AI needs to answer this question:
"$question"
Read the file below and output ONLY the minimal facts needed to answer (function names, line hints, keys, IPC calls). Be terse. Max 150 words. No code blocks.

FILE (SettingsPane.tsx):
$raw
"@

$body = @{
  model      = "qwen2.5:3b"
  prompt     = $prompt
  stream     = $false
  keep_alive = -1
  options    = @{ num_ctx = 16384; temperature = 0.1 }
} | ConvertTo-Json -Depth 6

$sw = [System.Diagnostics.Stopwatch]::StartNew()
$resp = Invoke-RestMethod -Uri "http://127.0.0.1:11434/api/generate" -Method Post -Body $body -ContentType "application/json" -TimeoutSec 600
$sw.Stop()

$distilled = $resp.response.Trim()
$distChars = $distilled.Length
$distTokens = [math]::Round($distChars / 4)
$saved = [math]::Round((1 - ($distTokens / $rawTokens)) * 100, 1)

Write-Host "=================== PREUVE BOOST LOCAL ==================="
Write-Host ("Fichier            : SettingsPane.tsx")
Write-Host ("Question posee     : " + $question)
Write-Host "----------------------------------------------------------"
Write-Host ("SANS boost (lecture brute du fichier) : {0} jetons" -f $rawTokens)
Write-Host ("AVEC boost (resume du 3B local)        : {0} jetons" -f $distTokens)
Write-Host ("ECONOMIE de jetons cloud               : {0} %" -f $saved)
Write-Host ("Temps de distillation locale           : {0} s" -f [math]::Round($sw.Elapsed.TotalSeconds,1))
Write-Host "----------------------------------------------------------"
Write-Host "Reponse distillee fournie a l'IA cloud :"
Write-Host $distilled
Write-Host "=========================================================="
