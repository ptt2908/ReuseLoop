# ReuseLoop - Requirements

## 1. Overview

This document specifies the functional and non-functional requirements and acceptance criteria for the ReuseLoop circular packaging lifecycle application on Nervos CKB.

All requirements reflect the approved Week 10 architecture and implemented contracts:
- Canonical Registry Type Script (`contracts/registry`)
- Canonical State Type Script (`contracts/state`)
- Frontend application and test integration suite (`tests/src/tests.rs`)

## 2. On-Chain Data Model Specifications

The on-chain system consists of two distinct cell types.

### 2.1 Canonical Registry Cell
Maintains the authoritative membership catalog of registered item IDs within its canonical Registry lineage.

- **Type Script Args (98 bytes)**:
  - `[0..1]`: `version = 0x01` (1 byte)
  - `[1..33]`: `registry_id` (32 bytes, Type ID derived at Genesis)
  - `[33..65]`: `admin_lock_hash` (32 bytes)
  - `[65..97]`: `state_code_hash` (32 bytes)
  - `[97..98]`: `state_hash_type` (1 byte, `0x00`, `0x01`, `0x02`, or `0x04`)
- **Cell Data (37 + 32 * item_count bytes)**:
  - `[0..1]`: `schema_version = 0x01` (1 byte)
  - `[1..33]`: `registry_id` (32 bytes)
  - `[33..37]`: `item_count` (`u32` little-endian, 4 bytes)
  - `[37..]`: List of registered 32-byte item IDs (`MAX_ITEMS = 100`)
- **Lock Script**: Locked by `admin_lock_hash`.
- **Type Script**: Canonical Registry Type Script.

### 2.2 Per-Item State Cell
Tracks the individual lifecycle status of one registered packaging item.

- **Type Script Args (97 bytes)**:
  - `[0..1]`: `version = 0x01` (1 byte)
  - `[1..33]`: `item_id` (32 bytes)
  - `[33..65]`: `trusted_registry_type_hash` (32 bytes)
  - `[65..97]`: `admin_lock_hash` (32 bytes)
- **Cell Data (34 bytes)**:
  - `[0..1]`: `schema_version = 0x01` (1 byte)
  - `[1..33]`: `item_id` (32 bytes)
  - `[33..34]`: `status` (1 byte):
    - `0x00`: `AVAILABLE`
    - `0x01`: `BORROWED`
    - `0x02`: `RETURNED`
- **Lock Script**: Locked by `admin_lock_hash`.
- **Type Script**: Canonical State Type Script.

## 3. Functional Requirements

### 3.1 Item Identity and Registry
- **FR-01 Item Identity**: The system shall assign each reusable item a unique 32-byte identifier (`item_id`).
- **FR-02 Item Lookup**: The system shall allow an item to be queried by its 32-byte identifier; QR codes may encode the item lookup link or identifier.
- **FR-03 Registry Genesis (0 -> 1)**: An authorized administrator shall be able to initialize the canonical Registry Cell (`item_count = 0`, data length 37 bytes) via a Genesis transaction enforcing Type ID derivation (`blake2b(Input[0] || output_index)`).
- **FR-04 Atomic Item Registration (Registry 1 -> 1, State 0 -> 1)**: An authorized administrator shall be able to register a new item by atomically updating the Registry Cell and creating the initial State Cell with status `AVAILABLE` (`0x00`).
- **FR-05 Registry Append-Only Invariant**: The Registry Type Script shall ensure that item registration strictly preserves all existing registered item IDs, increments `item_count` by exactly 1, and appends the new `item_id` to the end of the data list.
- **FR-06 Registry Uniqueness and Capacity Limit**: The Registry Type Script shall reject any registration attempt containing a duplicate `item_id` (`ERROR_DUPLICATE_ITEM = 8`) or exceeding the capacity limit of 100 items (`MAX_ITEMS = 100`, `ERROR_CAPACITY_LIMIT = 9`).
- **FR-07 Cryptographic Cell Linkage**: The State Type Script and Registry Type Script shall mutually enforce bidirectional cryptographic linkage:
  - State Type Script Args shall commit to `trusted_registry_type_hash`.
  - Registry Type Script Args shall commit to `state_code_hash` and `state_hash_type`.
  - During Registration Mode (`0 -> 1`), the State Type Script shall verify that the created State Cell matches the item ID appended to the trusted Registry input/output pair.

### 3.2 State Lifecycle Transitions
- **FR-08 Lifecycle Transitions (State 1 -> 1)**: The system shall support three distinct lifecycle transitions, each consuming exactly one State Cell and creating exactly one successor State Cell:
  - **Borrow**: `AVAILABLE (0x00)` -> `BORROWED (0x01)`
  - **Confirm Return**: `BORROWED (0x01)` -> `RETURNED (0x02)`
  - **Make Available**: `RETURNED (0x02)` -> `AVAILABLE (0x00)`
- **FR-09 Persistent RETURNED State**: The `RETURNED` status (`0x02`) shall be an explicit, persistent on-chain state recording the return transition. Cleaning, inspection, and refurbishment are off-chain operational responsibilities and are not verified by the State Type Script. The item may transition to `AVAILABLE` through `Make Available` after the operational process confirms that it is reusable.
- **FR-10 Read-Only Registry Dep Verification**: During lifecycle transitions (`1 -> 1`), the State Type Script shall read the trusted Registry Cell via `CellDep` (without spending it) and verify that the State Cell's `item_id` is present in the registered item list.
- **FR-11 State Script Immutability**: During lifecycle transitions, State Type Script Args (`item_id`, `trusted_registry_type_hash`, `admin_lock_hash`) and State Data `item_id` shall remain strictly immutable.
- **FR-12 Prohibition of Cell Burning**: The State Type Script shall reject any transaction that attempts to consume a State Cell without producing an authorized successor State Cell (`1 -> 0`, `ERROR_MISSING_CELL = 4`).

### 3.3 Authorization and Off-Chain Association
- **FR-13 Administrator Authorization**: All Registry and State cell transactions shall be authorized by the administrator Lock Script matching `admin_lock_hash`. The Admin Lock Script performs cryptographic signature verification (secp256k1), while Type Scripts enforce invariant rules and Lock Script hash equivalence.
- **FR-14 Off-Chain Borrower Association**: Customer IDs, borrower records, or rental agreements shall be maintained strictly in off-chain records or UI state; personal data, customer wallet addresses, and borrower identities shall NOT be written to on-chain cell data or script args.
- **FR-15 Transaction History and Status Display**: The system UI shall query live on-chain cells and transaction history to display current item status and audit logs.
- **FR-16 Error Feedback**: The system shall surface clear error messages when transactions fail or contracts return specific error codes (`1..=10`).

## 4. Non-Functional Requirements

- **NFR-01 Verifiability**: The application shall distinguish chain-verified state (34-byte State Data, 97-byte State Args) from off-chain display data.
- **NFR-02 Privacy**: No personal data, customer names, or customer wallet addresses shall be stored on-chain.
- **NFR-03 Reliability**: Transaction creation, submission, and confirmation shall be tracked as distinct states. The UI shall not report an operation as complete until on-chain confirmation is observed.
- **NFR-04 Maintainability**: On-chain contract logic (`contracts/registry`, `contracts/state`) and off-chain transaction-building services shall be strictly decoupled.
- **NFR-05 Testability**: All valid state transitions, registration flows, and invalid operations shall have automated, repeatable tests.
- **NFR-06 Usability**: The complete item lifecycle workflow shall be demonstrable using an administrator wallet without requiring physical stores or physical hardware sensors.

## 5. Acceptance Criteria

- **AC-01 Registry Genesis**: An administrator submits a Genesis transaction (`0 -> 1`). The Registry contract validates Type ID derivation (Blake2b-256(Input[0].as_slice()[44 bytes] || global_output_index.to_le_bytes()[8 bytes]) using personalization "ckb-default-hash", where global_output_index is the actual 0-based index in Source::Output of the unique output matching the Registry Type Script), verifies `item_count == 0` and length 37 bytes, and outputs the canonical Registry Cell.
- **AC-02 Atomic Item Registration**: An administrator registers a new item (`Registry 1 -> 1`, `State 0 -> 1`). The Registry increments count from `N` to `N + 1` with `new_item_id` appended. Exactly one State Cell is created with status `AVAILABLE` (`0x00`), matching `item_id`, and locked by `admin_lock_hash`. Both scripts return `0`.
- **AC-03 Duplicate Item Rejection**: Attempting to register an item ID that already exists in the Registry fails with `ERROR_DUPLICATE_ITEM (8)`.
- **AC-04 Registry Capacity Limit**: Attempting to register an item when `item_count == 100` fails with `ERROR_CAPACITY_LIMIT (9)`.
- **AC-05 Non-Atomic Registration Rejection**: Attempting to create a State Cell without the corresponding Registry update, or attempting to update the Registry without the canonical State output, fails with `ERROR_MISSING_CELL (4)` or `ERROR_UNTRUSTED_REGISTRY (10)`.
- **AC-06 State Borrow Transition**: Consuming an `AVAILABLE` State Cell and creating a `BORROWED` State Cell (`1 -> 1`) with trusted Registry in `CellDep` succeeds.
- **AC-07 State Return Transition**: Consuming a `BORROWED` State Cell and creating a `RETURNED` State Cell (`1 -> 1`) with trusted Registry in `CellDep` succeeds.
- **AC-08 State Make Available Transition**: Consuming a `RETURNED` State Cell and creating an `AVAILABLE` State Cell (`1 -> 1`) with trusted Registry in `CellDep` succeeds.
- **AC-09 Invalid Transition Rejection**: Attempting an out-of-order transition (e.g. `AVAILABLE` -> `RETURNED`, `BORROWED` -> `AVAILABLE`, or `RETURNED` -> `BORROWED`) fails with `ERROR_INVALID_TRANSITION (3)`.
- **AC-10 Unregistered Item Lifecycle Rejection**: Attempting a lifecycle transition for a State Cell whose `item_id` is not present in the Registry `CellDep` fails with `ERROR_UNTRUSTED_REGISTRY (10)`.
- **AC-11 State Cell Burning Rejection**: Attempting to consume a State Cell without producing an authorized output State Cell (`1 -> 0`) fails with `ERROR_MISSING_CELL (4)`.
- **AC-12 Multiple State Cells Rejection**: Attempting a transaction with multiple State inputs or outputs fails with `ERROR_MULTIPLE_CELLS (5)`.
- **AC-13 Unauthorized Lock Rejection**: Transactions using a Registry or State Lock Script hash that does not match the configured admin_lock_hash are rejected by the Type Script with ERROR_UNAUTHORIZED (6). Transactions with missing or invalid administrator signatures fail verification at the Admin Lock Script level.
- **AC-14 Capacity Validation**: The Registry Genesis output and newly created State outputs during Registration must satisfy occupied capacity. During Registry Registration (1 -> 1), the Registry output must satisfy occupied capacity and must not have less capacity than the Registry input. During State Lifecycle (1 -> 1), the State output must satisfy occupied capacity and must not have less capacity than the State input. Violations fail with ERROR_CAPACITY (7). State Registration (0 -> 1) has no State input capacity comparison.
- **AC-15 UI Confirmation and Lookup**: The frontend looks up items by ID, displays real-time state (`AVAILABLE`, `BORROWED`, `RETURNED`), and awaits on-chain block confirmation before updating status.

## 6. Architectural Decisions and Scope Resolution

### 6.1 Resolved Week 10 Decisions

| Original Question | Resolution in Implemented Architecture |
|---|---|
| **Persistence of `RETURNED` state** | Resolved: `RETURNED` is an explicit, persistent on-chain state (`0x02`) requiring an explicit `Make Available` transition to return to `AVAILABLE` (`0x00`). |
| **Authorization mechanism** | Resolved for MVP: Centralized administrator lock (`admin_lock_hash`). The Admin Lock Script verifies cryptographic signatures; Type Scripts verify lock hashes and invariants. |
| **Custom Script vs standard prototype** | Resolved: Custom Rust contracts implemented on CKB RISC-V (`contracts/registry` and `contracts/state`) with full Molecule and linkage validation. |
| **On-chain data encoding** | Resolved: Fixed byte-array schema without dynamic serialization overhead. State Data is 34 bytes; State Args is 97 bytes; Registry Args is 98 bytes; Registry Data is `37 + 32 * item_count` bytes. |
| **Borrower association** | Resolved: Borrower identity is maintained off-chain. No personal data or borrower addresses are stored in on-chain cell data or script args. |
| **Registry scaling model** | Resolved for MVP: Canonical Registry lineage tracking up to 100 items (MAX_ITEMS = 100) in the Week 10 MVP. Read-only `CellDep` inclusion allows parallel item lifecycle transitions without contention. |

### 6.2 Future Scope (Out of Scope for Week 10 MVP)

The following items are deferred to future development phases and are not required for the Week 10 MVP:
- **Decentralized Multi-Party Authorization**: Allowing distinct store or customer lock scripts to independently authorize returns or borrows on-chain.
- **On-Chain Customer Signatures**: Direct cryptographic verification of borrower identity on-chain.
- **Registry Sharding**: Multi-cell or partitioned registry structures to scale beyond 100 items.
- **Payment & Deposit Protocols**: Fiber network integration or escrow deposits.
- **Tokenized Packaging**: Spore protocol or DOB digital object representation.
- **Hardware IoT Integration**: Automated smart-bin RFID/optical sensors directly triggering CKB transactions.
