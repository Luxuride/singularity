# Singularity

Singularity is a desktop Matrix client written in tauri.

The main point of singularity is to make performant matrix client. Most of the computational logic is handled in Rust utilizing multi-threading and async.

## Prerequisites

### Option 1: VS Code Dev Containers (recommended)

1. Install the [Dev Containers](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers) extension in VS Code.
2. Open the workspace in a container:
   - Press `Ctrl+Shift+P` → **Dev Containers: Reopen in Container**
3. The container will build with all dependencies (Node.js, Rust, Tauri native libs) pre-installed.

### Option 2: Manual setup

Required tools:

- Node.js 22+
- pnpm
- Rust toolchain (stable)
- cargo-tauri

On Linux, you also need Tauri/WebKitGTK native dependencies.

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