# Singularity

Singularity is a desktop Matrix client written in tauri.

The main point of singularity is to make performant matrix client. Most of the computational logic is handled in Rust utilizing multi-threading and async.

## Prerequisites

Required tools:

- Node.js 22+
- pnpm
- Rust toolchain (see rust-toolchain.toml)
- cargo-tauri

On Linux, you also need Tauri/WebKitGTK native dependencies.

If you use Nix, a ready-to-use dev shell is provided:

```bash
nix develop
```

## Getting Started

1. Install JS dependencies:

```bash
pnpm install
```

2. Run in desktop dev mode:

```bash
pnpm tauri dev
```

3. Build production artifacts:

```bash
pnpm tauri build
```

## Development Commands

Frontend checks:

```bash
pnpm check
```

Frontend build:

```bash
pnpm build
```

Rust tests (from src-tauri):

```bash
cargo test
```