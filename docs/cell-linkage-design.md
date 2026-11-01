
# ReuseLoop Cell Linkage Design

## 1. Purpose

Define how the Registry Cell and item State Cells are linked
during item registration and lifecycle transitions.

The Registry is the authoritative record of registered item IDs.
Each State Cell represents the current lifecycle state of one item.

This document specifies the State Type Script and its linkage to the
Registry. The Registry Type Script itself (Args, Data, Genesis and
Registration rules, error codes) is specified in
[registry-design.md](registry-design.md) and is not repeated here.

Statements in this document describe implemented behavior unless they
are explicitly marked as a future consideration.

## 2. Registry Identity and Trust Model

Three related but distinct values are involved:

| Value | Where it is stored | Meaning |
|---|---|---|
| `registry_id` | Registry Type Script Args `[1..33]` and Registry Data `[1..33]` | 32-byte Type ID derived at Registry Genesis. |
| Registry Type Script hash | Not stored in the Registry. Computed by CKB from the complete Molecule-serialized Registry Type Script (`code_hash`, `hash_type`, `args`). | The complete identity of one specific Registry Type Script instance. |
| `trusted_registry_type_hash` | State Type Script Args `[33..65]` | The Registry Type Script hash that this State Cell trusts. |

The State Type Script trusts one specific Registry Type Script
identity through its complete Script hash, not merely through the
`registry_id`. A Registry Cell is located by comparing the type hash
of candidate cells (`load_cell_type_hash`) with
`trusted_registry_type_hash`. Because the Registry Type Script hash
covers the Registry Args, it also commits to `registry_id`,
`admin_lock_hash`, `state_code_hash` and `state_hash_type`.

A matching Cell Data layout alone is not sufficient to identify a
trusted Registry Cell.

The Registry Type ID is derived inside the Registry Type Script at
Genesis (see [registry-design.md](registry-design.md), Section 3).
Its uniqueness is tied to the selected Genesis seed input and the
global output index. It does not guarantee a global singleton across
all possible seeds: a different seed produces a different
`registry_id` and a different Registry Type Script hash. The canonical
Registry for a State Cell is the one whose complete Type Script hash
appears in that State Cell's Args.

### 2.1 Bidirectional Linkage

The linkage is verified from both sides:

- **Registry to State**: Registry Args carry the canonical State Type
  Script `code_hash` and `hash_type` (`state_code_hash`,
  `state_hash_type`). During registration the Registry Type Script
  finds the canonical State output and checks that its Args contain
  the Registry's own Type Script hash, the registered item ID and
  `admin_lock_hash` (see [registry-design.md](registry-design.md),
  Section 4).
- **State to Registry**: State Args carry
  `trusted_registry_type_hash`. The State Type Script locates the
  trusted Registry Cell by that hash, validates the Registry Args, and
  checks that its own `code_hash` and `hash_type` equal the
  `state_code_hash` and `state_hash_type` configured in the Registry
  Args.

The Registry Args hold only the State `code_hash` and `hash_type`,
while the State Args hold the complete Registry Type Script hash.

## 3. State Type Script Args

The State Type Script requires an exact 97-byte `args` payload:

| Byte Range | Field | Size |
|---|---|---:|
| `[0..1]` | `version` | 1 byte |
| `[1..33]` | `item_id` | 32 bytes |
| `[33..65]` | `trusted_registry_type_hash` | 32 bytes |
| `[65..97]` | `admin_lock_hash` | 32 bytes |

Validation:

- `version` must be `0x01`.
- The Args length must be exactly 97 bytes.
- Args with any other length or version are rejected with
  `ERROR_INVALID_ARGS (1)`.
- `trusted_registry_type_hash` must equal the complete canonical
  Registry Type Script hash. The State Type Script locates the
  Registry Cell by this hash. If no matching Registry Cell is found
  where one is required, the transaction is rejected with
  `ERROR_UNTRUSTED_REGISTRY (10)`.
- `admin_lock_hash` must equal the administrator Lock Script hash
  configured in the Registry Args, and must equal the Lock Script hash
  of the State Cells and the Registry Cell that the State Type Script
  checks. A mismatch is rejected with `ERROR_UNAUTHORIZED (6)`.

The State Cell Data layout (34 bytes: `schema_version`, `item_id`,
`status`) is specified in [transaction-design.md](transaction-design.md).
In both modes the Data `item_id` must equal the Args `item_id`.

## 4. State Script Dispatch and Cardinality

After validating the Args, the State Type Script counts the State
cells in its script group (`GroupInput` and `GroupOutput`) and
dispatches as follows:

| State GroupInputs | State GroupOutputs | Result |
|---|---|---|
| more than 1 | any | `ERROR_MULTIPLE_CELLS (5)` |
| any | more than 1 | `ERROR_MULTIPLE_CELLS (5)` |
| 0 | 0 | `ERROR_MISSING_CELL (4)` |
| 1 | 0 | `ERROR_MISSING_CELL (4)` (State burn is rejected) |
| 0 | 1 | Registration Mode (Section 5) |
| 1 | 1 | Lifecycle Mode (Section 6) |

Only the combinations `(0, 1)` and `(1, 1)` are accepted for further
validation; every other combination is rejected. The `(0, 0)` row is
a defensive branch of the implementation. CKB executes a Type Script
group only when it contains at least one cell, so this row is not
expected to be reachable in a real transaction (not exercised by a
test).

The multiple-cell check is evaluated before the missing-cell check.
A failure while counting group cells other than an out-of-bound index
is reported as `ERROR_MISSING_CELL (4)`.

The two modes are mutually exclusive and have opposite requirements
for the Registry:

| | Registration Mode (`0 -> 1`) | Lifecycle Mode (`1 -> 1`) |
|---|---|---|
| Registry in transaction Inputs | Exactly one required | Must not be present |
| Registry in transaction Outputs | Exactly one required | Must not be present |
| Registry in `CellDep` | Not used | Exactly one required |
| Registry modified | Yes (append one item ID) | No (read-only) |

A transaction that creates a State Cell with a Registry `CellDep` but
without the Registry `1 -> 1` update is rejected by Registration Mode
with `ERROR_UNTRUSTED_REGISTRY (10)`. A transaction that spends a State
Cell while also spending or recreating the Registry is rejected by
Lifecycle Mode with `ERROR_UNTRUSTED_REGISTRY (10)`.

## 5. Registration Mode (State 0 -> 1)

### 5.1 Atomic Registration Transaction

A registration transaction must:

1. Consume the current Registry Cell (Registry `1 -> 1`).
2. Create exactly one successor Registry Cell.
3. Preserve the Registry Type Script identity.
4. Append exactly one new item ID.
5. Preserve every previously registered ID in its original order.
6. Create exactly one initial State Cell for the new item
   (State `0 -> 1`).
7. Initialize the new State Cell with status `AVAILABLE (0x00)`.

Registry and State are updated atomically in the same transaction.
Both Type Scripts execute and both must accept it: the Registry Type
Script validates the Registry update and the canonical State output,
and the State Type Script validates the new State Cell and its
relationship to the trusted Registry input and output. If either
script rejects the transaction, the whole transaction fails.

### 5.2 State Type Script Checks

In Registration Mode the State Type Script checks the following, in
the order shown. Error constants are those returned by the
implementation.

| Check | Error |
|---|---|
| State output Lock hash equals `admin_lock_hash` | `ERROR_UNAUTHORIZED (6)` |
| At least one transaction Input has a Lock hash equal to `admin_lock_hash` | `ERROR_UNAUTHORIZED (6)` |
| State output capacity is at least its occupied capacity | `ERROR_CAPACITY (7)` |
| State output Data is exactly 34 bytes, `schema_version = 0x01`, Data `item_id` equals Args `item_id`, and the status byte is a defined value | `ERROR_INVALID_DATA (2)` |
| Initial status is `AVAILABLE (0x00)` | `ERROR_INVALID_TRANSITION (3)` |
| More than one Input, or more than one Output, has the trusted Registry type hash | `ERROR_MULTIPLE_CELLS (5)` |
| No Input, or no Output, has the trusted Registry type hash | `ERROR_UNTRUSTED_REGISTRY (10)` |
| Registry Input and Output Lock hashes equal `admin_lock_hash` | `ERROR_UNAUTHORIZED (6)` |
| Registry Input and Output have no Type Script, or their Type Scripts are not identical | `ERROR_UNTRUSTED_REGISTRY (10)` |
| Registry Args are exactly 98 bytes with `version = 0x01` | `ERROR_INVALID_ARGS (1)` |
| Registry Args `admin_lock_hash` equals the State Args `admin_lock_hash` | `ERROR_UNAUTHORIZED (6)` |
| State Type Script `code_hash` and `hash_type` equal Registry Args `state_code_hash` and `state_hash_type` | `ERROR_UNTRUSTED_REGISTRY (10)` |
| At most one transaction Output has the canonical State `code_hash` and `hash_type` | `ERROR_MULTIPLE_CELLS (5)` |
| Registry Input Data header is valid (length, `schema_version = 0x01`, `registry_id` equals Registry Args `registry_id`, exact length `37 + 32 * item_count`) | `ERROR_INVALID_DATA (2)` |
| Registry Input `item_count` does not exceed 100 | `ERROR_CAPACITY_LIMIT (9)` |
| Registry Input Data contains no duplicate item IDs | `ERROR_INVALID_DATA (2)` |
| Registry Output Data header is valid (same rules as the Input header), and its `item_count` does not exceed 100 | `ERROR_INVALID_DATA (2)` / `ERROR_CAPACITY_LIMIT (9)` |
| Registry Output `item_count` equals Input `item_count + 1` | `ERROR_INVALID_TRANSITION (3)` |
| Registry Output preserves the Input item ID prefix in the same order | `ERROR_INVALID_TRANSITION (3)` |
| The appended item ID does not already exist in Registry Input Data | `ERROR_DUPLICATE_ITEM (8)` |
| The appended item ID equals the State Args `item_id` | `ERROR_UNTRUSTED_REGISTRY (10)` |

Notes:

- Trusted Registry cells are matched by type hash. Multiple matches
  in the Inputs or the Outputs are reported as
  `ERROR_MULTIPLE_CELLS (5)` before a missing match is reported as
  `ERROR_UNTRUSTED_REGISTRY (10)`.
- A newly appended item ID that already exists in the Registry Input
  Data is rejected with `ERROR_DUPLICATE_ITEM (8)` by the State Type
  Script (this table) and also by the Registry Type Script
  (see [registry-design.md](registry-design.md)). The two scripts
  report the same code for this condition, but each runs its own
  check. Duplicates already present in the Registry Input Data are
  `ERROR_INVALID_DATA (2)` in both scripts.
- The State Type Script does not repeat every Registry check. For
  example, Registry capacity preservation and the Registry Genesis
  rules are enforced by the Registry Type Script
  (see [registry-design.md](registry-design.md)).

### 5.3 State Cell Creation Rules

The State Type Script distinguishes initial registration
(`0 GroupInput -> 1 GroupOutput`) from an ordinary lifecycle
transition (`1 -> 1`) by group cardinality.

A newly created State Cell is accepted only when it is linked to a
valid Registry `1 -> 1` update in the same transaction. A State Cell
whose item ID is not appended by that update is rejected, and a second
canonical State output in the same transaction is rejected, either by
the State Type Script (`ERROR_MULTIPLE_CELLS (5)`) or by the Registry
Type Script.

## 6. Lifecycle Mode (State 1 -> 1)

A lifecycle transaction consumes the existing item State Cell and
creates its successor State Cell (`1 GroupInput -> 1 GroupOutput`).

The Registry is read-only during lifecycle transitions. The
transaction must include the trusted Registry Cell as a `CellDep` so
that the State Type Script can verify registration. The Registry must
not be consumed or recreated by a lifecycle transaction.

In Lifecycle Mode the State Type Script checks the following, in the
order shown:

| Check | Error |
|---|---|
| State Input and Output Lock hashes equal `admin_lock_hash` | `ERROR_UNAUTHORIZED (6)` |
| State output capacity is at least its occupied capacity, and does not decrease relative to the State input | `ERROR_CAPACITY (7)` |
| State Input and Output Data are each exactly 34 bytes, `schema_version = 0x01`, Data `item_id` equals Args `item_id` (item identity is preserved), and the status byte is a defined value | `ERROR_INVALID_DATA (2)` |
| Status transition is one of `AVAILABLE -> BORROWED`, `BORROWED -> RETURNED`, `RETURNED -> AVAILABLE` | `ERROR_INVALID_TRANSITION (3)` |
| No Input or Output has the trusted Registry type hash | `ERROR_UNTRUSTED_REGISTRY (10)` |
| Exactly one `CellDep` has the trusted Registry type hash (none, or more than one, is rejected) | `ERROR_UNTRUSTED_REGISTRY (10)` |
| Registry `CellDep` Lock hash equals `admin_lock_hash` | `ERROR_UNAUTHORIZED (6)` |
| Registry `CellDep` has a Type Script | `ERROR_UNTRUSTED_REGISTRY (10)` |
| Registry Args are exactly 98 bytes with `version = 0x01` | `ERROR_INVALID_ARGS (1)` |
| Registry Args `admin_lock_hash` equals the State Args `admin_lock_hash` | `ERROR_UNAUTHORIZED (6)` |
| State Type Script `code_hash` and `hash_type` equal Registry Args `state_code_hash` and `state_hash_type` | `ERROR_UNTRUSTED_REGISTRY (10)` |
| Registry Data header is valid (`schema_version = 0x01`, `registry_id` equals Registry Args `registry_id`, exact length `37 + 32 * item_count`) | `ERROR_INVALID_DATA (2)` |
| Registry `item_count` does not exceed 100 | `ERROR_CAPACITY_LIMIT (9)` |
| Registry Data contains no duplicate item IDs | `ERROR_DUPLICATE_ITEM (8)` |
| The State `item_id` is present in Registry Data | `ERROR_UNTRUSTED_REGISTRY (10)` |

The State Cell Type Script and Lock Script identity are preserved
because the Type Script group contains both the input and output
cell, and both Lock hashes must equal `admin_lock_hash`.

Note that duplicate IDs already present in Registry Data are reported
as `ERROR_DUPLICATE_ITEM (8)` by the State Type Script in Lifecycle
Mode, whereas the State Type Script in Registration Mode reports
duplicates in the Registry input Data as `ERROR_INVALID_DATA (2)`.
This reflects the implementation as it stands.

## 7. Atomicity

Registration updates the Registry and creates the initial State Cell
in one transaction. Both the Registry and State Type Scripts must
accept the transaction. If any required script fails, the entire
transaction fails.

Lifecycle transitions update only the relevant State Cell and leave
the Registry unchanged.

## 8. Authorization

The Registry and State Type Scripts do not verify cryptographic
signatures. They validate transaction invariants and compare Lock
Script hashes:

- Registry and State Type Scripts check that the relevant cells use
  the Lock Script whose hash equals `admin_lock_hash`. In State
  Registration Mode the State Type Script also checks that at least one
  transaction Input uses that Lock Script hash.
- The administrator Lock Script, which guards each Input that uses
  `admin_lock_hash`, validates the actual signature when the
  transaction is executed. The Type Scripts depend on that Lock
  Script running and succeeding.

The integration tests use the CKB `secp256k1_blake160_sighash_all`
Lock Script as the administrator Lock Script and sign transactions
with a real secp256k1 key. Tests that use `ALWAYS_SUCCESS` as a lock
do not verify signatures.

This is the MVP single-administrator model. Independent authorization
by multiple stores or return points is not part of this design.

## 9. Security Considerations

- A frontend check is not an on-chain uniqueness guarantee.
- Registry uniqueness depends on Registry Type Script validation.
- State Cell creation is linked to a valid registration through the
  Registry `1 -> 1` update in the same transaction.
- A Registry Cell is identified by its complete Type Script hash and
  validated (Lock, Args, Data), not blindly trusted because of its
  Data layout.
- Type ID uniqueness is bound to the Genesis seed and output index.
  State Cells are bound to one Registry through
  `trusted_registry_type_hash`.
- Initial Registry creation is restricted to the administrator by the
  Registry Type Script (see [registry-design.md](registry-design.md)).
- Registry initialization and upgrade policies beyond the implemented
  Genesis and append-only registration rules remain future
  considerations and must be defined before production deployment.

## 10. Test Coverage Notes

The following State Type Script checks are implemented in
`contracts/state/src/main.rs` but currently have no dedicated State
Registration Mode test. These are test coverage limitations, not
implementation limitations:

- The State output occupied-capacity check in Registration Mode.
- The Registry Input and Output Lock hash check (`admin_lock_hash`)
  in Registration Mode.
- The Registry Args `state_hash_type` mismatch check in Registration
  Mode. The equivalent check in Lifecycle Mode is tested.
- The check that more than one canonical State output is rejected at
  the State Type Script level. The same condition is covered at the
  Registry Type Script level.
