FROM debian:bookworm-slim

# Prevent interactive prompts during package installation
ENV DEBIAN_FRONTEND=noninteractive

# ── Pinned versions ─────────────────────────────────────────────────────────
ARG NODE_VERSION=22.17.0

# ── System dependencies ──────────────────────────────────────────────────────
# Tauri runtime: GTK3, WebKitGTK 4.1, GStreamer, OpenSSL, etc.

RUN apt-get update && apt-get install -y --no-install-recommends \
    # Build essentials
    build-essential \
    cmake \
    curl \
    git \
    pkg-config \
    ca-certificates \
    wget \
    xz-utils \
    fish \
    \
    # Tauri runtime dependencies
    libgtk-3-dev \
    libwebkit2gtk-4.1-dev \
    libcairo2-dev \
    libgdk-pixbuf-2.0-dev \
    libglib2.0-dev \
    libdbus-1-dev \
    libssl-dev \
    librsvg2-dev \
    libsqlite3-dev \
    \
    # GStreamer
    libgstreamer1.0-dev \
    libgstreamer-plugins-base1.0-dev \
    gstreamer1.0-plugins-good \
    gstreamer1.0-plugins-bad \
    gstreamer1.0-plugins-ugly \
    gstreamer1.0-libav \
    gstreamer1.0-vaapi \
    \
    # Graphics / display
    libgl1-mesa-dev \
    libglu1-mesa-dev \
    libgbm1 \
    \
    # Additional Tauri helpers
    libayatana-appindicator3-dev \
    librsvg2-bin \
    ssh \
    \
    # Desktop file utilities (required for OAuth browser authentication)
    desktop-file-utils \
    && rm -rf /var/lib/apt/lists/*

# ── Node.js (static tarball — no network installer) ─────────────────────────
RUN wget -qO- "https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-x64.tar.xz" \
    | tar -xJ -C /usr/local --strip-components=1 \
    && node --version

# ── Rust (rustup — uses latest stable, ignores rust-toolchain.toml) ─────────
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y \
    && . "$HOME/.cargo/env" \
    && rustup component add rust-src \
    && chmod -R a+rX /root/.cargo

# ── Environment variables ───────────────────────────────────────────────────
ENV PATH="/root/.cargo/bin:${PATH}"

# Dev container flag: frontend detects this and shows authorization URL for
# manual copy-and-paste into the host browser, plus a callback URL input.
ENV SINGULARITY_DEV_CONTAINER="true"
