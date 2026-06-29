[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)]
    [string]$Url,
    [Parameter(Mandatory=$false)]
    [string]$ProjectPath = ".",
    [Parameter(Mandatory=$false)]
    [int]$TimeoutSeconds = 300,
    [Parameter(Mandatory=$false)]
    [int]$IntervalSeconds = 10
)

$ProjectPath = [System.IO.Path]::GetFullPath($ProjectPath)
Write-Host "Assurance Loop initiated."
Write-Host "Target URL: $Url"
Write-Host "Project Path: $ProjectPath"

# 1. Resolve Target Commit SHA
$targetSha = (git -C $ProjectPath rev-parse HEAD).Trim()
Write-Host "Target Commit SHA: $targetSha"

# 2. Setup paths and directories
$frontendDir = Join-Path $ProjectPath "capadvisors-frontend"
$scriptsDir = Join-Path $frontendDir "scripts"
if (-not (Test-Path $scriptsDir)) {
    New-Item -ItemType Directory -Path $scriptsDir -Force | Out-Null
}

# 3. Write write-build-info.mjs
$nodeScriptContent = @"
import fs from 'fs';
import path from 'path';
import { execSync } from 'child_process';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

function getGitCommit() {
  try {
    return execSync('git rev-parse HEAD').toString().trim();
  } catch (e) {
    return 'unknown';
  }
}

function getGitBranch() {
  try {
    return execSync('git rev-parse --abbrev-ref HEAD').toString().trim();
  } catch (e) {
    return 'unknown';
  }
}

const buildInfo = {
  commitSha: process.env.VERCEL_GIT_COMMIT_SHA || getGitCommit(),
  commitRef: process.env.VERCEL_GIT_COMMIT_REF || getGitBranch(),
  env: process.env.VERCEL_ENV || 'local',
  deploymentId: process.env.VERCEL_DEPLOYMENT_ID || 'local-dev',
  timestamp: new Date().toISOString()
};

const outputDir = path.join(__dirname, '../public');
if (!fs.existsSync(outputDir)) {
  fs.mkdirSync(outputDir, { recursive: true });
}

fs.writeFileSync(
  path.join(outputDir, 'build-info.json'),
  JSON.stringify(buildInfo, null, 2)
);

console.log('Build info written successfully:', buildInfo);
"@

$nodeScriptPath = Join-Path $scriptsDir "write-build-info.mjs"
Set-Content -Path $nodeScriptPath -Value $nodeScriptContent -Encoding utf8
Write-Host "Generated Node compilation generator at $nodeScriptPath"

# 4. Inject package.json prebuild hook
$packageJsonPath = Join-Path $frontendDir "package.json"
if (Test-Path $packageJsonPath) {
    $pjContent = Get-Content $packageJsonPath -Raw
    # Simple regex parsing to safely inject prebuild hook without destroying formatting
    if ($pjContent -like "*`"prebuild`"*") {
        Write-Host "prebuild hook already exists in package.json"
    } else {
        # Inject "prebuild": "node scripts/write-build-info.mjs", right after "scripts": {
        $pjContent = $pjContent -replace '"scripts"\s*:\s*\{', '"scripts": {`n    "prebuild": "node scripts/write-build-info.mjs",'
        Set-Content -Path $packageJsonPath -Value $pjContent -Encoding utf8
        Write-Host "Injected prebuild hook into package.json"
    }
}

# 5. Add to .gitignore
$gitignorePath = Join-Path $frontendDir ".gitignore"
if (Test-Path $gitignorePath) {
    $giContent = Get-Content $gitignorePath -Raw
    if ($giContent -notmatch "public/build-info\.json") {
        $giContent = $giContent.TrimEnd() + "`npublic/build-info.json`n"
        Set-Content -Path $gitignorePath -Value $giContent -Encoding utf8
        Write-Host "Added public/build-info.json to .gitignore"
    }
} else {
    Set-Content -Path $gitignorePath -Value "public/build-info.json`n" -Encoding utf8
    Write-Host "Created .gitignore and added public/build-info.json"
}

# 6. Polling loop
$startTime = [DateTime]::UtcNow
$endTime = $startTime.AddSeconds($TimeoutSeconds)
$success = $false

Write-Host "Starting deployment monitoring loop..."
while ([DateTime]::UtcNow -lt $endTime) {
    $timestamp = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
    $checkUrl = "$Url/build-info.json?check=$timestamp"
    
    try {
        $headers = @{
            "Cache-Control" = "no-cache"
            "Pragma"        = "no-cache"
        }
        $response = Invoke-WebRequest -Uri $checkUrl -Headers $headers -UseBasicParsing -TimeoutSec 10
        
        # Verify Content-Type is application/json
        $contentType = $response.Headers["Content-Type"]
        if (-not $contentType -or $contentType -notmatch "application/json") {
            Write-Warning "Assurance failed: Endpoint returned non-JSON Content-Type ($contentType). Might be SPA fallback HTML redirection."
        } else {
            $data = $response.Content | ConvertFrom-Json
            $remoteSha = $data.commitSha
            
            Write-Host "Checking: remote SHA=$remoteSha | local SHA=$targetSha"
            if ($remoteSha -eq $targetSha) {
                Write-Host "DEPLOYMENT VERIFIED SUCCESSFUL! Remote matches local target commit."
                $success = $true
                break
            }
        }
    } catch {
        Write-Host "Polling error: $_.Exception.Message"
    }
    
    Start-Sleep -Seconds $IntervalSeconds
}

if (-not $success) {
    Write-Error "Deployment verification TIMEOUT ($TimeoutSeconds seconds elapsed). Running Vercel CLI diagnostics..."
    
    Write-Host "`n--- Running 'vercel list' ---"
    try {
        vercel list --prod --cwd $frontendDir
    } catch {
        Write-Warning "Failed to run vercel list: $_"
    }
    
    Write-Host "`n--- Running 'vercel logs' ---"
    try {
        vercel logs --environment production --level error --since 1h --limit 50 --expand --cwd $frontendDir
    } catch {
        Write-Warning "Failed to run vercel logs: $_"
    }
    
    exit 1
} else {
    exit 0
}
