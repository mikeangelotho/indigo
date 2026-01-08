# Indigo Project Context

## Project Overview

**Indigo** is a distributed AI inference and orchestration system. It separates the orchestration logic (Hub) from the compute-heavy inference tasks (Node), allowing for flexible deployment and management of AI agents.

### Core Architecture

*   **Indigo Hub (`indigo-core/crates/indigo-hub`):** The central authority and orchestrator.
    *   **Role:** Manages client connections (WebSockets), orchestrates inference tasks, maintains state (SQLite), and handles tool execution (Agents).
    *   **Tech Stack:** Rust, Axum (HTTP/WS), Tonic (gRPC), Tokio.
    *   **Ports:** HTTP/WS on `3001`, gRPC on `3002`.
*   **Indigo Node (`indigo-core/crates/indigo-node`):** The inference worker.
    *   **Role:** Performs LLM inference using the Candle framework. Connects to the Hub via gRPC to receive tasks and stream tokens.
    *   **Tech Stack:** Rust, Candle, Tonic (gRPC).
    *   **Ports:** gRPC (default `50050`), HTTP Tool API (default `50150`).
*   **Indigo UI (`indigo-ui`):** The user interface.
    *   **Role:** A web-based chat interface for interacting with Agents and managing the system.
    *   **Tech Stack:** TypeScript, SolidStart (SolidJS), Vinxi, TailwindCSS.

## Building and Running

### Prerequisites

*   **Rust:** Latest stable toolchain (`rustup update`).
*   **Node.js:** Version 22+ (as specified in `package.json`).
*   **Protobuf Compiler:** Required for building the gRPC definitions (e.g., `protoc`).

### Backend (Indigo Core)

The backend is a Rust workspace located in `indigo-core`.

1.  **Build:**
    ```bash
    cd indigo-core
    cargo build
    ```

2.  **Run Hub:**
    ```bash
    cargo run -p indigo-hub
    # Optional: Run with a local sidecar node
    cargo run -p indigo-hub -- --with-sidecar
    ```

3.  **Run Node:**
    ```bash
    cargo run -p indigo-node -- --model-file <path-to-gguf> --port 50050
    ```

### Frontend (Indigo UI)

The frontend is a SolidStart application located in `indigo-ui`.

1.  **Install Dependencies:**
    ```bash
    cd indigo-ui
    npm install
    ```

2.  **Run Development Server:**
    ```bash
    npm run dev
    ```
    This typically starts the UI on `http://localhost:3000`.

## Development Conventions

*   **Rust Workspace:** The `indigo-core` directory is a Cargo workspace. Shared logic and protobuf definitions reside in `indigo-common`.
*   **Protobufs:** Interface definitions are in `indigo-core/proto/inference.proto`. Changes here require rebuilding `indigo-common` to regenerate Rust code.
*   **Communication:**
    *   **Node <-> Hub:** gRPC for registration and inference streaming.
    *   **Client <-> Hub:** WebSockets for chat streaming, HTTP for REST API.
*   **Tooling:**
    *   **Linting:** `cargo clippy` for Rust, `eslint` (implied) for TypeScript.
    *   **Formatting:** `cargo fmt` for Rust, `prettier` (implied) for TypeScript.

## Key Directories

*   `indigo-core/crates/indigo-hub`: Source for the Hub service.
*   `indigo-core/crates/indigo-node`: Source for the Node service.
*   `indigo-core/crates/indigo-common`: Shared Rust crates and generated protobuf code.
*   `indigo-ui/src`: Source code for the SolidStart frontend.
*   `docs/`: Architecture and Protocol documentation.
