# Implementation Plan - Phase 9: Advanced Rule Engine

## Goal
Enhance the Rule Engine to support stateful logic and time-based conditions. This allows use cases like "Alert if temperature > 50 for 5 minutes" or "Count number of failures".

## User Requirements
- Support **Shared State** between rule executions.
- Support **time-based** logic (e.g., getting current timestamp).

## Proposed Changes

### 1. Script Engine Enhancements (`flux-script`)
We need to introduce a mechanism to persist data across script executions.

#### [MODIFY] `crates/flux-script/src/lib.rs`
- Add `state_store: Arc<RwLock<HashMap<String, Dynamic>>>`.
    - Key: A unique string identifier (e.g., `rule_id:key` or just generic keys).
- Register helper functions in `Engine`:
    - `state_get(key: &str) -> Dynamic`: Retrieve value.
    - `state_set(key: &str, value: Dynamic)`: Save value.
    - `now_ms() -> i64`: Return current Unix timestamp (ms).

### 2. Rule Context
The `script_id` (Rule ID) should probably be implicitly used for namespacing state, or we let the user manage keys manually.
- **Decision**: Let's namespace automatically if possible? Or provide `ctx.id`.
- **Simplest approach**: `state_get` takes a global key. User constructs key like `"temp_alert_" + device_id`.

## Verification Plan
1.  **Test Script**: Create a rule that counts events.
    ```rust
    let count = state_get("counter") ?? 0;
    count += 1;
    state_set("counter", count);
    print("Count: " + count);
    ```
2.  **Verify**: Publish multiple messages and check logs for increasing count.
