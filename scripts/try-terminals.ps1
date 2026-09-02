# Runs one real query in every installed Windows terminal and prints a markdown
# table (terminal, version, OS, answer, time). Each terminal is started with
# pwsh running scripts\probe-terminal.ps1; windows flash open and close.
#
#   pwsh scripts\try-terminals.ps1
#   pwsh scripts\try-terminals.ps1 -WindowsTerminal C:\path\to\wt.exe
param([string]$WindowsTerminal = (Get-Command wt.exe -ErrorAction SilentlyContinue).Source)
$ErrorActionPreference = 'Continue'
$root = Split-Path $PSScriptRoot -Parent
$probe = Join-Path $root 'scripts\probe-terminal.ps1'
$results = Join-Path $root 'tsi-terminals'
Remove-Item -Recurse -Force $results -ErrorAction SilentlyContinue
$os = Get-CimInstance Win32_OperatingSystem
$osName = "$($os.Caption) $($os.Version)"

"| Terminal | Version | OS | Answers OSC 10/11 | Time |"
"| --- | --- | --- | --- | --- |"

function Probe([string]$Label, [string]$Slug, [string]$Exe, [string[]]$Prefix = @()) {
    if (-not $Exe -or -not (Test-Path $Exe)) { "| $Label | not installed | $osName | | |"; return }
    $version = (Get-Item $Exe).VersionInfo.ProductVersion
    if (-not $version) { $version = '?' }
    $args = $Prefix + @('pwsh', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $probe, $Slug)
    Start-Process -FilePath $Exe -ArgumentList $args
    $file = Join-Path $results "$Slug.txt"
    $deadline = (Get-Date).AddSeconds(20)
    while (-not (Test-Path $file) -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 500 }
    Start-Sleep -Milliseconds 500
    if (-not (Test-Path $file)) { "| $Label | $version | $osName | no result: the terminal did not run the probe | |"; return }
    $text = Get-Content $file -Raw
    $ms = [regex]::Match($text, '(?m)^ms=(\d+)').Groups[1].Value
    $bg = [regex]::Match($text, "TERMINAL_BACKGROUND = '(#[0-9A-F]+)'").Groups[1].Value
    $fg = [regex]::Match($text, "TERMINAL_FOREGROUND = '(#[0-9A-F]+)'").Groups[1].Value
    $scheme = [regex]::Match($text, "TERMINAL_COLOR_SCHEME = '([a-z]+)'").Groups[1].Value
    if ($bg) { "| $Label | $version | $osName | yes ($bg on $fg, $scheme) | $ms ms |" }
    else { "| $Label | $version | $osName | no (silent) | $ms ms |" }
}

Probe 'Windows Terminal' 'windows-terminal' $WindowsTerminal
Probe 'conhost (legacy console)' 'conhost' "$env:SystemRoot\System32\conhost.exe"
Probe 'mintty (Git for Windows)' 'mintty' 'C:\Program Files\Git\usr\bin\mintty.exe' @('-e')
Probe 'Alacritty' 'alacritty' 'C:\Program Files\Alacritty\alacritty.exe' @('-e')
Probe 'WezTerm' 'wezterm' 'C:\Program Files\WezTerm\wezterm.exe' @('start', '--')
