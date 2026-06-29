param (
    [string]$ProductionUrl = "https://www.capadvisors.in",
    [string]$Remote        = "origin",
    [string]$Branch        = "main",
    [int]   $MaxAttempts   = 30,
    [int]   $DelaySeconds  = 15
)

Write-Host "Fetching latest remote state from $Remote..." -ForegroundColor Cyan
git fetch $Remote

$TargetSha = (git rev-parse "$Remote/$Branch").Trim()
$ShortSha  = $TargetSha.Substring(0, 7)
Write-Host "Target SHA on $Remote/$Branch : $TargetSha ($ShortSha)" -ForegroundColor Yellow

$FingerprintUrl = "$($ProductionUrl.TrimEnd('/'))/build-info.json"
Write-Host "Polling: $FingerprintUrl" -ForegroundColor Cyan
Write-Host "--------------------------------------------------------"

for ($i = 1; $i -le $MaxAttempts; $i++) {
    Write-Host "[Attempt $i/$MaxAttempts]" -ForegroundColor Gray -NoNewline

    try {
        $Response = Invoke-WebRequest -Uri $FingerprintUrl -UseBasicParsing -TimeoutSec 10 -ErrorAction Stop
        if ($Response.StatusCode -eq 200) {
            $BuildInfo = $Response.Content | ConvertFrom-Json
            $LiveSha   = $BuildInfo.gitSha

            if ($LiveSha -eq $TargetSha) {
                Write-Host ""
                Write-Host "PASS: $ProductionUrl is serving $ShortSha (built $($BuildInfo.builtAtUtc))" -ForegroundColor Green
                exit 0
            } else {
                Write-Host " live SHA $($BuildInfo.shortSha) (built $($BuildInfo.builtAtUtc)) — waiting..." -ForegroundColor Yellow
            }
        }
    } catch {
        Write-Host " connection error: $($_.Exception.Message)" -ForegroundColor Red
    }

    if ($i -lt $MaxAttempts) { Start-Sleep -Seconds $DelaySeconds }
}

Write-Host ""
Write-Host "FAILURE: production did not serve $ShortSha after $($MaxAttempts * $DelaySeconds)s" -ForegroundColor Red
Write-Host "=================== VERCEL CLI DIAGNOSTICS ===================" -ForegroundColor Magenta

if (Get-Command vercel -ErrorAction SilentlyContinue) {
    Write-Host "`n--- Deployments for SHA $ShortSha ---" -ForegroundColor Cyan
    vercel deployments --meta githubCommitSha=$TargetSha --prod

    Write-Host "`n--- Recent deployments ---" -ForegroundColor Cyan
    vercel deployments

    Write-Host "`n--- Production build logs ---" -ForegroundColor Cyan
    vercel inspect --logs --prod

    Write-Host "`n--- Recent runtime errors ---" -ForegroundColor Cyan
    vercel logs --prod --limit 10
} else {
    Write-Host "vercel CLI not found — run 'npm i -g vercel' and 'vercel login' to enable diagnostics." -ForegroundColor Yellow
}

exit 1
