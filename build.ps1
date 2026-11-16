# Build inside Docker and extract the .exe to the project root.
$ErrorActionPreference = "Stop"
$ProjectDir = $PSScriptRoot
if (-not $ProjectDir) { $ProjectDir = (Get-Location).Path }

Write-Host "Building floating-todo in Docker..." -ForegroundColor Cyan

# Build the Docker image
docker build -t floating-todo-builder "$ProjectDir"
if ($LASTEXITCODE -ne 0) {
    Write-Error "Docker build failed."
    exit 1
}

# Remove any previous extraction container if it exists
$prev = docker ps -a -q -f name=^ftodo-extract$
if ($prev) {
    docker rm -f ftodo-extract | Out-Null
}

# Extract the .exe from the builder stage
docker create --name ftodo-extract floating-todo-builder | Out-Null
docker cp "ftodo-extract:/app/target/x86_64-pc-windows-msvc/release/floating-todo.exe" "$ProjectDir\floating-todo.exe"
docker rm -f ftodo-extract | Out-Null

if (-not (Test-Path "$ProjectDir\floating-todo.exe")) {
    Write-Error "Failed to extract floating-todo.exe"
    exit 1
}

$size = (Get-Item "$ProjectDir\floating-todo.exe").Length
$sizeKB = [math]::Round($size / 1024)
$color = if ($size -lt 1048576) { "Green" } else { "Yellow" }

Write-Host "Build complete: floating-todo.exe ($sizeKB KB)" -ForegroundColor $color

if ($size -ge 1048576) {
    Write-Host "Binary exceeds 1 MB target ($sizeKB KB)" -ForegroundColor Yellow
}
