# ReuseLoop Architecture

## 1. Overview

ReuseLoop is a circular economy packaging lifecycle application built on Nervos CKB.
It provides an auditable, verifiable on-chain record for reusable packaging items
as they move through cycles of availability, borrowing, return, and refurbishment.

The architecture uses a two-tier on-chain cell model:
1. **Registry Cell**: Authoritative record of registered item IDs within its canonical Registry lineage.
2. **State Cell**: Per-item cell tracking the current lifecycle status of one registered item.

## 2. System Architecture and Components

### Frontend
- React, TypeScript, and Vite.
- CCC (Common Chain Connector) SDK for CKB wallet interaction.
- Displays item state, transaction history, and QR-based item lookup.
- Constructs transactions and prompts the administrator wallet for signatures.

### Administrator Wallet
- Authorizes and signs on-chain transactions using the administrator Lock Script (e.g., standard secp256k1_blake160).
- The MVP uses an administrator-controlled authorization model (`admin_lock_hash`) where Registry updates, item registrations, and lifecycle transitions are authorized by the administrator.
- Private keys and recovery credentials remain exclusively in the wallet and are never stored in frontend source code.

### CKB Layer 1
- **Registry Cell**: Maintains the authorized catalog of registered item IDs in an append-only structure governed by Type ID lineage.
- **State Cells**: Maintain the current lifecycle state of individual items on-chain.
- **On-Chain Scripts (Type Scripts)**: Custom RISC-V contracts enforcing business rules, cryptographic linkage, and valid state transitions.
- **Lock Scripts**: Verify cryptographic signatures to ensure only authorized entities can spend cells.

### Off-Chain Data and Services
- **Item Metadata**: Item display names, physical dimensions, material descriptions, packaging images, and store or depot location information are stored off-chain.
- **Borrower Association**: Customer IDs, rental receipts, or temporary loan associations are maintained off-chain for privacy and simplicity; personal data is not written to CKB cells.
- **Indexers and RPC**: CKB RPC or indexers are queried to fetch current cells and transaction history for the UI.
- **Off-Chain vs On-Chain Boundaries**: Off-chain data enriches user presentation but never replaces or overrides on-chain Type Script validation.

## 3. On-Chain Cell Model

### 3.1 Registry Cell

The Registry Cell establishes an append-only record of registered item identifiers within a canonical Registry lineage.

- **Type Script Args (98 bytes)**:
  - `[0..1]`: `version = 0x01` (1 byte)
  - `[1..33]`: `registry_id` (32 bytes, Type ID derived at Genesis)
  - `[33..65]`: `admin_lock_hash` (32 bytes)
  - `[65..97]`: `state_code_hash` (32 bytes)
  - `[97..98]`: `state_hash_type` (1 byte, `0x00`, `0x01`, `0x02`, or `0x04`)
  See [registry-design.md](registry-design.md) (Section 12) for details.
  Type ID derivation ties the lineage to the specific Genesis seed input and global output index; it establishes the unique identity for that lineage, but does not claim a global singleton across all possible seeds.
- **Cell Data (37 + 32 * item_count bytes)**:
  - `[0..1]`: `schema_version = 0x01` (1 byte)
  - `[1..33]`: `registry_id` (32 bytes)
  - `[33..37]`: `item_count` (`u32` little-endian, 4 bytes)
  - `[37..]`: List of registered 32-byte item IDs (`MAX_ITEMS = 100`).
  See [registry-design.md](registry-design.md) (Section 10) for details.
- **Lock Script**: Locked by the administrator Lock Script (`admin_lock_hash`).
- **Type Script**: Canonical Registry Type Script enforcing Type ID genesis, append-only uniqueness, and registration linkage.

### 3.2 State Cell

Each registered packaging item is represented on-chain by exactly one State Cell tracking its real-time lifecycle status.

- **Type Script Args (97 bytes)**:
  - `[0..1]`: `version = 0x01` (1 byte)
  - `[1..33]`: `item_id` (32 bytes)
  - `[33..65]`: `trusted_registry_type_hash` (32 bytes)
  - `[65..97]`: `admin_lock_hash` (32 bytes)
  See [cell-linkage-design.md](cell-linkage-design.md) (Section 3) for details.
- **Cell Data (34 bytes)**:
  - `[0..1]`: `schema_version = 0x01` (1 byte)
  - `[1..33]`: `item_id` (32 bytes)
  - `[33..34]`: `status` (1 byte):
    - `0x00`: `AVAILABLE`
    - `0x01`: `BORROWED`
    - `0x02`: `RETURNED`
  See [cell-linkage-design.md](cell-linkage-design.md) (Section 4) for details.
- **Lock Script**: Locked by the administrator Lock Script (`admin_lock_hash`).
- **Type Script**: Canonical State Type Script validating lifecycle transitions and registration linkage.

## 4. Cryptographic Linkage and Execution Modes

State Cells and Registry Cells maintain bidirectional cryptographic linkage:
- **State -> Registry**: State Type Script Args includes `trusted_registry_type_hash`, which must match the Molecule-serialized Type Script hash of the canonical Registry.
- **Registry -> State**: Registry Type Script Args includes `state_code_hash` and `state_hash_type`, allowing the Registry script to reconstruct and verify the expected State Type Script.

The State Type Script operates in two distinct execution modes based on cell group cardinality:
1. **Registration Mode (`0 GroupInput -> 1 GroupOutput`)**:
   - Executes during Atomic Item Registration alongside a `1 -> 1` Registry update.
   - Validates that the newly created State Cell matches the item ID appended to the trusted Registry.
2. **Lifecycle Mode (`1 GroupInput -> 1 GroupOutput`)**:
   - Executes during state transitions (Borrow, ConfirmReturn, MakeAvailable).
   - Validates that the item exists in the trusted Registry Cell provided as a read-only `CellDep`.

The State Type Script rejects unsupported group combinations that cause the script to execute, including State Cell burning (1 -> 0) and multiple State inputs or outputs. The 0 -> 0 combination does not invoke the State Type Script group and is therefore not directly rejected by this script.

## 5. Transaction Flows

### 5.1 Registry Genesis (0 -> 1)

Initializes the canonical Registry Cell before any items can be registered.

- **Inputs**:
  - `Input[0]`: Seed cell owned by admin (`admin_lock_hash`), consumed for Type ID derivation.
  - Optional funding inputs.
- **Outputs**:
  - Registry Output: Empty registry (`item_count = 0`, data length 37 bytes), locked by `admin_lock_hash`.
  - Optional change cell.
- **CellDeps**: Code dependencies for Registry contract and Lock Script.
- **Validation**: Enforces Type ID derivation (`blake2b(Input[0] || output_index)`), `item_count == 0`, and admin authorization.

### 5.2 Atomic Item Registration (Registry 1 -> 1, State 0 -> 1)

Registers a new packaging item by appending its `item_id` to the Registry and creating its initial State Cell atomically in the same transaction.

- **Inputs**:
  - `Registry GroupInput[0]`: Current Registry Cell (`item_count = N`), locked by `admin_lock_hash`.
  - Optional additional funding inputs, as needed.
- **Outputs**:
  - `Registry GroupOutput[0]`: Updated Registry Cell (`item_count = N + 1`), preserving existing entries and appending `new_item_id`.
  - `State GroupOutput[0]`: Initial State Cell (`status = AVAILABLE`, data length 34 bytes, locked by `admin_lock_hash`).
  - Optional change cell.
- **CellDeps**: Code dependencies for Registry, State, and Lock scripts.
- **Why Atomic Registration is Required**:
  - **Prevents Orphaned State Cells**: No State Cell can be created without being recorded in the canonical Registry.
  - **Prevents Orphaned Registry Entries**: No item ID can be added to the Registry without its corresponding on-chain State Cell initialized to `AVAILABLE`.
  - **Ensures Atomic Consensus**: Both contracts validate the shared transaction and verify each other's outputs.

### 5.3 Item Lifecycle Transitions (State 1 -> 1)

Lifecycle transitions update the status of an existing registered packaging item. The transaction consumes the current State Cell and produces an updated successor State Cell.

- **Permitted Transitions**:
  - **Borrow**: `AVAILABLE (0x00)` -> `BORROWED (0x01)`
  - **Confirm Return**: `BORROWED (0x01)` -> `RETURNED (0x02)`
  - **Make Available**: `RETURNED (0x02)` -> `AVAILABLE (0x00)`
- **Transaction Structure**:
  - **Inputs**: Exactly 1 State Cell (`GroupInput[0]`) in the prerequisite status, locked by `admin_lock_hash`.
  - **Outputs**: Exactly 1 State Cell (`GroupOutput[0]`) in the target status, locked by `admin_lock_hash`.
  - **CellDeps**:
    - The canonical Registry Cell included as a read-only `CellDep`.
    - Code dependencies for State contract and Lock Script.
- **Validation Rules**:
  - State Type Script validates status progression according to the lifecycle state machine.
  - State Type Script Args (`item_id`, `trusted_registry_type_hash`, `admin_lock_hash`) and Data `item_id` remain strictly immutable.
  - The State contract reads the Registry `CellDep` and verifies that `item_id` is present in the registered item list.
  - The Registry Cell is not spent, avoiding contention and allowing parallel lifecycle transitions for different items.

## 6. Authorization Model

The ReuseLoop architecture strictly separates script responsibilities:

- **Lock Scripts (Signature Verification)**:
  - Cryptographic signature verification (e.g., secp256k1) is executed solely by CKB Lock Scripts.
  - In the MVP architecture, all Registry and State Cells are locked by the administrator Lock Script (`admin_lock_hash`).
  - Transactions must provide valid cryptographic witness signatures satisfying the administrator Lock Script.
- **Type Scripts (Invariant Validation)**:
  - Type Scripts validate structural formats, byte lengths, data schemas, state transition logic, cryptographic hashes, and cell linkage.
  - Type Scripts verify that the cell Lock Script matches the configured `admin_lock_hash`, ensuring that transitions cannot bypass administrative control.
  - Type Scripts do NOT perform cryptographic signature checks directly.

## 7. Off-Chain vs On-Chain Responsibilities and Boundaries

- **On-Chain Guarantees**:
  - Uniqueness and membership of registered item IDs within the canonical Registry lineage.
  - Immutability of item identity and administrative ownership.
  - Strict enforcement of valid lifecycle state transitions.
  - Protection against unauthorized cell modification or burning.
  - Capacity preservation and CKB consensus compliance.
- **Off-Chain Responsibilities**:
  - Storage of rich metadata (item names, descriptions, images, serial numbers).
  - Recording user or borrower associations and store or depot location logs.
  - Indexing transaction history for frontend queries and reporting.
  - Generation and scanning of QR codes encoding item IDs.
- **Physical-World Limitation**:
  - The blockchain provides an immutable, authorized audit trail of statements signed by the administrator.
  - The blockchain cannot independently verify physical reality (e.g., whether a physical container was actually handed over, cleaned, or physically damaged). Operational verification remains the responsibility of the physical operating protocol.

## 8. MVP Scope Boundaries

### Included in Week 10 MVP:
- Canonical Registry Cell with Type ID genesis and append-only registration up to 100 items (`MAX_ITEMS = 100`).
- State Cell representing individual packaging items.
- Atomic Item Registration (`Registry 1 -> 1` and `State 0 -> 1`).
- Implemented lifecycle transitions: `AVAILABLE` -> `BORROWED` -> `RETURNED` -> `AVAILABLE`.
- Read-only Registry `CellDep` validation during lifecycle transitions.
- Administrator authorization model (`admin_lock_hash`).
- Frontend interface for querying state, initiating transitions, and viewing history.
- Comprehensive integration test suite covering positive flows and edge-case errors.

### Out of Scope for MVP:
- Multi-party decentralized authorization (e.g., distinct customer or store lock scripts).
- Direct on-chain verification of borrower identity or customer signatures.
- Partitioned multi-cell registry sharding.
- Spore or DOB protocol integration.
- Fiber network payments or deposit handling.
- Physical IoT automated sensor verification.
