# Preuve v2 : la BONNE architecture (recherche cible -> petit contexte -> reponse juste)
$ErrorActionPreference = "Stop"
$target = "C:\dev\sinew\src\components\SettingsPane.tsx"
$question = "Where is the semantic embeddings toggle defined, what is its localStorage key, and which IPC function does it call?"

$lines = Get-Content -Path $target
$rawTokens = [math]::Round(($lines -join "`n").Length / 4)

# Etape 1 : RECHERCHE CIBLEE (ce que fait l'index/grep) -> ne garde que les lignes pertinentes + contexte
$pattern = "semantic|embeddings|setSemanticEmbeddings|sinew\.semantic"
$hits = @()
for ($i = 0; $i -lt $lines.Count; $i++) {
  if ($lines[$i] -match $pattern) {
    $start = [math]::Max(0, $i - 2); $end = [math]::Min($lines.Count - 1, $i + 2)
    $hits += ($start..$end)
  }
}
$hits = $hits | Sort-Object -Unique
$snippet = ($hits | ForEach-Object { "{0}: {1}" -f ($_ + 1), $lines[$_] }) -join "`n"
$snipTokens = [math]::Round($snippet.Length / 4)

# Etape 2 : le distillateur local resume CE bout borne (et non tout le fichier)
$prompt = @"
Answer this question using ONLY the snippet. Be terse, max 60 words, give exact key and IPC name.
Question: $question

SNIPPET:
$snippet
"@
$body = @{ model="qwen2.5:3b"; prompt=$prompt; stream=$false; keep_alive=-1; options=@{ num_ctx=8192; temperature=0 } } | ConvertTo-Json -Depth 6
$sw = [System.Diagnostics.Stopwatch]::StartNew()
$resp = Invoke-RestMethod -Uri "http://127.0.0.1:11434/api/generate" -Method Post -Body $body -ContentType "application/json" -TimeoutSec 300
$sw.Stop()
$ans = $resp.response.Trim()

Write-Host "=================== PREUVE BOOST LOCAL v2 ==================="
Write-Host ("Question : " + $question)
Write-Host "------------------------------------------------------------"
Write-Host ("SANS boost  : lire tout SettingsPane.tsx          = {0} jetons" -f $rawTokens)
Write-Host ("AVEC boost  : recherche ciblee -> {0} lignes        = {1} jetons" -f $hits.Count, $snipTokens)
Write-Host ("ECONOMIE    : {0} %" -f [math]::Round((1-($snipTokens/$rawTokens))*100,1))
Write-Host ("Distillation locale (3B) en {0}s, reponse juste :" -f [math]::Round($sw.Elapsed.TotalSeconds,1))
Write-Host "------------------------------------------------------------"
Write-Host $ans
Write-Host "============================================================"
