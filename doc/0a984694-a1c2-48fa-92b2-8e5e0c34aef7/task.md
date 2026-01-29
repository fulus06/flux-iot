# FLUX IOT Project Design & Implementation Roadmap

## Phase 0: Project Setup
- [x] Design Project Structure <!-- id: 0 -->
    - [x] Analyze requirements from docs <!-- id: 1 -->
    - [x] Define Rust Workspace structure <!-- id: 2 -->
    - [x] Define Crate boundaries <!-- id: 3 -->
- [x] Implement Project Skeleton <!-- id: 4 -->
    - [x] Create Cargo Workspace <!-- id: 5 -->
    - [x] Create Core Crates (core, server, plugin-api, etc.) <!-- id: 6 -->
    - [x] Define shared Types/Traits <!-- id: 7 -->
- [x] Document Architecture <!-- id: 8 -->
    - [x] Create ARCHITECTURE.md detailed guide <!-- id: 9 -->

## Phase 1: Core Foundation & Types
- [x] **Data Types Definition** (`flux-types`) <!-- id: 10 -->
    - [x] Define `Message` struct (Standard Event) <!-- id: 11 -->
    - [x] Define `DeviceData` and `Command` structs <!-- id: 12 -->
    - [x] Fix Wasm compatibility for `flux-types` (handle `uuid`/`getrandom` issues) <!-- id: 13 -->
- [x] **Event Bus** (`flux-core`) <!-- id: 14 -->
    - [x] Implement async `EventBus` using tokio broadcast/mpsc <!-- id: 15 -->
    - [x] Define Subscriber/Publisher traits <!-- id: 16 -->

## Phase 2: Plugin System (Wasmtime)
- [x] **Plugin Host Implementation** (`flux-plugin`) <!-- id: 17 -->
    - [x] Configure Wasmtime Engine (Enable Epoch interruption for safety) <!-- id: 18 -->
    - [x] Implement Memory Bridge (Shared Memory for passing strings/bytes) <!-- id: 19 -->
    - [x] Implement `PluginManager` to load/unload `.wasm` files <!-- id: 20 -->
- [x] **Plugin SDK** (`flux-plugin-sdk`) <!-- id: 21 -->
    - [x] Create convenience macros (`export_plugin!`) <!-- id: 22 -->
    - [x] Example: Implement a `dummy_plugin` for testing <!-- id: 23 -->

## Phase 3: Script Engine (Rhai)
- [x] **Script Engine Implementation** (`flux-script`) <!-- id: 24 -->
    - [x] Configure `rhai::Engine` with safety limits (max operations) <!-- id: 25 -->
    - [x] Register `flux-types` into Rhai Scope <!-- id: 26 -->
    - [x] Implement `ScriptService` for hot-reloading scripts <!-- id: 27 -->

## Phase 4: Server Integration & API
- [x] **Server Application** (`flux-server`) <!-- id: 28 -->
    - [x] Wire up EventBus, PluginManager, and ScriptEngine <!-- id: 29 -->
    - [x] Implement Configuration Loading (`config.toml`) <!-- id: 30 -->
    - [x] Basic HTTP API (Axum) for status checks <!-- id: 31 -->

## Phase 5: Functional Implementation (Next Steps)
- [x] **Plugin Auto-Loading** <!-- id: 32 -->
    - [x] Create `plugins/` directory support in Server <!-- id: 33 -->
    - [x] Auto-load `.wasm` files on startup <!-- id: 34 -->
- [x] **Device Data API** <!-- id: 35 -->
    - [x] Implement `POST /api/v1/event` endpoint <!-- id: 36 -->
    - [x] Wire API -> EventBus -> Script Engine <!-- id: 37 -->
- [x] **Rule Engine Logic** <!-- id: 38 -->
    - [x] Create `RuleWorker` that listens to `EventBus` <!-- id: 39 -->
    - [x] Execute Rhai scripts against incoming data <!-- id: 40 -->

## Phase 6: Persistence & Dynamic Rules (Next Priority)
- [x] **Database Integration (Sea-ORM)** <!-- id: 41 -->
    - [x] Define Entities (`devices`, `events`, `rules`) <!-- id: 42 -->
    - [x] Setup Database Connection & Migrations <!-- id: 43 -->
- [x] **Dynamic Rule Management** <!-- id: 44 -->
    - [x] Load rules from Database (replace hardcoded implementation) <!-- id: 45 -->
    - [x] Implement CRUD APIs for Rules (`POST /api/v1/rules`) <!-- id: 46 -->
        - [x] Added `GET /api/v1/rules` to list active rules
        - [x] Added `POST /api/v1/rules/reload` to hot-reload from DB
- [x] **Data Storage** <!-- id: 47 -->
    - [x] Asynchronously save `Message` to `events` table <!-- id: 48 -->

## Phase 7: Protocol & Connectivity
- [x] **MQTT Integration (ntex-mqtt)** <!-- id: 49 -->
    - [x] Create `flux-mqtt` crate with `ntex` and `ntex-mqtt` <!-- id: 50 -->
    - [x] Implement Manager and Handlers for v3/v5 <!-- id: 51 -->
    - [x] Implement Control Services (Subscribe, Ping, Disconnect)
    - [x] Implement Session Cleanup & Graceful Shutdown
    - [x] Bridge subscriber to `EventBus`
    - [x] Upgrade to `ntex-mqtt` v7.0.0-pre.1

## Phase 8: Hardening & Advanced Features (Proposed)
- [/] **MQTT Security**
    - [x] **Design Authentication Interface**
        - [x] Define `Authenticator` trait in `flux-core` (async validation)
        - [x] Update `flux-mqtt` to accept `Authenticator`
    - [x] **Device Authentication (DB)**
        - [x] Update `devices` table: Add `token` (password) field
        - [x] Implement `DeviceAuthenticator` in `flux-server` (check DB)
        - [x] Support `ClientId` validation (must verify against DB)
        - [x] Support `Username/Password` validation
    - [ ] **ACL (Optional/Next)** (`[Low Priority]`)
- [x] **Advanced Rule Engine**
    - [x] **State Persistence**
        - [x] Add `state_store` to `ScriptEngine`
        - [x] Implement `state_get(key)` in Rhai
        - [x] Implement `state_set(key, val)` in Rhai
    - [x] **Time Functions**
        - [x] Implement `now_ms()` in Rhai
    - [x] **Verification**
        - [x] Create stateful rule (counter)
        - [x] Verify persistence across messages
- [ ] **System Observability**
    - [ ] Add Metrics (Prometheus endpoint)
    - [ ] Structured Logging improvements
