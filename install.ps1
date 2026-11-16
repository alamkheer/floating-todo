<#
.SYNOPSIS
    Interactive installer, uninstaller, and Docker build manager for Floating TODO on Windows.

.DESCRIPTION
    Installs Floating TODO to %LOCALAPPDATA%\FloatingTodo.
    When run interactively without switches, it:
      - Prompts whether to build a new version using Docker, incrementing and persisting the patch version in Cargo.toml.
      - Detects existing installations and displays the installed version.
      - Prompts whether to uninstall the existing installation gracefully.
      - Gracefully removes configuration flags (registry autostart, shortcuts) without force or hidden deletes.
      - Collects optional setup arguments (AutoStart, Start Menu shortcut, Desktop shortcut, Launch).
      - Handles active processes safely and mitigates file locks thoroughly.

.PARAMETER Build
    Triggers an automated Docker build to compile a new version, incrementing and persisting the patch version in Cargo.toml.

.PARAMETER DockerBuild
    Alias for -Build.

.PARAMETER AutoStart
    Configures Floating TODO to start automatically on Windows user login (HKCU Run key).

.PARAMETER Shortcut
    Creates a Start Menu shortcut for Floating TODO.

.PARAMETER DesktopShortcut
    Creates a Desktop shortcut for Floating TODO.

.PARAMETER Launch
    Launches Floating TODO immediately upon successful installation.

.PARAMETER Uninstall
    Uninstalls the application, removing executable, registry key, and shortcuts gracefully.

.PARAMETER Interactive
    Forces interactive prompts even if switches are provided.

.PARAMETER NonInteractive
    Runs in unattended mode, suppressing interactive prompts and using provided command-line flags.

.EXAMPLE
    # Run interactively (recommended)
    .\install.ps1

.EXAMPLE
    # Build a new Docker version (bumps patch version) and install interactively
    .\install.ps1 -Build

.EXAMPLE
    # Unattended build and install with AutoStart and Start Menu shortcut
    .\install.ps1 -Build -AutoStart -Shortcut -NonInteractive

.EXAMPLE
    # Uninstall Floating TODO gracefully
    .\install.ps1 -Uninstall
#>

[CmdletBinding(SupportsShouldProcess = $false)]
param(
    [switch]$Build,
    [switch]$DockerBuild,
    [switch]$AutoStart,
    [switch]$Shortcut,
    [switch]$DesktopShortcut,
    [switch]$Launch,
    [switch]$Uninstall,
    [switch]$Interactive,
    [switch]$NonInteractive
)

$ErrorActionPreference = "Stop"

# Paths and configuration
$ExeName             = "floating-todo.exe"
$ScriptDir           = $PSScriptRoot
if (-not $ScriptDir) { $ScriptDir = (Get-Location).Path }
$SourceExe           = Join-Path $ScriptDir $ExeName
$InstallDir          = Join-Path $env:LOCALAPPDATA "FloatingTodo"
$DestExe             = Join-Path $InstallDir $ExeName
$RegPath             = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
$RegName             = "FloatingTodo"
$ShortcutDir         = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs"
$ShortcutPath        = Join-Path $ShortcutDir "Floating TODO.lnk"
$DesktopShortcutPath = Join-Path ([Environment]::GetFolderPath("Desktop")) "Floating TODO.lnk"
$UserDataDir         = Join-Path $env:APPDATA "FloatingTodo"
$CargoTomlPath       = Join-Path $ScriptDir "Cargo.toml"
$CargoLockPath       = Join-Path $ScriptDir "Cargo.lock"

# Determine interactive mode
$hasExplicitFlags = $PSBoundParameters.ContainsKey('AutoStart') -or 
                     $PSBoundParameters.ContainsKey('Shortcut') -or 
                     $PSBoundParameters.ContainsKey('DesktopShortcut') -or 
                     $PSBoundParameters.ContainsKey('Launch') -or 
                     $PSBoundParameters.ContainsKey('Uninstall') -or 
                     $PSBoundParameters.ContainsKey('Build') -or 
                     $PSBoundParameters.ContainsKey('DockerBuild')

$isInteractive = (-not $NonInteractive) -and ($Interactive -or -not $hasExplicitFlags -or $PSBoundParameters.ContainsKey('Uninstall'))

# ─────────────────────────────────────────────────────────────
# Helper Output and Prompt Functions
# ─────────────────────────────────────────────────────────────

function Write-InfoMessage {
    param([string]$Message)
    Write-Host "  [INFO] $Message" -ForegroundColor Cyan
}

function Write-SuccessMessage {
    param([string]$Message)
    Write-Host "  [SUCCESS] $Message" -ForegroundColor Green
}

function Write-WarningMessage {
    param([string]$Message)
    Write-Host "  [WARNING] $Message" -ForegroundColor Yellow
}

function Write-ErrorMessage {
    param([string]$Message)
    Write-Host "  [ERROR] $Message" -ForegroundColor Red
}

function Write-NoticeMessage {
    param([string]$Message)
    Write-Host "  [NOTICE] $Message" -ForegroundColor Magenta
}

function Read-YesNoPrompt {
    param(
        [string]$Prompt,
        [bool]$Default = $false
    )
    $hint = if ($Default) { "[Y/n]" } else { "[y/N]" }
    while ($true) {
        Write-Host "$Prompt ${hint}: " -NoNewline -ForegroundColor White
        $response = Read-Host
        if ([string]::IsNullOrWhiteSpace($response)) {
            return $Default
        }
        $trimmed = $response.Trim().ToLower()
        if ($trimmed -in @("y", "yes", "1", "true")) {
            return $true
        }
        if ($trimmed -in @("n", "no", "0", "false")) {
            return $false
        }
        Write-Host "  Please enter 'y' for Yes or 'n' for No." -ForegroundColor Yellow
    }
}

function Get-AppVersion {
    param([string]$FilePath)
    if (-not (Test-Path -LiteralPath $FilePath -PathType Leaf)) {
        return $null
    }
    try {
        $info = (Get-Item -LiteralPath $FilePath).VersionInfo
        if ($info.ProductVersion) { return $info.ProductVersion }
        if ($info.FileVersion) { return $info.FileVersion }
    } catch {
        # Fallback if version resource cannot be read directly
    }
    return "Unknown"
}

# ─────────────────────────────────────────────────────────────
# Version Management & Docker Build
# ─────────────────────────────────────────────────────────────

function Get-CargoPackageVersion {
    param([string]$TomlPath)
    if (Test-Path -LiteralPath $TomlPath -PathType Leaf) {
        $content = Get-Content -LiteralPath $TomlPath -Raw
        if ($content -match '(?m)^version\s*=\s*"([^"]+)"') {
            return $matches[1]
        }
    }
    return $null
}

function Increment-SemVerPatch {
    param([string]$VersionString)
    if ($VersionString -match '^(\d+)\.(\d+)\.(\d+)(.*)$') {
        $major = [int]$matches[1]
        $minor = [int]$matches[2]
        $patch = [int]$matches[3] + 1
        $suffix = $matches[4]
        return "$major.$minor.$patch$suffix"
    }
    throw "Invalid SemVer version string: $VersionString"
}

function Persist-VersionBump {
    param(
        [string]$TomlPath,
        [string]$LockPath,
        [string]$NewVersion
    )

    Write-InfoMessage "Persisting new version v$NewVersion to Cargo.toml..."
    $toml = Get-Content -LiteralPath $TomlPath -Raw
    $patternToml = '(?m)(^version\s*=\s*")[^"]+(")'
    $replacementToml = "`${1}$NewVersion`${2}"
    $newToml = [regex]::Replace($toml, $patternToml, $replacementToml, 1)
    [System.IO.File]::WriteAllText($TomlPath, $newToml, [System.Text.Encoding]::UTF8)

    if (Test-Path -LiteralPath $LockPath -PathType Leaf) {
        Write-InfoMessage "Persisting new version v$NewVersion to Cargo.lock..."
        $lock = Get-Content -LiteralPath $LockPath -Raw
        $patternLock = '(?s)(\[\[package\]\]\r?\nname = "floating-todo"\r?\nversion = ")[^"]+(")'
        $replacementLock = "`${1}$NewVersion`${2}"
        $newLock = [regex]::Replace($lock, $patternLock, $replacementLock, 1)
        [System.IO.File]::WriteAllText($LockPath, $newLock, [System.Text.Encoding]::UTF8)
    }

    Write-SuccessMessage "Version successfully updated and persisted to v$NewVersion."
}

function Invoke-DockerBuildProcess {
    param(
        [string]$ProjectDir,
        [string]$TargetVersion
    )

    Write-Host ""
    Write-Host "============================================================" -ForegroundColor Cyan
    Write-Host "         DOCKER BUILD: FLOATING TODO v$TargetVersion        " -ForegroundColor Cyan
    Write-Host "============================================================" -ForegroundColor Cyan

    # Verify Docker CLI is installed
    $dockerCmd = Get-Command docker -ErrorAction SilentlyContinue
    if (-not $dockerCmd) {
        Write-ErrorMessage "Docker CLI is not found in PATH."
        Write-NoticeMessage "Mitigation: Install Docker Desktop and add docker to PATH."
        return $false
    }

    # Verify Docker daemon is running
    Write-InfoMessage "Verifying Docker engine status..."
    try {
        $daemonVersion = & docker info --format '{{.ServerVersion}}' 2>$null
        if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($daemonVersion)) {
            Write-ErrorMessage "Docker engine is not currently running."
            Write-NoticeMessage "Mitigation: Start Docker Desktop and wait for the daemon to start."
            return $false
        }
        Write-SuccessMessage "Docker engine is running (v$daemonVersion)."
    } catch {
        Write-ErrorMessage "Failed to communicate with Docker engine: $($_.Exception.Message)"
        return $false
    }

    # Ensure running instances in project dir are gracefully stopped before file extraction
    $null = Stop-FloatingTodoGracefully -Interactive $true

    # Build Docker image
    Write-InfoMessage "Building Docker image 'floating-todo-builder'..."
    & docker build -t floating-todo-builder "$ProjectDir"
    if ($LASTEXITCODE -ne 0) {
        Write-ErrorMessage "Docker build failed during compilation."
        Write-NoticeMessage "Mitigation: Check the compiler output above for syntax or dependency errors."
        return $false
    }
    Write-SuccessMessage "Docker image compiled successfully."

    # Remove any existing extraction container
    Write-InfoMessage "Extracting binary from Docker container..."
    $prev = & docker ps -a -q -f name=^ftodo-extract$
    if ($prev) {
        & docker rm -f ftodo-extract | Out-Null
    }

    & docker create --name ftodo-extract floating-todo-builder | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Write-ErrorMessage "Failed to create extraction container 'ftodo-extract'."
        return $false
    }

    $destBinary = Join-Path $ProjectDir "floating-todo.exe"
    & docker cp "ftodo-extract:/app/target/x86_64-pc-windows-msvc/release/floating-todo.exe" "$destBinary"
    $cpSuccess = ($LASTEXITCODE -eq 0)

    & docker rm -f ftodo-extract | Out-Null

    if (-not $cpSuccess -or -not (Test-Path -LiteralPath $destBinary)) {
        Write-ErrorMessage "Failed to extract floating-todo.exe to $destBinary."
        return $false
    }

    $sizeBytes = (Get-Item -LiteralPath $destBinary).Length
    $sizeKB = [math]::Round($sizeBytes / 1024)
    Write-SuccessMessage "Extraction complete: floating-todo.exe ($sizeKB KB) ready at $destBinary"
    return $true
}

# ─────────────────────────────────────────────────────────────
# Process Lifecycle Management (Graceful Termination)
# ─────────────────────────────────────────────────────────────

function Stop-FloatingTodoGracefully {
    param([bool]$Interactive = $true)

    $procs = Get-Process -Name "floating-todo" -ErrorAction SilentlyContinue
    if (-not $procs) {
        return $true
    }

    foreach ($p in $procs) {
        Write-WarningMessage "Active Floating TODO process detected (PID: $($p.Id))."
        Write-InfoMessage "Attempting graceful window close for PID $($p.Id)..."

        try {
            $p.CloseMainWindow() | Out-Null
        } catch {
            Write-WarningMessage "Could not send close message to window: $($_.Exception.Message)"
        }

        # Wait up to 3 seconds for graceful exit
        $waitedMs = 0
        while (-not $p.HasExited -and $waitedMs -lt 3000) {
            Start-Sleep -Milliseconds 300
            $waitedMs += 300
            $p.Refresh()
        }

        if (-not $p.HasExited) {
            Write-WarningMessage "Process PID $($p.Id) did not terminate within timeout."
            $terminate = $false
            if ($Interactive) {
                $terminate = Read-YesNoPrompt "Would you like to terminate process $($p.Id) to proceed?" -Default $false
            } else {
                $terminate = $true
            }

            if ($terminate) {
                Write-InfoMessage "Terminating process PID $($p.Id)..."
                try {
                    Stop-Process -Id $p.Id -ErrorAction Stop
                    Start-Sleep -Milliseconds 300
                    Write-SuccessMessage "Process PID $($p.Id) terminated."
                } catch {
                    Write-ErrorMessage "Failed to terminate process PID $($p.Id): $($_.Exception.Message)"
                    Write-NoticeMessage "Mitigation: Open Task Manager and end 'floating-todo.exe' manually."
                    return $false
                }
            } else {
                Write-ErrorMessage "Process PID $($p.Id) remains running. Cannot proceed while files are locked."
                Write-NoticeMessage "Mitigation: Please save any unsaved work and close Floating TODO, then re-run this script."
                return $false
            }
        } else {
            Write-SuccessMessage "Process PID $($p.Id) closed gracefully."
        }
    }

    return $true
}

# ─────────────────────────────────────────────────────────────
# Uninstallation Implementation (No Force / Hidden Deletes)
# ─────────────────────────────────────────────────────────────

function Invoke-AppUninstall {
    param(
        [bool]$Interactive = $true
    )

    Write-Host ""
    Write-Host "============================================================" -ForegroundColor Cyan
    Write-Host "             FLOATING TODO - UNINSTALLATION                 " -ForegroundColor Cyan
    Write-Host "============================================================" -ForegroundColor Cyan

    # Step 1: Ensure process is stopped cleanly
    Write-Host "Step 1: Checking for running process..." -ForegroundColor White
    $stopped = Stop-FloatingTodoGracefully -Interactive $Interactive
    if (-not $stopped) {
        Write-ErrorMessage "Uninstallation aborted due to active process."
        return $false
    }
    Write-SuccessMessage "No active processes running."

    # Step 2: Remove Auto-start registry key gracefully
    Write-Host "`nStep 2: Checking Auto-start registry entry..." -ForegroundColor White
    $regProp = Get-ItemProperty -Path $RegPath -Name $RegName -ErrorAction SilentlyContinue
    if ($regProp) {
        Write-InfoMessage "Removing registry entry '$RegPath\$RegName'..."
        try {
            Remove-ItemProperty -Path $RegPath -Name $RegName -ErrorAction Stop
            Write-SuccessMessage "Auto-start registry key removed."
        } catch {
            Write-WarningMessage "Unable to remove registry property: $($_.Exception.Message)"
            Write-NoticeMessage "Mitigation: Open regedit and verify permissions on '$RegPath'."
        }
    } else {
        Write-InfoMessage "No Auto-start registry entry found."
    }

    # Step 3: Remove Start Menu shortcut
    Write-Host "`nStep 3: Checking Start Menu and Desktop shortcuts..." -ForegroundColor White
    if (Test-Path -LiteralPath $ShortcutPath) {
        Write-InfoMessage "Removing Start Menu shortcut: $ShortcutPath"
        try {
            Remove-Item -LiteralPath $ShortcutPath -ErrorAction Stop
            Write-SuccessMessage "Start Menu shortcut removed."
        } catch {
            Write-WarningMessage "Unable to remove Start Menu shortcut: $($_.Exception.Message)"
            Write-NoticeMessage "Mitigation: Delete '$ShortcutPath' manually if access is restricted."
        }
    } else {
        Write-InfoMessage "No Start Menu shortcut found."
    }

    if (Test-Path -LiteralPath $DesktopShortcutPath) {
        Write-InfoMessage "Removing Desktop shortcut: $DesktopShortcutPath"
        try {
            Remove-Item -LiteralPath $DesktopShortcutPath -ErrorAction Stop
            Write-SuccessMessage "Desktop shortcut removed."
        } catch {
            Write-WarningMessage "Unable to remove Desktop shortcut: $($_.Exception.Message)"
            Write-NoticeMessage "Mitigation: Delete '$DesktopShortcutPath' manually if access is restricted."
        }
    } else {
        Write-InfoMessage "No Desktop shortcut found."
    }

    # Step 4: Remove application executable
    Write-Host "`nStep 4: Checking installed executable..." -ForegroundColor White
    if (Test-Path -LiteralPath $DestExe) {
        Write-InfoMessage "Removing executable: $DestExe"
        try {
            Remove-Item -LiteralPath $DestExe -ErrorAction Stop
            Write-SuccessMessage "Application executable removed."
        } catch {
            Write-ErrorMessage "Unable to remove executable '$DestExe': $($_.Exception.Message)"
            Write-NoticeMessage "Mitigation: Ensure the file is not locked by another process or antivirus."
            return $false
        }
    } else {
        Write-InfoMessage "Executable not found at install path."
    }

    # Step 5: Remove install directory if empty
    Write-Host "`nStep 5: Checking installation directory..." -ForegroundColor White
    if (Test-Path -LiteralPath $InstallDir) {
        $remainingFiles = Get-ChildItem -LiteralPath $InstallDir -Force
        if ($remainingFiles.Count -eq 0) {
            Write-InfoMessage "Removing empty directory: $InstallDir"
            try {
                Remove-Item -LiteralPath $InstallDir -ErrorAction Stop
                Write-SuccessMessage "Installation directory removed."
            } catch {
                Write-WarningMessage "Could not remove directory: $($_.Exception.Message)"
            }
        } else {
            Write-WarningMessage "Installation directory contains other files: $($remainingFiles.Name -join ', ')"
            Write-NoticeMessage "Leaving '$InstallDir' intact to prevent accidental loss of non-application files."
        }
    }

    # Step 6: User tasks and configuration data (%APPDATA%\FloatingTodo)
    Write-Host "`nStep 6: User data and configuration..." -ForegroundColor White
    if (Test-Path -LiteralPath $UserDataDir) {
        Write-NoticeMessage "Saved tasks and settings are located at: $UserDataDir"
        $deleteUserData = $false

        if ($Interactive) {
            $deleteUserData = Read-YesNoPrompt "Do you want to permanently delete your saved tasks and configuration?" -Default $false
        }

        if ($deleteUserData) {
            Write-InfoMessage "Removing user data directory: $UserDataDir"
            try {
                Remove-Item -LiteralPath $UserDataDir -Recurse -ErrorAction Stop
                Write-SuccessMessage "User data and configuration deleted."
            } catch {
                Write-WarningMessage "Could not remove user data directory: $($_.Exception.Message)"
                Write-NoticeMessage "Mitigation: You can manually delete '$UserDataDir' if desired."
            }
        } else {
            Write-SuccessMessage "User tasks preserved intact at $UserDataDir."
        }
    } else {
        Write-InfoMessage "No user configuration found at $UserDataDir."
    }

    Write-Host ""
    Write-Host "============================================================" -ForegroundColor Cyan
    Write-Host "         UNINSTALLATION COMPLETED SUCCESSFULLY             " -ForegroundColor Green
    Write-Host "============================================================" -ForegroundColor Cyan
    return $true
}

# ─────────────────────────────────────────────────────────────
# Installation Implementation
# ─────────────────────────────────────────────────────────────

function Invoke-AppInstall {
    param(
        [bool]$OptAutoStart,
        [bool]$OptShortcut,
        [bool]$OptDesktopShortcut,
        [bool]$OptLaunch
    )

    Write-Host ""
    Write-Host "============================================================" -ForegroundColor Cyan
    Write-Host "              FLOATING TODO - INSTALLATION                  " -ForegroundColor Cyan
    Write-Host "============================================================" -ForegroundColor Cyan

    # Verify source binary exists
    if (-not (Test-Path -LiteralPath $SourceExe -PathType Leaf)) {
        Write-ErrorMessage "Source executable not found: $SourceExe"
        Write-NoticeMessage "Mitigation: Compile the application first by running '.\install.ps1 -Build' or '.\build.ps1'."
        exit 1
    }

    # Verify active process is stopped before copying binary
    $stopped = Stop-FloatingTodoGracefully -Interactive $true
    if (-not $stopped) {
        Write-ErrorMessage "Installation aborted because active instance could not be closed."
        exit 1
    }

    # Create destination directory
    if (-not (Test-Path -LiteralPath $InstallDir)) {
        Write-InfoMessage "Creating installation directory: $InstallDir"
        try {
            New-Item -ItemType Directory -Path $InstallDir -ErrorAction Stop | Out-Null
            Write-SuccessMessage "Directory created."
        } catch {
            Write-ErrorMessage "Failed to create directory '$InstallDir': $($_.Exception.Message)"
            Write-NoticeMessage "Mitigation: Ensure you have write permissions to %LOCALAPPDATA%."
            exit 1
        }
    }

    # Copy binary
    Write-InfoMessage "Copying $ExeName to $InstallDir..."
    try {
        Copy-Item -LiteralPath $SourceExe -Destination $DestExe -ErrorAction Stop
        Write-SuccessMessage "Installed executable to $DestExe"
    } catch {
        Write-ErrorMessage "Failed to copy executable: $($_.Exception.Message)"
        Write-NoticeMessage "Mitigation: Check if $DestExe is open or locked by another tool."
        exit 1
    }

    # Configure AutoStart if requested
    if ($OptAutoStart) {
        Write-InfoMessage "Configuring Auto-start on login in Windows registry..."
        try {
            Set-ItemProperty -Path $RegPath -Name $RegName -Value "`"$DestExe`"" -ErrorAction Stop
            Write-SuccessMessage "Auto-start enabled ($RegPath\$RegName)"
        } catch {
            Write-WarningMessage "Could not configure Auto-start registry: $($_.Exception.Message)"
            Write-NoticeMessage "Mitigation: Ensure HKCU registry is accessible."
        }
    } else {
        # If not requested, remove any obsolete autostart pointing to this app
        $existingReg = Get-ItemProperty -Path $RegPath -Name $RegName -ErrorAction SilentlyContinue
        if ($existingReg) {
            Write-InfoMessage "Removing preexisting Auto-start registry entry..."
            try {
                Remove-ItemProperty -Path $RegPath -Name $RegName -ErrorAction Stop
                Write-SuccessMessage "Auto-start disabled."
            } catch {
                Write-WarningMessage "Could not clean up preexisting Auto-start registry: $($_.Exception.Message)"
            }
        }
    }

    # Shell object for shortcut creation
    $shell = $null

    # Configure Start Menu shortcut if requested
    if ($OptShortcut) {
        Write-InfoMessage "Creating Start Menu shortcut..."
        try {
            if (-not (Test-Path -LiteralPath $ShortcutDir)) {
                New-Item -ItemType Directory -Path $ShortcutDir -ErrorAction Stop | Out-Null
            }
            if (-not $shell) { $shell = New-Object -ComObject WScript.Shell }
            $sc = $shell.CreateShortcut($ShortcutPath)
            $sc.TargetPath = $DestExe
            $sc.WorkingDirectory = $InstallDir
            $sc.Description = "Floating TODO"
            $sc.Save()
            Write-SuccessMessage "Start Menu shortcut created: $ShortcutPath"
        } catch {
            Write-WarningMessage "Failed to create Start Menu shortcut: $($_.Exception.Message)"
            Write-NoticeMessage "Mitigation: Verify write access to '$ShortcutDir'."
        }
    }

    # Configure Desktop shortcut if requested
    if ($OptDesktopShortcut) {
        Write-InfoMessage "Creating Desktop shortcut..."
        try {
            if (-not $shell) { $shell = New-Object -ComObject WScript.Shell }
            $sc = $shell.CreateShortcut($DesktopShortcutPath)
            $sc.TargetPath = $DestExe
            $sc.WorkingDirectory = $InstallDir
            $sc.Description = "Floating TODO"
            $sc.Save()
            Write-SuccessMessage "Desktop shortcut created: $DesktopShortcutPath"
        } catch {
            Write-WarningMessage "Failed to create Desktop shortcut: $($_.Exception.Message)"
            Write-NoticeMessage "Mitigation: Verify write access to Desktop."
        }
    }

    Write-Host ""
    Write-Host "============================================================" -ForegroundColor Cyan
    Write-Host "           INSTALLATION COMPLETED SUCCESSFULLY              " -ForegroundColor Green
    Write-Host "============================================================" -ForegroundColor Cyan
    Write-Host "Installed Path   : $DestExe" -ForegroundColor White
    Write-Host "Auto-Start       : $(if ($OptAutoStart) { 'Enabled' } else { 'Disabled' })" -ForegroundColor White
    Write-Host "Start Menu       : $(if ($OptShortcut) { 'Created' } else { 'Skipped' })" -ForegroundColor White
    Write-Host "Desktop Shortcut : $(if ($OptDesktopShortcut) { 'Created' } else { 'Skipped' })" -ForegroundColor White
    Write-Host ""

    # Launch application if requested
    if ($OptLaunch) {
        Write-InfoMessage "Launching Floating TODO..."
        try {
            Start-Process -FilePath $DestExe
            Write-SuccessMessage "Floating TODO launched successfully."
        } catch {
            Write-WarningMessage "Could not automatically launch Floating TODO: $($_.Exception.Message)"
            Write-Host "Launch manually using: & `"$DestExe`"" -ForegroundColor Cyan
        }
    } else {
        Write-Host "Launch command: & `"$DestExe`"" -ForegroundColor Cyan
    }
}

# ─────────────────────────────────────────────────────────────
# Main Orchestration
# ─────────────────────────────────────────────────────────────

$isInstalled = Test-Path -LiteralPath $DestExe -PathType Leaf
$installedVersion = if ($isInstalled) { Get-AppVersion -FilePath $DestExe } else { $null }
$sourceVersion = if (Test-Path -LiteralPath $SourceExe -PathType Leaf) { Get-AppVersion -FilePath $SourceExe } else { "Unknown" }

# Repository version info from Cargo.toml
$repoVersion = Get-CargoPackageVersion -TomlPath $CargoTomlPath
$nextPatchVersion = if ($repoVersion) { Increment-SemVerPatch -VersionString $repoVersion } else { $null }

# 1. Handle explicit CLI uninstall switch
if ($Uninstall) {
    $success = Invoke-AppUninstall -Interactive $isInteractive
    if ($success) { exit 0 } else { exit 1 }
}

# 2. Check if already installed
if ($isInstalled) {
    Write-Host ""
    Write-Host "============================================================" -ForegroundColor Cyan
    Write-Host "           FLOATING TODO - DETECTED INSTALLATION            " -ForegroundColor Cyan
    Write-Host "============================================================" -ForegroundColor Cyan
    Write-Host "Installed Executable : $DestExe" -ForegroundColor White
    Write-Host "Installed Version    : v$installedVersion" -ForegroundColor Green
    Write-Host "Source Binary Version: v$sourceVersion" -ForegroundColor Yellow
    if ($repoVersion) {
        Write-Host "Repository Version   : v$repoVersion (Next patch: v$nextPatchVersion)" -ForegroundColor DarkCyan
    }

    # Check active flags
    $hasAutoStartReg = (Get-ItemProperty -Path $RegPath -Name $RegName -ErrorAction SilentlyContinue) -ne $null
    $hasShortcutFile = Test-Path -LiteralPath $ShortcutPath
    $hasDesktopFile  = Test-Path -LiteralPath $DesktopShortcutPath
    Write-Host "Auto-start On Login  : $(if ($hasAutoStartReg) { 'Active' } else { 'Not configured' })" -ForegroundColor White
    Write-Host "Start Menu Shortcut  : $(if ($hasShortcutFile) { 'Present' } else { 'Not found' })" -ForegroundColor White
    Write-Host "Desktop Shortcut     : $(if ($hasDesktopFile) { 'Present' } else { 'Not found' })" -ForegroundColor White
    Write-Host "============================================================" -ForegroundColor Cyan
    Write-Host ""

    if ($isInteractive) {
        $shouldUninstall = Read-YesNoPrompt "Floating TODO v${installedVersion} is already installed. Do you want to uninstall it?" -Default $false

        if ($shouldUninstall) {
            $uninstalled = Invoke-AppUninstall -Interactive $true
            if (-not $uninstalled) {
                Write-ErrorMessage "Uninstallation encountered issues. Aborting."
                exit 1
            }

            Write-Host ""
            $continueInstall = Read-YesNoPrompt "Would you like to install a fresh copy of Floating TODO now?" -Default $true
            if (-not $continueInstall) {
                Write-SuccessMessage "Uninstallation complete. Exiting without reinstalling."
                exit 0
            }
        } else {
            # User chose not to uninstall, ask if they want to reinstall/update
            $shouldReinstall = Read-YesNoPrompt "Would you like to reinstall / update Floating TODO?" -Default $true
            if (-not $shouldReinstall) {
                Write-InfoMessage "No changes made. Exiting."
                exit 0
            }
        }
    } else {
        # Non-interactive mode with already installed app: proceed with update if switches passed
        Write-InfoMessage "Updating existing installation in non-interactive mode..."
    }
}

# 3. Docker Build Decision & Version Increment
$shouldBuild = $Build -or $DockerBuild

if (-not $shouldBuild) {
    $sourceExists = Test-Path -LiteralPath $SourceExe -PathType Leaf
    if (-not $sourceExists) {
        Write-Host ""
        Write-NoticeMessage "Source executable '$ExeName' not found in $ScriptDir."
        if ($isInteractive) {
            $shouldBuild = Read-YesNoPrompt "Would you like to build a new version using Docker now (will bump patch version to v${nextPatchVersion})?" -Default $true
            if (-not $shouldBuild) {
                Write-ErrorMessage "Cannot proceed with installation without an executable."
                Write-NoticeMessage "Mitigation: Compile using '.\build.ps1' or run '.\install.ps1 -Build'."
                exit 1
            }
        } else {
            Write-ErrorMessage "Source executable missing and non-interactive mode specified."
            exit 1
        }
    } elseif ($isInteractive -and $repoVersion) {
        Write-Host ""
        Write-Host "Build Options:" -ForegroundColor Cyan
        $shouldBuild = Read-YesNoPrompt "Build a new version with Docker before installing? (Bumps patch from v${repoVersion} to v${nextPatchVersion})" -Default $false
    }
}

if ($shouldBuild) {
    if (-not $repoVersion) {
        Write-ErrorMessage "Cargo.toml not found or invalid; cannot determine patch version to increment."
        exit 1
    }

    # Backup files in case build fails
    $origToml = Get-Content -LiteralPath $CargoTomlPath -Raw
    $origLock = if (Test-Path -LiteralPath $CargoLockPath) { Get-Content -LiteralPath $CargoLockPath -Raw } else { $null }

    # Increment and persist patch version
    Write-Host ""
    Write-Host "Version Increment:" -ForegroundColor Cyan
    Persist-VersionBump -TomlPath $CargoTomlPath -LockPath $CargoLockPath -NewVersion $nextPatchVersion

    # Run Docker build
    $buildSuccess = Invoke-DockerBuildProcess -ProjectDir $ScriptDir -TargetVersion $nextPatchVersion

    if (-not $buildSuccess) {
        Write-WarningMessage "Docker build was unsuccessful. Reverting version changes..."
        [System.IO.File]::WriteAllText($CargoTomlPath, $origToml, [System.Text.Encoding]::UTF8)
        if ($origLock) {
            [System.IO.File]::WriteAllText($CargoLockPath, $origLock, [System.Text.Encoding]::UTF8)
        }
        Write-NoticeMessage "Cargo.toml and Cargo.lock reverted back to v$repoVersion."

        if (Test-Path -LiteralPath $SourceExe -PathType Leaf) {
            if ($isInteractive) {
                $useExisting = Read-YesNoPrompt "Would you like to proceed with installing the preexisting binary?" -Default $false
                if (-not $useExisting) {
                    Write-ErrorMessage "Installation aborted after build failure."
                    exit 1
                }
            } else {
                Write-ErrorMessage "Installation aborted after build failure."
                exit 1
            }
        } else {
            Write-ErrorMessage "No preexisting binary available to install."
            exit 1
        }
    } else {
        # Update source version reference
        $sourceVersion = $nextPatchVersion
    }
}

# 4. Collect optional arguments interactively if needed
$finalAutoStart       = $AutoStart
$finalShortcut        = $Shortcut
$finalDesktopShortcut = $DesktopShortcut
$finalLaunch          = $Launch

if ($isInteractive) {
    Write-Host ""
    Write-Host "Please select installation options:" -ForegroundColor Cyan

    # Auto-start on login
    if (-not $PSBoundParameters.ContainsKey('AutoStart')) {
        $defaultAutoStart = (Get-ItemProperty -Path $RegPath -Name $RegName -ErrorAction SilentlyContinue) -ne $null
        $finalAutoStart = Read-YesNoPrompt "Enable auto-start on Windows login?" -Default $defaultAutoStart
    }

    # Start Menu shortcut
    if (-not $PSBoundParameters.ContainsKey('Shortcut')) {
        $finalShortcut = Read-YesNoPrompt "Create a Start Menu shortcut?" -Default $true
    }

    # Desktop shortcut
    if (-not $PSBoundParameters.ContainsKey('DesktopShortcut')) {
        $finalDesktopShortcut = Read-YesNoPrompt "Create a Desktop shortcut?" -Default $false
    }

    # Launch immediately
    if (-not $PSBoundParameters.ContainsKey('Launch')) {
        $finalLaunch = Read-YesNoPrompt "Launch Floating TODO immediately after install?" -Default $true
    }
}

# 5. Perform installation
Invoke-AppInstall -OptAutoStart $finalAutoStart -OptShortcut $finalShortcut -OptDesktopShortcut $finalDesktopShortcut -OptLaunch $finalLaunch
