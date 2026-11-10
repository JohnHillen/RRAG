# ==============================================================================
# Multi-Stage Dockerfile (with BuildKit Cargo & Target Cache):
# 1. Builds the Leptos WebAssembly frontend (rust-rag-ui) using Trunk
# 2. Builds the Axum RAG backend (rust-rag-orchestrator)
# 3. Packages both into a minimal Debian Trixie runtime image
# License: MIT / Apache 2.0
# ==============================================================================

# --- Stage 1: Build Backend & Leptos WASM Frontend ---
# Debian Trixie provides glibc 2.40+ (required for prebuilt ARM64 trunk binaries)
FROM rust:slim-trixie AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    curl \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Automatic retries on transient DNS/network hiccups in Docker Desktop
ENV CARGO_NET_RETRY=10 \
    CARGO_HTTP_TIMEOUT=120

# Update Rust stable toolchain + WebAssembly target + prebuilt binaries for Trunk & wasm-bindgen
RUN rustup update stable && \
    rustup default stable && \
    rustup target add wasm32-unknown-unknown && \
    curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash && \
    cargo binstall -y trunk wasm-bindgen-cli

WORKDIR /workspace

# 1A. Build Leptos WASM frontend (with persistent BuildKit cache for crates.io & target)
COPY rust-rag-ui/Cargo.toml rust-rag-ui/Cargo.lock rust-rag-ui/index.html ./rust-rag-ui/
COPY rust-rag-ui/src ./rust-rag-ui/src
WORKDIR /workspace/rust-rag-ui
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/workspace/rust-rag-ui/target \
    (CARGO_NET_OFFLINE=true trunk build --release || trunk build --release)

# 1B. Build Axum backend (with persistent BuildKit cache for crates.io & target)
WORKDIR /workspace/rust-rag-orchestrator
COPY rust-rag-orchestrator/Cargo.toml rust-rag-orchestrator/Cargo.lock ./
COPY rust-rag-orchestrator/src ./src
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/workspace/rust-rag-orchestrator/target \
    (CARGO_NET_OFFLINE=true cargo build --release || cargo build --release) && \
    cp /workspace/rust-rag-orchestrator/target/release/rust-rag-orchestrator /usr/local/bin/rust-rag-orchestrator

# --- Stage 2: Minimal Runtime Image (incl. Poppler & Tesseract OCR for scanned PDFs) ---
FROM debian:trixie-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    poppler-utils \
    tesseract-ocr \
    tesseract-ocr-deu \
    tesseract-ocr-eng \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /usr/local/bin/rust-rag-orchestrator /app/rust-rag-orchestrator
COPY --from=builder /workspace/rust-rag-ui/dist /app/static

EXPOSE 8080

CMD ["/app/rust-rag-orchestrator"]
