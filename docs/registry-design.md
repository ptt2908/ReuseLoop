
# ReuseLoop Registry Design

## 1. Purpose

The Registry controls item registration and prevents duplicate
item IDs at the application protocol level.

The Registry and item State Cells have separate responsibilities:

- Registry: item registration and uniqueness.
- State Cell: current lifecycle state of one registered item.

## 2. Registry Cell

The MVP uses one canonical Registry Cell (a single Registry lineage,
updated by successive `1 -> 1` transactions) containing a fixed 37-byte
header followed by an append-only list of registered 32-byte item IDs.

The Registry Cell contains:

- Registry schema version (`0x01`).
- Registry identity (`registry_id`, 32 bytes).
- Registered item count (`u32` little-endian, 4 bytes).
- Registered item IDs (32 bytes each).

The Registry Cell is governed by the canonical Registry Type Script,
configured with 98-byte Type Script Args, and locked by the
administrator Lock Script (`admin_lock_hash`).

The Registry identity (`registry_id` and the complete Registry Type
Script hash) remains stable across all Registry updates.

## 3. Initial Registry Creation (Genesis: 0 -> 1)

The initial Registry Cell is created through a controlled Genesis
transaction with zero Registry GroupInputs and exactly one Registry
GroupOutput (`0 -> 1`).

The Registry Type Script enforces the following Genesis rules:

- **Group cardinality**: Zero Registry GroupInputs and exactly one
  Registry GroupOutput (`0` GroupOutputs returns `ERROR_MISSING_CELL`;
  more than one GroupInput or GroupOutput returns
  `ERROR_MULTIPLE_CELLS`).
- **Administrator authorization**:
  - Transaction `Input[0]` (`Source::Input[0]`) Lock Script hash must
    equal `admin_lock_hash` (`ERROR_UNAUTHORIZED`).
  - Registry `GroupOutput[0]` Lock Script hash must equal
    `admin_lock_hash` (`ERROR_UNAUTHORIZED`).
- **Occupied capacity**: Registry `GroupOutput[0]` capacity must be
  greater than or equal to its occupied capacity
  (`load_cell_capacity >= load_cell_occupied_capacity`, otherwise
  `ERROR_CAPACITY`).
- **Type ID derivation**: The Registry Type Script directly computes and
  validates the CKB Type ID for `registry_id`:
  - The 52-byte preimage consists of the complete 44-byte serialized
    `CellInput` at `Source::Input[0]` (36-byte `previous_output`
    `OutPoint` plus 8-byte `since`) followed by the 8-byte
    little-endian `u64` global output index (`Source::Output`) of the
    unique output matching the Registry Type Script hash.
  - `registry_id = Blake2b-256(preimage)` using the CKB default hash
    personalization (`"ckb-default-hash"`). Any mismatch with
    `registry_id` in Registry Args returns `ERROR_INVALID_ARGS`.
  - Type ID uniqueness is tied to the selected Genesis seed input
    (`Source::Input[0]`) and the global output index. It makes that
    particular `registry_id` unique, but it does not guarantee a global
    singleton: a different seed input or output index yields a different
    `registry_id` and therefore a different Registry Type Script hash.
    The canonical Registry is the one whose complete Type Script hash
    is trusted by the State Type Script (State Args). Type ID derivation
    also does not independently enforce uniqueness of item IDs stored
    in Registry Cell Data.
- **Initial Registry Data**: Must be a valid 37-byte header with
  `schema_version = 0x01`, `registry_id` matching Registry Args, and
  `item_count = 0` (`ERROR_INVALID_DATA`).
- **State Cell isolation**: No canonical State Cells (matching
  `state_code_hash` and `state_hash_type` from Registry Args) may
  appear in transaction `Inputs` or `Outputs` during Registry Genesis
  (`ERROR_INVALID_TRANSITION`).

## 4. Item Registration (1 -> 1)

Registering an item consumes the current Registry Cell and creates its
successor (`1 -> 1`) while atomically creating one initial `AVAILABLE`
State Cell (`0 -> 1`) for the newly registered item in the same
transaction.

The Registry Type Script validates that:

- **Group cardinality and burn protection**: Exactly one Registry
  `GroupInput` and one Registry `GroupOutput` are present. Attempting to
  consume the Registry without creating a successor (`1 -> 0`) is
  rejected with `ERROR_MISSING_CELL`; multiple Registry inputs or
  outputs are rejected with `ERROR_MULTIPLE_CELLS`.
- **Administrator authorization**: Both Registry `GroupInput[0]` and
  `GroupOutput[0]` Lock Script hashes must match `admin_lock_hash`
  (`ERROR_UNAUTHORIZED`).
- **Capacity preservation**: Registry `GroupOutput[0]` capacity must
  satisfy its occupied capacity (`out_cap >= out_occupied_cap`) and
  cannot decrease relative to `GroupInput[0]` (`out_cap >= in_cap`),
  otherwise `ERROR_CAPACITY`.
- **Registry Data and append-only update**:
  - Both input and output Registry Data headers must have
    `schema_version = 0x01`, `registry_id` matching Registry Args,
    `item_count <= 100` (`ERROR_CAPACITY_LIMIT`), and exact byte length
    `37 + (32 * item_count)` (`ERROR_INVALID_DATA`).
  - Input Registry Data must not contain duplicate item IDs
    (`ERROR_INVALID_DATA`).
  - Output `item_count` must equal `in_count + 1`
    (`ERROR_INVALID_TRANSITION`).
  - All existing item IDs (`bytes [37..37 + 32 * in_count]`) must
    remain unchanged and in the exact same order
    (`ERROR_INVALID_TRANSITION`).
  - The newly appended 32-byte item ID (`new_item_id`) must not already
    exist in the input Registry Data (`ERROR_DUPLICATE_ITEM`).
- **Canonical State Cell constraints**:
  - No canonical State Cell (matching `state_code_hash` and
    `state_hash_type` in Registry Args) may appear in transaction
    `Inputs` (`ERROR_INVALID_TRANSITION`).
  - Exactly one canonical State Cell must be created in transaction
    `Outputs` (`0` matches returns `ERROR_MISSING_CELL`; more than one
    match returns `ERROR_MULTIPLE_CELLS`).
- **Canonical State Output verification**:
  - **Script identity**: The State output Type Script `code_hash` and
    `hash_type` match `state_code_hash` and `state_hash_type` configured
    in Registry Args.
  - **97-byte State Args**: The State Type Script Args must be exactly
    97 bytes with `version = 0x01` and `item_id == new_item_id`
    (`ERROR_INVALID_ARGS`), `trusted_registry_type_hash` equal to the
    Registry's own Type Script hash (`ERROR_UNTRUSTED_REGISTRY`), and
    `admin_lock_hash` equal to the Registry's `admin_lock_hash`
    (`ERROR_UNAUTHORIZED`).
  - **State Lock**: The State output Lock Script hash must match
    `admin_lock_hash` (`ERROR_UNAUTHORIZED`).
  - **State capacity**: The State output capacity must be greater than
    or equal to its occupied capacity (`ERROR_CAPACITY`).
  - **34-byte State Data**: The State output Data must be exactly 34
    bytes with `schema_version = 0x01`, `item_id == new_item_id`, and a
    valid status byte (`ERROR_INVALID_DATA`), and its initial status
    must be `AVAILABLE (0x00)` (`ERROR_INVALID_TRANSITION`).

In the same transaction, the State Type Script executes in Registration
Mode (`0 -> 1`) to verify that the newly created State Cell is paired
with a valid `1 -> 1` update of its trusted Registry Cell. The
transaction fails if either script rejects the operation.

## 5. Lifecycle Transitions

Lifecycle transitions update an existing State Cell (`1 -> 1`) without
modifying the Registry.

During a lifecycle transaction, the Registry Cell is included as a
read-only `CellDep` (and must not appear in transaction `Inputs` or
`Outputs`) so the State Type Script can verify the canonical Registry
identity and confirm that the item ID is registered.

The Registry serves as the authoritative registration record.
The State Cell stores the current status of the registered item.

The State Type Script preserves item identity and enforces the
permitted status transitions.

## 6. Uniqueness Invariant

An item ID must be registered at most once.

Registered IDs cannot be removed or reused.

Uniqueness must be enforced by on-chain validation, not only
by frontend checks or administrator procedures.

## 7. MVP Scalability Limit

The single-Registry design has bounded practical capacity.

The implementation defines a maximum number of registered items
(`MAX_ITEMS = 100`) and rejects registration when the limit is reached.

The maximum accounts for Cell Data size, occupied capacity,
transaction cost, and script execution cost.

## 8. Future Scaling

Future versions may partition the Registry into multiple shards.

A partitioned design must define:

- Deterministic item-to-shard assignment.
- Stable shard identity.
- On-chain enforcement of global uniqueness.
- Shard creation and update authorization.
- A strategy for lookup and migration.

Partitioning must not weaken the uniqueness invariant.

## 9. Out of Scope

- Unbounded single-Cell registries.
- Independent authorization by multiple return points.
- Automatic proof of physical item handoff or return.
- DOB/Spore and Fiber integration.

## 10. Registry Data Encoding

The MVP uses the following binary layout:

| Byte Range | Field | Size | Encoding |
|---|---|---:|---|
| `[0..1]` | `schema_version` | 1 byte | `0x01` |
| `[1..33]` | `registry_id` | 32 bytes | Fixed 32-byte Registry identifier |
| `[33..37]` | `item_count` | 4 bytes | Unsigned little-endian `u32` |
| `[37..]` | `item_ids` | 32 bytes each | Ordered list of registered item IDs |

The total Cell Data size is:

`37 + (32 * item_count)` bytes.

The item count must match the number of item IDs encoded in
the Registry Cell Data.

The validator rejects unsupported schema versions, mismatched
`registry_id`, malformed data, invalid lengths, duplicate IDs, and
inconsistent counts.

Existing item IDs must remain unchanged and in the same order
when a new item is registered.

## 11. MVP Capacity Limit

The MVP supports at most 100 registered items per Registry Cell
(`MAX_ITEMS = 100`).

The Registry Type Script rejects registration with
`ERROR_CAPACITY_LIMIT (9)` when `item_count` exceeds 100.

This is an initial engineering limit, not a CKB protocol limit.

The limit must be reviewed against occupied capacity, transaction
size, and script execution cost before production deployment.

## 12. Registry Type Script Args Encoding

The Registry Type Script requires an exact 98-byte `args` payload:

| Byte Range | Field | Size | Encoding / Validation |
|---|---|---:|---|
| `[0..1]` | `version` | 1 byte | Must be `0x01` (`SCHEMA_VERSION`) |
| `[1..33]` | `registry_id` | 32 bytes | Unique Registry Type ID derived at Genesis |
| `[33..65]` | `admin_lock_hash` | 32 bytes | Expected administrator Lock Script hash |
| `[65..97]` | `state_code_hash` | 32 bytes | Canonical State Type Script `code_hash` |
| `[97..98]` | `state_hash_type` | 1 byte | Canonical State Type Script `hash_type` |

Accepted `state_hash_type` values:

- `0x00`: `Data`
- `0x01`: `Type`
- `0x02`: `Data1`
- `0x04`: `Data2`

Any Registry Type Script `args` with length other than 98 bytes,
`version != 0x01`, or `state_hash_type` outside `{0x00, 0x01, 0x02, 0x04}`
is rejected with `ERROR_INVALID_ARGS (1)`.

## 13. Registry Error Code Reference

The Registry Type Script returns `0` on success and uses the following
error codes on validation failure:

| Code | Constant | Trigger Conditions in Registry Type Script |
|---:|---|---|
| `1` | `ERROR_INVALID_ARGS` | Malformed Registry Args (length != 98, `version != 0x01`, invalid `state_hash_type`); Genesis `registry_id` mismatch against the 52-byte Type ID preimage; canonical State output Args length != 97, `version != 0x01`, or `state_args.item_id != new_item_id`. |
| `2` | `ERROR_INVALID_DATA` | Registry Cell Data length `< 37` or `!= 37 + 32 * item_count`; `schema_version != 0x01`; `data.registry_id != args.registry_id`; Genesis `item_count != 0` (with distinct IDs); duplicate item IDs already present in Registry input Data during Registration (`1 -> 1`); canonical State output Data length != 34, `schema_version != 0x01`, `state_data.item_id != new_item_id`, or invalid status byte. |
| `3` | `ERROR_INVALID_TRANSITION` | Canonical State cell present in `Source::Input` during any Registry transaction; canonical State cell present in `Source::Output` during Genesis; Registration `out_count != in_count + 1`; modified or reordered existing item ID prefix; canonical State output initial status `!= AVAILABLE (0x00)`. |
| `4` | `ERROR_MISSING_CELL` | Missing Registry `GroupOutput` (`group_out_count == 0`, including `1 -> 0` burn attempts); missing canonical State output during Registration (`1 -> 1`); unexpected syscall failure when loading a required cell or input. |
| `5` | `ERROR_MULTIPLE_CELLS` | More than one Registry `GroupInput` or `GroupOutput`; more than one matching Registry output during Genesis Type ID check; more than one canonical State output during Registration (`1 -> 1`). |
| `6` | `ERROR_UNAUTHORIZED` | Registry `GroupOutput[0]` Lock hash `!= admin_lock_hash`; Genesis `Source::Input[0]` Lock hash `!= admin_lock_hash`; Registration `GroupInput[0]` Lock hash `!= admin_lock_hash`; canonical State output `state_args.admin_lock_hash != admin_lock_hash` or State output Lock hash `!= admin_lock_hash`. |
| `7` | `ERROR_CAPACITY` | Registry `GroupOutput[0]` capacity `< occupied_capacity`; Registration `GroupOutput[0]` capacity `< GroupInput[0]` capacity; canonical State output capacity `< occupied_capacity`. |
| `8` | `ERROR_DUPLICATE_ITEM` | Newly appended `new_item_id` during Registration (`1 -> 1`) already exists in `GroupInput[0]` Registry Data. In Genesis, the shared Registry Data parser also returns this code if the output payload has `item_count > 0` and contains duplicate IDs (checked before the `item_count == 0` rule); a valid Genesis payload has `item_count = 0` and cannot contain duplicates. This Genesis path is present in the implementation but is not covered by a dedicated test. |
| `9` | `ERROR_CAPACITY_LIMIT` | Registry `item_count > 100` (`MAX_ITEMS`). |
| `10` | `ERROR_UNTRUSTED_REGISTRY` | Canonical State output `state_args.trusted_registry_type_hash` (`[33..65]`) does not match the Registry's own Type Script hash (`self_type_hash`). |
