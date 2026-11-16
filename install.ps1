param(
    [switch]$AutoStart,
    [switch]$Shortcut
)

$ErrorActionPreference = "Stop"
$ExeName    = "floating-todo.exe"
$ScriptDir  = $PSScriptRoot
if (-not $ScriptDir) { $ScriptDir = (Get-Location).Path }
$SourceExe  = Join-Path $ScriptDir $ExeName
$InstallDir = Join-Path $env:LOCALAPPDATA "FloatingTodo"
$DestExe    = Join-Path $InstallDir $ExeName

if (-not (Test-Path $SourceExe)) {
    Write-Error "$ExeName not found. Run build.ps1 first."
    exit 1
}

# Create install directory
New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null

# Copy binary
Copy-Item $SourceExe $DestExe -Force
Write-Host "Installed to $DestExe" -ForegroundColor Green

# Optional: Auto-start on login
if ($AutoStart) {
    $regPath = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
    Set-ItemProperty -Path $regPath -Name "FloatingTodo" -Value "`"$DestExe`""
    Write-Host "Auto-start enabled (registry)" -ForegroundColor Green
}

# Optional: Start Menu shortcut
if ($Shortcut) {
    $shortcutDir  = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs"
    $shortcutPath = Join-Path $shortcutDir "Floating TODO.lnk"
    $shell = New-Object -ComObject WScript.Shell
    $sc = $shell.CreateShortcut($shortcutPath)
    $sc.TargetPath = $DestExe
    $sc.WorkingDirectory = $InstallDir
    $sc.Description = "Floating TODO"
    $sc.Save()
    Write-Host "Start Menu shortcut created" -ForegroundColor Green
}

Write-Host "Launch: & `"$DestExe`"" -ForegroundColor Cyan
