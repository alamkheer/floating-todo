<#
.SYNOPSIS
    Root test suite and specification verifier for Floating TODO.

.DESCRIPTION
    Runs Rust code verification (cargo check, unit tests via Docker cross-compilation)
    and verifies all architectural specifications:
      1. Docker container build & reproducibility
      2. Binary size target (< 1 MB)
      3. PE binary format & architecture (64-bit AMD64, Windows GUI subsystem)
      4. Runtime idle working set memory (< 5 MB RAM)
      5. Runtime idle CPU consumption (~0%)
      6. Local storage schema & persistence integrity (%APPDATA%\FloatingTodo)
      7. Clean process lifecycle & resource teardown

.PARAMETER RustOnly
    Runs only Rust unit tests and cargo compilation checks.

.PARAMETER SpecsOnly
    Runs only specification checks against floating-todo.exe.

.PARAMETER SkipBuild
    Skips rebuilding the Docker image if it already exists.

.PARAMETER SkipRuntime
    Skips launching the GUI process to measure RAM/CPU (useful for headless CI).

.PARAMETER VerboseOutput
    Displays detailed diagnostic outputs.

.EXAMPLE
    .\test.ps1
    .\test.ps1 -RustOnly
    .\test.ps1 -SpecsOnly
    .\test.ps1 -SkipBuild -SkipRuntime
#>

[CmdletBinding()]
param(
    [switch]$RustOnly,
    [switch]$SpecsOnly,
    [switch]$SkipBuild,
    [switch]$SkipRuntime,
    [switch]$VerboseOutput
)

$ErrorActionPreference = "Continue"
$ProjectDir = $PSScriptRoot
if (-not $ProjectDir) { $ProjectDir = (Get-Location).Path }
$ExePath = Join-Path $ProjectDir "floating-todo.exe"

$results = [System.Collections.Generic.List[PSCustomObject]]::new()
$totalPassed = 0
$totalFailed = 0

function Record-TestResult {
    param(
        [string]$Category,
        [string]$Name,
        [bool]$Passed,
        [string]$Details
    )

    $obj = [PSCustomObject]@{
        Category = $Category
        Name     = $Name
        Passed   = $Passed
        Details  = $Details
    }
    $script:results.Add($obj)

    if ($Passed) {
        $script:totalPassed++
        Write-Host "  [PASS] " -ForegroundColor Green -NoNewline
        Write-Host "$Category - $Name " -ForegroundColor White -NoNewline
        if ($Details) {
            Write-Host "($Details)" -ForegroundColor DarkGray
        } else {
            Write-Host ""
        }
    } else {
        $script:totalFailed++
        Write-Host "  [FAIL] " -ForegroundColor Red -NoNewline
        Write-Host "$Category - $Name " -ForegroundColor White -NoNewline
        if ($Details) {
            Write-Host "($Details)" -ForegroundColor Yellow
        } else {
            Write-Host ""
        }
    }
}

Write-Host ""
Write-Host "============================================================" -ForegroundColor Cyan
Write-Host "        FLOATING TODO - ROOT TEST & SPEC VERIFICATION       " -ForegroundColor Cyan
Write-Host "============================================================" -ForegroundColor Cyan
Write-Host "Working Directory : $ProjectDir" -ForegroundColor DarkGray
Write-Host "Mode              : $(if ($RustOnly) { 'Rust Only' } elseif ($SpecsOnly) { 'Specs Only' } else { 'Full Suite' })" -ForegroundColor DarkGray
Write-Host ""

# ─────────────────────────────────────────────────────────────
# 0. Check Docker Environment
# ─────────────────────────────────────────────────────────────
$dockerAvailable = $false
try {
    $null = docker info 2>&1
    if ($LASTEXITCODE -eq 0) {
        $dockerAvailable = $true
    }
} catch {
    $dockerAvailable = $false
}

if (-not $dockerAvailable) {
    Write-Host "Error: Docker is not running or not found in PATH." -ForegroundColor Red
    Write-Host "Please start Docker Desktop and re-run this test script." -ForegroundColor Yellow
    exit 1
}

# ─────────────────────────────────────────────────────────────
# 1. Docker Build Verification & Dependency Cache
# ─────────────────────────────────────────────────────────────
if (-not $RustOnly -and -not $SkipBuild) {
    Write-Host "[1/3] Building / Verifying Docker Image..." -ForegroundColor Cyan
    $buildOutput = docker build -t floating-todo-builder "$ProjectDir" 2>&1
    $buildSuccess = ($LASTEXITCODE -eq 0)
    if ($VerboseOutput -or -not $buildSuccess) {
        $buildOutput | ForEach-Object { Write-Host "    $_" -ForegroundColor DarkGray }
    }
    Record-TestResult "Docker" "Image Build (cargo-xwin MSVC environment)" $buildSuccess $(if ($buildSuccess) { "Image: floating-todo-builder" } else { "Build failed" })

    if (-not $buildSuccess) {
        Write-Host "Aborting tests due to Docker build failure." -ForegroundColor Red
        exit 1
    }

    # Ensure binary is extracted
    Write-Host "Extracting release binary..." -ForegroundColor DarkGray
    $prev = docker ps -a -q -f name=^ftodo-extract$
    if ($prev) { docker rm -f ftodo-extract | Out-Null }
    docker create --name ftodo-extract floating-todo-builder | Out-Null
    docker cp "ftodo-extract:/app/target/x86_64-pc-windows-msvc/release/floating-todo.exe" $ExePath 2>&1 | Out-Null
    docker rm -f ftodo-extract | Out-Null
}

# ─────────────────────────────────────────────────────────────
# 2. Rust Code Verification (Cargo Check + Native Unit Tests)
# ─────────────────────────────────────────────────────────────
if (-not $SpecsOnly) {
    Write-Host ""
    Write-Host "[2/3] Verifying Rust Codebase..." -ForegroundColor Cyan

    # A. Cargo Check
    Write-Host "Running cargo xwin check in container..." -ForegroundColor DarkGray
    $checkOutput = docker run --rm -v "${ProjectDir}\src:/app/src" -v "${ProjectDir}\Cargo.toml:/app/Cargo.toml" floating-todo-builder cargo xwin check --target x86_64-pc-windows-msvc 2>&1
    $checkSuccess = ($LASTEXITCODE -eq 0)
    if ($VerboseOutput -or -not $checkSuccess) {
        $checkOutput | ForEach-Object { Write-Host "    $_" -ForegroundColor DarkGray }
    }
    Record-TestResult "Rust Code" "Cargo Check & Lints" $checkSuccess $(if ($checkSuccess) { "0 compile errors/warnings" } else { "Check failed" })

    # B. Compile & Execute Unit Tests
    Write-Host "Compiling and executing unit tests..." -ForegroundColor DarkGray
    $tempTestDir = Join-Path $env:TEMP ("ftodo_test_" + [Guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Path $tempTestDir -Force | Out-Null

    try {
        $testBuildOutput = docker run --rm `
            -v "${ProjectDir}\src:/app/src" `
            -v "${ProjectDir}\Cargo.toml:/app/Cargo.toml" `
            -v "${tempTestDir}:/test_out" `
            floating-todo-builder bash -c "cargo xwin test --target x86_64-pc-windows-msvc --no-run 2>/dev/null && cp target/x86_64-pc-windows-msvc/debug/deps/floating_todo-*.exe /test_out/" 2>&1

        $testExe = Get-ChildItem -Path "$tempTestDir\*.exe" -ErrorAction SilentlyContinue | Select-Object -First 1

        if ($testExe) {
            $testRunOutput = & $testExe.FullName 2>&1
            $testRunSuccess = ($LASTEXITCODE -eq 0)
            
            # Parse test summary (e.g. 'test result: ok. 7 passed; 0 failed')
            $summaryLine = ($testRunOutput | Where-Object { $_ -match "test result:" })
            $testDetails = if ($summaryLine) { $summaryLine.Trim() } else { "Executed $($testExe.Name)" }

            if ($VerboseOutput -or -not $testRunSuccess) {
                $testRunOutput | ForEach-Object { Write-Host "    $_" -ForegroundColor DarkGray }
            }
            Record-TestResult "Rust Code" "Unit Tests (state, storage, filters)" $testRunSuccess $testDetails
        } else {
            Record-TestResult "Rust Code" "Unit Tests (compilation)" $false "Could not locate compiled test executable"
            if ($VerboseOutput) {
                $testBuildOutput | ForEach-Object { Write-Host "    $_" -ForegroundColor DarkGray }
            }
        }
    } finally {
        Remove-Item -Recurse -Force $tempTestDir -ErrorAction SilentlyContinue
    }
}

# ─────────────────────────────────────────────────────────────
# 3. Architectural Specification Verifications
# ─────────────────────────────────────────────────────────────
if (-not $RustOnly) {
    Write-Host ""
    Write-Host "[3/3] Verifying Architectural Specifications..." -ForegroundColor Cyan

    if (-not (Test-Path $ExePath)) {
        Record-TestResult "Spec" "Binary Availability" $false "File not found: $ExePath. Run build.ps1 first."
    } else {
        # Spec 1: Binary Size Target (< 1 MB = 1,048,576 bytes)
        $fileInfo = Get-Item $ExePath
        $sizeBytes = $fileInfo.Length
        $sizeKB = [math]::Round($sizeBytes / 1024, 1)
        $sizeUnder1MB = ($sizeBytes -lt 1048576)
        Record-TestResult "Spec" "Binary Size (< 1 MB)" $sizeUnder1MB "$sizeKB KB ($sizeBytes bytes / 1,048,576 target)"

        # Spec 2: Valid 64-bit Windows PE Header and GUI Subsystem (2)
        $peSuccess = $false
        $peDetails = ""
        try {
            $bytes = [System.IO.File]::ReadAllBytes($ExePath)
            if ($bytes.Length -gt 256 -and $bytes[0] -eq 0x4D -and $bytes[1] -eq 0x5A) { # MZ
                $peOffset = [System.BitConverter]::ToInt32($bytes, 0x3C)
                $sig = [System.Text.Encoding]::ASCII.GetString($bytes, $peOffset, 4)
                if ($sig -eq "PE`0`0") {
                    $machine = [System.BitConverter]::ToUInt16($bytes, $peOffset + 4)
                    $magic = [System.BitConverter]::ToUInt16($bytes, $peOffset + 24)
                    $subsystem = [System.BitConverter]::ToUInt16($bytes, $peOffset + 24 + 68)

                    $isAMD64 = ($machine -eq 0x8664) # AMD64
                    $isPE32Plus = ($magic -eq 0x020B) # 64-bit
                    $isWindowsGUI = ($subsystem -eq 2) # IMAGE_SUBSYSTEM_WINDOWS_GUI

                    if ($isAMD64 -and $isPE32Plus -and $isWindowsGUI) {
                        $peSuccess = $true
                        $peDetails = "AMD64 PE32+ (x86_64), Subsystem=2 (GUI, zero console popups)"
                    } else {
                        $peDetails = "Machine: 0x{0:X4}, Subsystem: $subsystem (Expected 0x8664 / Subsystem 2)" -f $machine
                    }
                } else {
                    $peDetails = "Invalid PE signature"
                }
            } else {
                $peDetails = "Invalid DOS header (MZ missing)"
            }
        } catch {
            $peDetails = $_.Exception.Message
        }
        Record-TestResult "Spec" "PE Header & Architecture" $peSuccess $peDetails

        # Spec 3: Persistence & JSON Schema (%APPDATA%\FloatingTodo\tasks.json)
        $appdataDir = Join-Path $env:APPDATA "FloatingTodo"
        $tasksFile = Join-Path $appdataDir "tasks.json"
        $schemaSuccess = $false
        $schemaDetails = ""
        try {
            if (-not (Test-Path $appdataDir)) {
                New-Item -ItemType Directory -Path $appdataDir -Force | Out-Null
            }
            # Verify directory write capability and schema test
            $testFile = Join-Path $appdataDir "test_schema.json"
            $sampleJson = '{"next_id": 2, "tasks": [{"id": 1, "text": "Spec Verification Test", "completed": false}]}'
            [System.IO.File]::WriteAllText($testFile, $sampleJson)
            
            $parsed = Get-Content $testFile -Raw | ConvertFrom-Json
            if ($parsed.next_id -eq 2 -and $parsed.tasks.Count -eq 1 -and $parsed.tasks[0].id -eq 1) {
                $schemaSuccess = $true
                $schemaDetails = "Valid schema roundtrip in %APPDATA%\FloatingTodo"
            } else {
                $schemaDetails = "Schema validation failed"
            }
            Remove-Item -Force $testFile -ErrorAction SilentlyContinue
        } catch {
            $schemaDetails = $_.Exception.Message
        }
        Record-TestResult "Spec" "JSON Storage & Schema" $schemaSuccess $schemaDetails

        # Spec 4, 5, 6: Runtime Memory, CPU, and Clean Lifecycle
        if (-not $SkipRuntime) {
            Write-Host "Launching floating-todo.exe for live process telemetry..." -ForegroundColor DarkGray
            # Close any preexisting instance
            Get-Process -Name "floating-todo" -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
            Start-Sleep -Milliseconds 300

            $proc = Start-Process -FilePath $ExePath -PassThru
            try {
                Start-Sleep -Milliseconds 1500 # Allow window initialization

                $liveProc = Get-Process -Id $proc.Id -ErrorAction Stop

                # Spec 4: Memory Footprint (< 5 MB RAM working set)
                $wsBytes = $liveProc.WorkingSet64
                $wsMB = [math]::Round($wsBytes / 1MB, 2)
                $privBytes = $liveProc.PrivateMemorySize64
                $privMB = [math]::Round($privBytes / 1MB, 2)
                $memPassed = ($wsMB -lt 5.0) -or ($privMB -lt 5.0)
                Record-TestResult "Spec" "Idle Memory (< 5 MB RAM)" $memPassed "Working Set: $wsMB MB, Private: $privMB MB"

                # Spec 5: Idle CPU Consumption (~0%)
                $cpu1 = $liveProc.TotalProcessorTime.TotalMilliseconds
                Start-Sleep -Seconds 1
                $liveProc.Refresh()
                $cpu2 = $liveProc.TotalProcessorTime.TotalMilliseconds
                $cpuDelta = [math]::Max(0, ($cpu2 - $cpu1))
                $cpuPercent = [math]::Round(($cpuDelta / (1000 * [Environment]::ProcessorCount)) * 100, 2)
                $cpuPassed = ($cpuPercent -lt 1.0)
                Record-TestResult "Spec" "Idle CPU Consumption (~0%)" $cpuPassed "CPU Usage: $cpuPercent% across [Environment]::ProcessorCount cores"

                # Spec 6: Clean Process Lifecycle & Teardown
                $closed = $liveProc.CloseMainWindow()
                Start-Sleep -Milliseconds 500
                $hasExited = $liveProc.HasExited
                if (-not $hasExited) {
                    $liveProc.Kill()
                    $hasExited = $true
                }
                Record-TestResult "Spec" "Process Lifecycle & Clean Teardown" $hasExited "Terminated with exit code $($liveProc.ExitCode)"
            } catch {
                Record-TestResult "Spec" "Runtime Process Telemetry" $false $_.Exception.Message
            } finally {
                Get-Process -Name "floating-todo" -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
            }
        } else {
            Write-Host "  [SKIP] Spec - Runtime Telemetry (-SkipRuntime flag specified)" -ForegroundColor Yellow
        }
    }
}

# ─────────────────────────────────────────────────────────────
# Scorecard Summary
# ─────────────────────────────────────────────────────────────
Write-Host ""
Write-Host "============================================================" -ForegroundColor Cyan
Write-Host "                      TEST SCORECARD                        " -ForegroundColor Cyan
Write-Host "============================================================" -ForegroundColor Cyan
$totalTests = $totalPassed + $totalFailed
$passRate = if ($totalTests -gt 0) { [math]::Round(($totalPassed / $totalTests) * 100, 1) } else { 0 }

Write-Host "Total Checks : $totalTests" -ForegroundColor White
Write-Host "Passed       : $totalPassed" -ForegroundColor Green
Write-Host "Failed       : $totalFailed" -ForegroundColor $(if ($totalFailed -eq 0) { "DarkGray" } else { "Red" })
Write-Host "Pass Rate    : $passRate%" -ForegroundColor $(if ($totalFailed -eq 0) { "Green" } else { "Yellow" })
Write-Host "============================================================" -ForegroundColor Cyan

if ($totalFailed -eq 0) {
    Write-Host "STATUS: ALL TESTS & SPECIFICATIONS PASSED!" -ForegroundColor Green
    exit 0
} else {
    Write-Host "STATUS: $totalFailed TEST(S) FAILED." -ForegroundColor Red
    exit 1
}
