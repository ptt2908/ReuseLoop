# ReuseLoop Transaction Design

## 1. Purpose

Define the transaction structures, capacity requirements, authorization
mechanisms, and on-chain validation rules for the ReuseLoop item
lifecycle on CKB.

This document specifies how CKB transactions are constructed for:
- Registry Genesis (initial Registry creation)
- Atomic Item Registration (Registry update and initial State creation)
- Item Lifecycle transitions (Borrow, ConfirmReturn, MakeAvailable)

Statements in this document describe implemented and tested behavior
unless explicitly marked as a future consideration.

## 2. On-Chain Cell Model

ReuseLoop uses two distinct cell types on CKB:
1. **Registry Cell**: Authoritative record of registered item IDs within its canonical Registry lineage.
2. **State Cell**: Per-item cell tracking the current lifecycle status of one registered item.

### 2.1 State Cell

Each registered packaging item is represented on-chain by exactly one
State Cell:

- **Type Script Args (97 bytes)**:
  - `[0..1]`: `version = 0x01`
  - `[1..33]`: `item_id` (32 bytes)
  - `[33..65]`: `trusted_registry_type_hash` (32 bytes)
  - `[65..97]`: `admin_lock_hash` (32 bytes)
  See [cell-linkage-design.md](cell-linkage-design.md) (Section 3) for details.
- **Cell Data (34 bytes)**:
  - `[0..1]`: `schema_version = 0x01` (1 byte)
  - `[1..33]`: `item_id` (32 bytes)
  - `[33..34]`: `status` (1 byte)
    - `0x00`: `AVAILABLE`
    - `0x01`: `BORROWED`
    - `0x02`: `RETURNED`
- **Lock Script**: Locked by the administrator Lock Script (`admin_lock_hash`).
- **Type Script**: Canonical State Type Script validating lifecycle transitions and registration linkage.

### 2.2 Registry Cell

A canonical Registry Cell lineage (instantiated at Genesis and updated through
successive `1 -> 1` transactions) tracks all registered items:

- **Type Script Args (98 bytes)**:
  - `[0..1]`: `version = 0x01`
  - `[1..33]`: `registry_id` (32 bytes, Type ID derived at Genesis)
  - `[33..65]`: `admin_lock_hash` (32 bytes)
  - `[65..97]`: `state_code_hash` (32 bytes)
  - `[97..98]`: `state_hash_type` (1 byte, `0x00`, `0x01`, `0x02`, or `0x04`)
  See [registry-design.md](registry-design.md) (Section 12) for details.
  Type ID derivation ties the lineage to the specific Genesis seed input and
  global output index; it establishes the unique identity for that lineage, but
  does not claim a global singleton across all possible seeds.
- **Cell Data (`37 + 32 * item_count` bytes)**:
  - `[0..1]`: `schema_version = 0x01` (1 byte)
  - `[1..33]`: `registry_id` (32 bytes)
  - `[33..37]`: `item_count` (`u32` little-endian, 4 bytes)
  - `[37..]`: List of registered 32-byte item IDs (`MAX_ITEMS = 100`).
  See [registry-design.md](registry-design.md) (Section 10) for details.
- **Lock Script**: Locked by the administrator Lock Script (`admin_lock_hash`).
- **Type Script**: Canonical Registry Type Script enforcing Type ID genesis, append-only uniqueness, and registration linkage.

### 2.3 Capacity Concepts and Distinctions

On-chain capacity validation strictly separates three distinct concepts:

- **Actual Cell Data Size**:
  The raw byte length of the cell data payload (exactly 34 bytes for State Cells;
  `37 + 32 * item_count` bytes for the Registry Cell).
- **Occupied Capacity**:
  The minimum capacity in Shannons required by CKB consensus to store an individual
  cell on-chain. Calculated via the `load_cell_occupied_capacity` syscall based on:
  `8 bytes (capacity field) + serialized Lock Script bytes + serialized Type Script bytes (if present) + Cell Data bytes`.
  Every created or updated output cell must satisfy:
  `load_cell_capacity >= load_cell_occupied_capacity`.
- **Molecule Serialized Size**:
  The encoded wire size of the full transaction structure (inputs, outputs, cell deps,
  witnesses) transmitted across RPC or network boundaries.
  Serialized transaction size is not used to calculate a cell's occupied capacity,
  and on-chain scripts do not infer capacity requirements from transaction serialization.
  Capacity invariants are enforced directly on each individual cell's capacity field
  against its own occupied capacity and preceding input capacity.

## 3. Registry Genesis Transaction (0 -> 1)

The Registry Genesis transaction initializes the canonical Registry Cell
before any items can be registered.

### 3.1 Transaction Structure

| Component | Source / Count | Description |
|---|---|---|
| **Inputs** | `Source::Input[0]` | Seed cell locked by `admin_lock_hash`. Consumed to establish the unique Type ID. |
| **Outputs** | Unique Registry Output | The unique output matching the Registry Type Script (with `item_count = 0`). Its actual global output index in `Source::Output` is used in the Type ID derivation. |
| **CellDeps** | Code CellDeps | Contract and Lock Script code dependencies (e.g. Registry Type Script and admin Lock Script). No Registry Cell or State Cell data dependency is required. |
| **Witnesses** | `Witnesses[0]` | Valid signature over `Input[0]` satisfying the administrator Lock Script. |

### 3.2 Cardinality and Invariants

- **Registry Script Group**:
  Exactly 0 `GroupInput` cells and exactly 1 `GroupOutput` cell.
  Missing output returns `ERROR_MISSING_CELL (4)`; multiple inputs or outputs
  return `ERROR_MULTIPLE_CELLS (5)`.
- **Administrator Authorization**:
  - `Input[0]` Lock Script hash must equal `admin_lock_hash` (`ERROR_UNAUTHORIZED (6)`).
  - Registry `GroupOutput[0]` Lock Script hash must equal `admin_lock_hash` (`ERROR_UNAUTHORIZED (6)`).
- **Type ID Derivation**:
  The Registry Type Script calculates:
  `registry_id = Blake2b-256(Input[0].as_slice()[44B] || global_output_index.to_le_bytes()[8B])`
  using personalization `"ckb-default-hash"`, where `global_output_index` is the
  0-based index of the unique output matching the Registry Type Script in `Source::Output`.
  The calculated hash must match Registry Args `registry_id` (`ERROR_INVALID_ARGS (1)`).
  Multiple matching Registry outputs during the Type ID check return `ERROR_MULTIPLE_CELLS (5)`.
- **Initial Data**:
  Output Data must be exactly 37 bytes with `schema_version = 0x01`,
  `registry_id` matching Args, and `item_count = 0` (`ERROR_INVALID_DATA (2)`).
- **State Cell Isolation**:
  Zero canonical State cells are permitted in transaction Inputs or Outputs
  (`ERROR_INVALID_TRANSITION (3)`).
- **Occupied Capacity**:
  Output capacity must satisfy occupied capacity
  (`load_cell_capacity >= load_cell_occupied_capacity`, otherwise `ERROR_CAPACITY (7)`).

## 4. Atomic Item Registration Transaction

Registering a new packaging item is an atomic operation that transitions the
Registry (`1 -> 1`) and creates the initial State Cell (`0 -> 1`) in a single
transaction.

### 4.1 Transaction Structure

| Component | Count / Index | Description |
|---|---|---|
| **Inputs** | 1 (or more) | Current Registry Cell (`GroupInput[0]`), plus any additional admin-locked fee/capacity inputs. |
| **Outputs** | Output A | Successor Registry Cell (`GroupOutput[0]`) with appended item ID (`item_count + 1`). |
| | Output B | New canonical State Cell with status `AVAILABLE (0x00)` for the registered item. |
| **CellDeps** | As needed | Code cell dependencies for contracts and Lock Script. Registry Cell is an input, NOT a CellDep. |
| **Witnesses** | As needed | Valid administrator signature for all inputs locked by `admin_lock_hash`. |

### 4.2 Script Cardinality

- **Registry Group**: Exactly 1 `GroupInput` and 1 `GroupOutput` (`1 -> 1`).
- **State Group**: Exactly 0 `GroupInputs` and 1 `GroupOutput` (`0 -> 1`).

### 4.3 Validation Responsibilities

Both the Registry Type Script and State Type Script execute and must accept the
transaction. If either script rejects, the transaction fails atomically.

#### Checks Performed by Registry Type Script (`1 -> 1`)
- GroupInput and GroupOutput locks match `admin_lock_hash` (`ERROR_UNAUTHORIZED (6)`).
- Registry capacity cannot decrease: `out_cap >= in_cap` and `out_cap >= out_occ` (`ERROR_CAPACITY (7)`).
- Input Registry Data has no duplicate item IDs (`ERROR_INVALID_DATA (2)`).
- Output Data appends exactly one item ID: `out_count == in_count + 1` (`ERROR_INVALID_TRANSITION (3)`).
- Existing item ID prefix is preserved in order (`ERROR_INVALID_TRANSITION (3)`).
- Newly appended item ID does not already exist in input Registry Data (`ERROR_DUPLICATE_ITEM (8)`).
- Registered item count does not exceed `MAX_ITEMS = 100` (`ERROR_CAPACITY_LIMIT (9)`).
- No canonical State cell in transaction Inputs (`ERROR_INVALID_TRANSITION (3)`).
- Exactly one canonical State cell in transaction Outputs (`0 -> ERROR_MISSING_CELL (4)`, `>1 -> ERROR_MULTIPLE_CELLS (5)`).
- Canonical State output validation:
  - Script identity matches Registry Args (`state_code_hash`, `state_hash_type`).
  - State Args: 97 bytes, `version = 0x01`, `item_id == new_item_id` (`ERROR_INVALID_ARGS (1)`).
  - State Args `trusted_registry_type_hash` matches Registry's own script hash (`ERROR_UNTRUSTED_REGISTRY (10)`).
  - State Args `admin_lock_hash` matches Registry's `admin_lock_hash` (`ERROR_UNAUTHORIZED (6)`).
  - State output Lock hash matches `admin_lock_hash` (`ERROR_UNAUTHORIZED (6)`).
  - State output capacity satisfies occupied capacity (`ERROR_CAPACITY (7)`).
  - State output Data: 34 bytes, `schema_version = 0x01`, `item_id == new_item_id` (`ERROR_INVALID_DATA (2)`).
  - State initial status is `AVAILABLE (0x00)` (`ERROR_INVALID_TRANSITION (3)`).

#### Checks Performed by State Type Script (`0 -> 1`, Registration Mode)
- State output Lock hash matches `admin_lock_hash` (`ERROR_UNAUTHORIZED (6)`).
- At least one transaction Input is locked by `admin_lock_hash` (`ERROR_UNAUTHORIZED (6)`).
- State output capacity satisfies occupied capacity (`ERROR_CAPACITY (7)`).
- State output Data: 34 bytes, `schema_version = 0x01`, `item_id == args.item_id`, valid status (`ERROR_INVALID_DATA (2)`).
- State initial status is `AVAILABLE (0x00)` (`ERROR_INVALID_TRANSITION (3)`).
- Exactly one trusted Registry input and one trusted Registry output exist (`0 -> ERROR_UNTRUSTED_REGISTRY (10)`, `>1 -> ERROR_MULTIPLE_CELLS (5)`).
- Registry Input and Output locks match `admin_lock_hash` (`ERROR_UNAUTHORIZED (6)`).
- Registry Input and Output Type Scripts are identical (`ERROR_UNTRUSTED_REGISTRY (10)`).
- Registry Args: 98 bytes, `version = 0x01` (`ERROR_INVALID_ARGS (1)`).
- Registry Args `admin_lock_hash` matches State `admin_lock_hash` (`ERROR_UNAUTHORIZED (6)`).
- State script identity matches Registry Args `state_code_hash` and `state_hash_type` (`ERROR_UNTRUSTED_REGISTRY (10)`).
- Exactly one canonical State output in transaction (`>1 -> ERROR_MULTIPLE_CELLS (5)`).
- Registry Data header validity and item count limit (`ERROR_INVALID_DATA (2)`, `ERROR_CAPACITY_LIMIT (9)`).
- Registry Input Data has no duplicates (`ERROR_INVALID_DATA (2)`).
- Registry output count equals input count + 1 (`ERROR_INVALID_TRANSITION (3)`).
- Registry output preserves existing prefix (`ERROR_INVALID_TRANSITION (3)`).
- Newly appended item ID does not already exist in Registry input (`ERROR_DUPLICATE_ITEM (8)`).
- Newly appended item ID matches State Args `item_id` (`ERROR_UNTRUSTED_REGISTRY (10)`).

## 5. Lifecycle Transactions

Lifecycle transactions record changes in the packaging item status without
modifying the Registry.

### 5.1 Permitted Transitions

Only three status transitions are valid:

1. **Borrow**:
   - Input State: `AVAILABLE (0x00)`
   - Output State: `BORROWED (0x01)`
2. **ConfirmReturn**:
   - Input State: `BORROWED (0x01)`
   - Output State: `RETURNED (0x02)`
3. **MakeAvailable**:
   - Input State: `RETURNED (0x02)`
   - Output State: `AVAILABLE (0x00)`

Any other transition (e.g., `AVAILABLE -> RETURNED`, `AVAILABLE -> AVAILABLE`,
`BORROWED -> AVAILABLE`, `RETURNED -> BORROWED`) is rejected with
`ERROR_INVALID_TRANSITION (3)`.

### 5.2 Transaction Structure

| Component | Source / Count | Description |
|---|---|---|
| **Inputs** | 1 State Cell | Current State Cell (`GroupInput[0]`) in preceding status. |
| **Outputs** | 1 State Cell | Successor State Cell (`GroupOutput[0]`) in successor status. |
| **CellDeps** | 1 Registry Cell | Trusted Registry Cell containing `item_id` in its Data. |
| **Witnesses** | As needed | Valid administrator signature for inputs locked by `admin_lock_hash`. |

### 5.3 Cardinality and Invariants

- **State Script Group**:
  Exactly 1 `GroupInput` and 1 `GroupOutput` (`1 -> 1`).
  Burning a State Cell (`1 -> 0`) is rejected with `ERROR_MISSING_CELL (4)`.
  Multiple State inputs or outputs are rejected with `ERROR_MULTIPLE_CELLS (5)`.
- **Registry Read-Only CellDep**:
  - Exactly 1 cell in `Source::CellDep` must match `trusted_registry_type_hash`
    (`0` or `>1` returns `ERROR_UNTRUSTED_REGISTRY (10)`).
  - The trusted Registry must NOT appear in `Source::Input` or `Source::Output`
    (`ERROR_UNTRUSTED_REGISTRY (10)`).
  - Registry CellDep Lock hash must equal `admin_lock_hash` (`ERROR_UNAUTHORIZED (6)`).
  - Registry CellDep Args and Data are validated (`ERROR_INVALID_ARGS (1)`, `ERROR_INVALID_DATA (2)`).
  - The State `item_id` must be present in the Registry CellDep item list
    (`ERROR_UNTRUSTED_REGISTRY (10)`).
- **Identity Preservation**:
  - `item_id` in State Data and State Args must remain identical.
  - State Type Script and Lock Script identities are preserved.
- **Capacity Preservation**:
  - State output capacity must satisfy occupied capacity: `out_cap >= occupied_capacity` (`ERROR_CAPACITY (7)`).
  - State output capacity cannot decrease: `out_cap >= in_cap` (`ERROR_CAPACITY (7)`).

## 6. Transaction Authorization and Workflow Distinction

### 6.1 Cryptographic Signature vs. Type Script Invariant Checks

ReuseLoop strictly separates cryptographic signature verification from Type Script
validation:

- **Type Scripts** (`registry` and `state`):
  Do not verify ECDSA signatures or inspect cryptographic witnesses directly.
  They verify that cells use the expected Lock Script hash (`admin_lock_hash`),
  that required admin-locked inputs are present, and that structural/business
  invariants hold across inputs, outputs, and cell deps.
- **Administrator Lock Script**:
  Guards inputs locked by `admin_lock_hash` (in integration tests, CKB's official
  `secp256k1_blake160_sighash_all` script). When the transaction executes, the
  Lock Script validates the recoverable ECDSA signature in `WitnessArgs.lock`
  against the transaction sighash digest.
- If the signature is missing or invalid, transaction verification fails at the
  Lock Script level before Type Scripts can succeed.

### 6.2 Application Workflow vs. On-Chain Item State

It is essential to distinguish off-chain business workflow states from on-chain
cell status:

- **Off-Chain Workflow Evidence**:
  Physical handoff scans, borrower confirmations, store receipt acknowledgments,
  and customer QR scans are application-level workflow events.
- **On-Chain Item Status**:
  The authoritative on-chain state exists solely in the 34-byte State Cell Data
  (`AVAILABLE`, `BORROWED`, `RETURNED`).
- **UI State Update Rule**:
  The frontend and off-chain indexing services must never report or display a
  transition as completed before the corresponding transaction is confirmed on
  the CKB blockchain.
- **Physical-World Limitation**:
  Blockchain confirmation proves that an authorized transaction satisfies protocol
  rules. It cannot physically guarantee that a package was handed over or returned
  in the real world.

## 7. Validation Invariants Summary

Every ReuseLoop transaction must satisfy the following core invariants:

1. **Schema Integrity**:
   State Args are 97 bytes (`version = 0x01`); State Data is 34 bytes (`schema_version = 0x01`).
   Registry Args are 98 bytes (`version = 0x01`); Registry Data is `37 + 32 * item_count` bytes (`schema_version = 0x01`).
2. **Identity Stability**:
   Registry identity (`registry_id` and complete Type Script hash) is immutable.
   State identity (`item_id` and complete Type Script hash) is immutable.
3. **Cardinality Control**:
   - Genesis: Registry `0 -> 1`, State `0 -> 0`.
   - Registration: Registry `1 -> 1`, State `0 -> 1`.
   - Lifecycle: State `1 -> 1`, Registry `CellDep (1)`.
   State burn (`1 -> 0`) and Registry burn (`1 -> 0`) are prohibited.
4. **Capacity Soundness**:
   Every created output cell satisfies `capacity >= occupied_capacity`.
   Every updated cell preserves capacity: `out_cap >= in_cap`.
5. **Item Uniqueness**:
   An item ID can only be registered once in the append-only Registry.
   Unregistered item IDs cannot participate in lifecycle transitions.

## 8. Contract Error Code Reference

Both contracts return `0` on success and use the following error codes:

| Code | Constant | Meaning in Registry Type Script | Meaning in State Type Script |
|---:|---|---|---|
| `1` | `ERROR_INVALID_ARGS` | Registry Args len != 98, version != 0x01, or invalid hash_type; Genesis Type ID mismatch; State output args len != 97, version != 0x01, or item_id mismatch. | State Args len != 97 or version != 0x01; Registry Args len != 98 or version != 0x01. |
| `2` | `ERROR_INVALID_DATA` | Registry Data len < 37 or len != 37 + 32*N; schema != 0x01; registry_id mismatch; Genesis count != 0; duplicate IDs in input Data; State output Data len != 34, schema != 0x01, item_id mismatch, or invalid status byte. | State Data len != 34, schema != 0x01, item_id mismatch, or invalid status byte; Registry Data header invalid; duplicate IDs in Registry input Data (Registration Mode). |
| `3` | `ERROR_INVALID_TRANSITION` | Canonical State cell in Input; canonical State cell in Output during Genesis; Registration out_count != in_count + 1; modified existing prefix; State output status != AVAILABLE. | Status transition not permitted (Borrow, ConfirmReturn, MakeAvailable); initial registration status != AVAILABLE; Registry output count != in_count + 1; modified Registry prefix. |
| `4` | `ERROR_MISSING_CELL` | Missing Registry GroupOutput (burn attempt); missing canonical State output during Registration. | Missing State GroupOutput (burn attempt); missing cell syscall failure. |
| `5` | `ERROR_MULTIPLE_CELLS` | More than 1 Registry GroupInput or GroupOutput; multiple matching outputs in Genesis Type ID check; multiple canonical State outputs in Registration. | More than 1 State GroupInput or GroupOutput; more than 1 matching Registry input or output (Registration Mode); more than 1 canonical State output. |
| `6` | `ERROR_UNAUTHORIZED` | GroupInput, GroupOutput, or Genesis Input[0] lock != admin_lock_hash; State output lock or args admin_lock_hash != admin_lock_hash. | State Input or Output lock != admin_lock_hash; no admin-locked input in transaction (Registration Mode); Registry lock != admin_lock_hash; Registry args admin_lock_hash mismatch. |
| `7` | `ERROR_CAPACITY` | Output capacity < occupied capacity (Genesis and Registration); Registration GroupOutput capacity < GroupInput capacity; canonical State output capacity < occupied capacity. | State output capacity < occupied capacity (Registration and Lifecycle); Lifecycle output capacity < State input capacity. |
| `8` | `ERROR_DUPLICATE_ITEM` | Appended item ID already exists in input Registry Data; duplicate IDs in Genesis output Data. | Appended item ID already exists in Registry input Data (Registration Mode); duplicate IDs in Registry CellDep Data (Lifecycle Mode). |
| `9` | `ERROR_CAPACITY_LIMIT` | Registry item_count > 100 (`MAX_ITEMS`). | Registry item_count > 100 (`MAX_ITEMS`). |
| `10` | `ERROR_UNTRUSTED_REGISTRY` | Canonical State output trusted_registry_type_hash != Registry's own script hash. | Missing trusted Registry cell in Inputs, Outputs, or CellDeps; multiple matching Registry CellDeps; Registry in Inputs/Outputs during Lifecycle; Registry script identity mismatch; appended item ID != State item ID; item not in Registry CellDep. |

## 9. Out of Scope

The following items are outside the scope of the Week 10 MVP architecture:

- Multi-party on-chain authorization (e.g. distinct customer or store lock scripts).
- Direct on-chain verification of borrower identity or customer signatures.
- Partitioned multi-cell registry sharding (future scaling consideration).
- Fiber network payments, Spore / DOB protocol integration.
- Independent physical verification of package delivery or possession.
