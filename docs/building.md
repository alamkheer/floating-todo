# Building from Source

This guide explains how to compile **Floating TODO** from source, detailing both the containerized Docker build workflow and the native Windows Cargo compilation environment.

---

## Table of Contents

- [Prerequisites](#prerequisites)
- [Method A: Automated Docker Build (Recommended)](#method-a-automated-docker-build-recommended)
  - [Option 1: One-Click PowerShell Script](#option-1-one-click-powershell-script)
  - [Option 2: Direct Docker Commands](#option-2-direct-docker-commands)
  - [How the Docker Build Works](#how-the-docker-build-works)
- [Method B: Native Windows Build (Cargo)](#method-b-native-windows-build-cargo)
- [Binary Size Profile & Compiler Flags](#binary-size-profile--compiler-flags)
- [Running Automated Verification & Tests](#running-automated-verification--tests)

---

## Prerequisites

### Target Environment
- **Operating System:** Windows 10 (Build 1809 or higher) or Windows 11 (64-bit).

### Build Tooling Options
Choose either Docker (no host C++ compiler needed) or Native Rust:

- **Option 1 (Recommended): [Docker Desktop](https://www.docker.com/products/docker-desktop/)**
  - No Windows SDK, MSVC, or Rust toolchain required on the host system.
  - Generates a bit-for-bit reproducible Windows PE binary via `cargo-xwin`.
- **Option 2: Native Windows Toolchain**
  - [Rust toolchain](https://www.rust-lang.org/) (stable, 2021 edition).
  - Target: `rustup target add x86_64-pc-windows-msvc`.
  - Visual Studio C++ Build Tools (including the Windows 10/11 SDK).

---

## Method A: Automated Docker Build (Recommended)

The repository provides a hermetic Docker compilation environment using `cargo-xwin`, Clang, and Microsoft's CRT/SDK headers.

### Option 1: One-Click PowerShell Script

Run the automated PowerShell build script from the repository root:

```powershell
.\build.ps1
```

Upon completion, `floating-todo.exe` is placed directly in the project root with reported binary size diagnostics.

### Option 2: Direct Docker Commands

You can execute the extraction pipeline manually in any shell (PowerShell, Command Prompt, or Linux/macOS Bash):

```bash
# 1. Build the Docker image containing Rust, cargo-xwin, and MSVC SDK
docker build -t floating-todo-builder .

# 2. Create a temporary container to extract the artifact
docker create --name ftodo-extract floating-todo-builder

# 3. Copy the compiled release executable to the project directory
docker cp ftodo-extract:/app/target/x86_64-pc-windows-msvc/release/floating-todo.exe .\floating-todo.exe

# 4. Remove the temporary container
docker rm -f ftodo-extract
```

### How the Docker Build Works

Under the hood, the `Dockerfile`:
1. Uses an official Debian-based Rust compiler base image.
2. Installs `cargo-xwin`, `clang`, and `llvm-rc`.
3. Downloads Microsoft CRT and Windows SDK headers directly using `xwin`.
4. Executes `build.rs`, compiling the dynamic Windows PE `VERSIONINFO` resource via `llvm-rc`.
5. Compiles with `x86_64-pc-windows-msvc` target using aggressive size optimization.

---

## Method B: Native Windows Build (Cargo)

If you have Rust and the Visual Studio C++ Build Tools installed natively:

```powershell
# 1. Ensure the MSVC target is installed
rustup target add x86_64-pc-windows-msvc

# 2. Compile release binary
cargo build --release

# The compiled binary is output to:
# target\release\floating-todo.exe
```

---

## Binary Size Profile & Compiler Flags

Floating TODO achieves its ultralight ~231 KB executable footprint via aggressive optimization flags defined in `Cargo.toml`:

```toml
[profile.release]
opt-level = "z"        # Optimize aggressively for binary size over raw speed
lto = true             # Full Link-Time Optimization across all crates
codegen-units = 1      # Single compilation unit for maximum dead-code elimination
strip = true           # Strip debug symbols and symbol tables
panic = "abort"        # Abort on panic (removes unwinding machinery and landing pads)
```

- **`opt-level = "z"`**: Tells LLVM to prioritize binary compactness over instruction throughput.
- **`lto = true` & `codegen-units = 1`**: Enables whole-program link-time optimization, allowing the compiler to aggressively strip unused Win32 function pointers.
- **`strip = true`**: Removes symbols and string tables from the final PE binary.
- **`panic = "abort"`**: Strips unwinding personality functions and stack frame unwinding tables.

---

## Running Automated Verification & Tests

To run the architectural specification verifier and unit test suite:

```powershell
# Run the full test suite (Docker build, size check, PE verification, memory/CPU benchmarks)
.\test.ps1

# Run only Rust unit tests
.\test.ps1 -RustOnly

# Run only specification and binary assertions against floating-todo.exe
.\test.ps1 -SpecsOnly

# Skip rebuilding container image and skip GUI runtime launch
.\test.ps1 -SkipBuild -SkipRuntime
```
*(Alternatively, you can call `.\tests.ps1`, which forwards arguments directly to `test.ps1`).*
