# FLUX IOT Arithectural Guide

## Project Structure

This project uses a **Cargo Workspace** to manage modular components.

| Crate | Path | Type | Responsibility |
| :--- | :--- | :--- | :--- |
| **flux-server** | `crates/flux-server` | Binary | Application entry point. Wires up Axum, Tokio, and other crates. |
| **flux-core** | `crates/flux-core` | Library | **The Kernel**. Contains domain logic, database entities (SeaORM), and service layers. |
| **flux-types** | `crates/flux-types` | Library | **Shared Types**. DTOs and Enums shared between Core and Plugins. |
| **flux-plugin** | `crates/flux-plugin` | Library | **Wasm Host**. Manages Wasmtime, plugin loading, and host/guest memory bridging. |
| **flux-script** | `crates/flux-script` | Library | **Script Host**. Wraps Rhai engine for dynamic logic execution. |
| **flux-plugin-sdk**| `sdk/flux-plugin-sdk` | Library | **Dev Kit**. Used by plugin developers to write Wasm plugins. |

## Development Workflow

### 1. Build the Server
```bash
cargo build -p flux-server
```

### 2. Run the Server
```bash
cargo run -p flux-server -- --config config.toml
```

### 3. Develop a Plugin
Create a new Rust project (e.g. `plugins/modbus-tcp`) and add `flux-plugin-sdk` as a dependency.
Build with:
```bash
cargo build -p flux-plugin-sdk --target wasm32-unknown-unknown
```

## Core Principles
*   **Zero-Cost Abstractions**: Use Rust generics and traits where possible.
*   **Isolation**: Plugins cannot crash the Core. They run in Wasm sandboxes.
*   **Async I/O**: All I/O operations (Database, Network) are non-blocking via `Tokio`.
