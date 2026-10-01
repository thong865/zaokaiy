<#
.SYNOPSIS
  Build zaokaiy on this PC and ship ONLY the compiled images + runtime files to the server.
  The server never gets source code, git, or a registry login.

  What goes to the server:
    docker-compose.prod.yml, .env.prod.example, deploy/Caddyfile, deploy/deploy.sh, deploy/backup.sh
    zaokaiy-api:<tag>  (Rust binary + ffmpeg on debian-slim)
    zaokaiy-web:<tag>  (Nuxt .output on node-alpine)

.EXAMPLE
  .\deploy\ship.ps1 -Server root@203.0.113.10
.EXAMPLE
  $env:ZK_SERVER = 'root@203.0.113.10'; .\deploy\ship.ps1
.EXAMPLE
  .\deploy\ship.ps1 -Server root@vps -FilesOnly      # first-time setup: upload config files only
.EXAMPLE
  .\deploy\ship.ps1 -Server root@vps -Rollback       # start the previous release again
#>
param(
  [string]$Server = $env:ZK_SERVER,
  [string]$Path = $(if ($env:ZK_PATH) { $env:ZK_PATH } else { '/var/serv/zaokaiy' }),
  [int]$Port = $(if ($env:ZK_PORT) { [int]$env:ZK_PORT } else { 22 }),
  [string]$Key = $env:ZK_SSH_KEY,
  [string]$Tag,
  [switch]$FilesOnly,
  [switch]$SkipBuild,
  [switch]$Rollback
)
$ErrorActionPreference = 'Stop'
if (-not $Server) { throw 'Pass -Server user@host (or set $env:ZK_SERVER)' }
Set-Location (Split-Path -Parent $PSScriptRoot)

function Run([string]$exe, [string[]]$argv) {
  & $exe @argv
  if ($LASTEXITCODE -ne 0) { throw "$exe failed (exit $LASTEXITCODE)" }
}
$sshOpt = @('-p', "$Port"); $scpOpt = @('-P', "$Port")
if ($Key) { $sshOpt += @('-i', $Key); $scpOpt += @('-i', $Key) }
function Remote([string]$cmd) { Run ssh ($sshOpt + @($Server, $cmd)) }

if ($Rollback) { Remote "cd '$Path' && bash deploy/deploy.sh rollback"; return }

# 1. build the images locally (source stays on this PC)
if (-not $FilesOnly) {
  if (-not $Tag) {
    $Tag = (git rev-parse --short=12 HEAD).Trim()
    if (git status --porcelain -- backend frontend) {
      Write-Warning 'backend/ or frontend/ has uncommitted changes - tagging as dirty'
      $Tag += '-dirty-' + (Get-Date -Format 'yyyyMMddHHmmss')
    }
  }
  $api = "zaokaiy-api:$Tag"; $web = "zaokaiy-web:$Tag"
  if (-not $SkipBuild) {
    Write-Host "== building $api and $web" -ForegroundColor Cyan
    Run docker @('build', '--platform', 'linux/amd64', '-t', $api, 'backend')
    Run docker @('build', '--platform', 'linux/amd64', '-t', $web, 'frontend')
  }
}

# 2. runtime files (no source)
Write-Host "== uploading runtime files to ${Server}:$Path" -ForegroundColor Cyan
Remote "mkdir -p '$Path/deploy'"
Run scp ($scpOpt + @('docker-compose.prod.yml', '.env.prod.example', "${Server}:$Path/"))
Run scp ($scpOpt + @('deploy/Caddyfile', 'deploy/deploy.sh', 'deploy/backup.sh', "${Server}:$Path/deploy/"))
# Windows checkouts may have CRLF line endings; bash needs LF
Remote "cd '$Path' && sed -i 's/\r`$//' deploy/*.sh deploy/Caddyfile && chmod +x deploy/*.sh"
if ($FilesOnly) {
  Write-Host "done. On the server: cp $Path/.env.prod.example $Path/.env.prod, fill it in, chmod 600 it." -ForegroundColor Green
  return
}

# 3. images -> server (docker save / scp / docker load)
$tar = Join-Path $env:TEMP "zaokaiy-$Tag.tar"
Write-Host "== exporting images" -ForegroundColor Cyan
Run docker @('save', '-o', $tar, $api, $web)
try {
  Write-Host ("== uploading {0:N0} MB" -f ((Get-Item $tar).Length / 1MB)) -ForegroundColor Cyan
  Run scp ($scpOpt + @('-C', $tar, "${Server}:/tmp/zaokaiy-images.tar"))
} finally {
  Remove-Item $tar -ErrorAction SilentlyContinue
}
Remote "docker load -i /tmp/zaokaiy-images.tar; rc=`$?; rm -f /tmp/zaokaiy-images.tar; exit `$rc"

# 4. start it (health check + automatic rollback if unhealthy)
Write-Host "== starting $api $web" -ForegroundColor Cyan
Remote "cd '$Path' && bash deploy/deploy.sh $api $web"
Write-Host "deployed $Tag" -ForegroundColor Green
