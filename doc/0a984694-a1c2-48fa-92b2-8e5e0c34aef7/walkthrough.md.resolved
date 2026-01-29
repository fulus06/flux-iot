# Walkthrough: Re-implementing MQTT Broker with ntex-mqtt

We have replaced the `rumqttd`-based embedded broker with a custom implementation using `ntex-mqtt` to support both MQTT v3.1.1 and v5.0 protocols on the same port (1883).

## Changes

### 1. New Crate: `flux-mqtt`
- **Location**: `crates/flux-mqtt`
- **Stack**: `ntex`, `ntex-mqtt` (v0.6/local), `tokio`.
- **Key Components**:
    - `manager.rs`: Manages client sessions (v3 & v5) and handles message broadcasting.
    - `handler.rs`: Implements protocol-specific handshake, publish, and control (Subscribe, Ping, Disconnect) handlers.
    - `lib.rs`: Entry point that spawns the `ntex` runtime and bridge task.

### 2. Integration with `flux-server`
- Updated `crates/flux-server/Cargo.toml` to depend on `flux-mqtt`.
- Removed `crates/flux-server/src/mqtt.rs` (legacy code).
- Modified `crates/flux-server/src/main.rs` to start the new broker:
    ```rust
    // 6. Start MQTT Broker (Ntex)
    let mqtt_bus = state.event_bus.clone();
    flux_mqtt::start_broker(mqtt_bus);
    ```
- **Updates**:
    - Added proper session cleanup in `flux-mqtt` to prevent zombie sessions.
    - Implemented graceful shutdown in `flux-server` to release ports correctly on exit.

### 3. Verification
- **Build**: `cargo build -p flux-server` passes.
- **Runtime**: `flux-server` logs confirmation:
    ```
    INFO flux_mqtt: Starting Flux MQTT Broker (ntex) on 0.0.0.0:1883
    ```
- **Functional Test**: `test_mqtt.py` (using `paho-mqtt`) successfully connects and publishes messages which are bridged to the `EventBus`.

### 4. Authentication (Phase 8)
- **Feature**: MQTT Authentication via Database.
- **Implementation**:
    - `devices` table has `token` column.
    - `flux-mqtt` checks credentials against DB.
    - `flux-server` seeds `test_device` / `password123` if DB is empty.
- **Verification Steps**:
    1.  **Reset DB**: `rm flux.db` (Schema change requires fresh DB).
    2.  **Start Server**: `cargo run -p flux-server`.
    3.  **Test Auth**:
        - Use MQTT Client (e.g. MQTTX).
        - Connect with ClientID=`test_device`, Password=`password123` -> **Success**.
        - Connect with Wrong Password -> **Failure** (Connection Closed).
        - Connect with Unknown ClientID -> **Failure**.
### 5. Advanced Rule Engine (Phase 9)
- **Feature**: State persistence and Time functions in Rules.
- **Support**:
    - `state_get(key)` / `state_set(key, val)`: Share data across executions.
    - `now_ms()`: Get current timestamp.
- **Verification**:
    - Unit Verified in `crates/flux-script/src/tests.rs`.
    - `test_state_persistence` confirms counter logic works across multiple messages.
