# ============================================================
# Stage 1: Builder — Rust + cargo-xwin + Windows MSVC SDK
# ============================================================
FROM rust:latest AS builder

# Install cargo-xwin (auto-downloads Microsoft CRT + Windows SDK)
RUN cargo install cargo-xwin --locked

# Add the Windows MSVC target
RUN rustup target add x86_64-pc-windows-msvc

# Install clang/lld (required by cargo-xwin for MSVC cross-linking)
RUN apt-get update && apt-get install -y \
    clang \
    lld \
    && rm -rf /var/lib/apt/lists/*

# Symlink llvm-rc for Windows resource compilation
RUN ln -sf /usr/bin/llvm-rc-19 /usr/local/bin/llvm-rc || true

# Create app directory
WORKDIR /app

# ── Dependency caching layer ──
# Copy only manifests first so cargo xwin build caches dependencies.
# Source changes won't invalidate this expensive layer.
COPY Cargo.toml Cargo.lock* ./
RUN mkdir -p src && echo "fn main() {}" > src/main.rs
RUN cargo xwin build --target x86_64-pc-windows-msvc --release 2>/dev/null || true
RUN rm -rf src

# ── Build the actual application ──
COPY build.rs* ./
COPY src/ src/
COPY assets/ assets/

# Touch main.rs to force recompile (not the deps)
RUN touch src/main.rs

RUN cargo xwin build --target x86_64-pc-windows-msvc --release
