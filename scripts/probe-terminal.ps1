# Runs INSIDE a terminal under test (Windows): times one query and records it
# in tsi-terminals\<name>.txt at the repository root, for scripts\try-terminals.ps1.
param([Parameter(Mandatory)][string]$Name)
$root = Split-Path $PSScriptRoot -Parent
$bin = Join-Path $root 'target\release\terminal-scheme-info.exe'
$dir = Join-Path $root 'tsi-terminals'
New-Item -ItemType Directory -Force -Path $dir | Out-Null
$sw = [Diagnostics.Stopwatch]::StartNew()
$out = & $bin query powershell 2>&1
$code = $LASTEXITCODE
$sw.Stop()
@(
    "terminal=$Name"
    "TERM=$env:TERM WT_SESSION=$env:WT_SESSION"
    "exit=$code"
    "ms=$($sw.ElapsedMilliseconds)"
    (@($out) -join "`n")
) | Set-Content -Path (Join-Path $dir "$Name.txt")
