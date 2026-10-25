
# ReuseLoop Registry Design

## 1. Purpose

The Registry controls item registration and prevents duplicate
item IDs at the application protocol level.

The Registry and item State Cells have separate responsibilities:

- Registry: item registration and uniqueness.
- State Cell: current lifecycle state of one registered item.

## 2. Registry Cell

The MVP uses one Registry Cell containing an append-only list
of registered 32-byte item IDs.

The Registry Cell contains:

- Registry schema version.
- Registry identity.
- Registered item count.
- Registered item IDs.

The exact binary encoding must be defined before implementation.

The Registry identity must remain stable across Registry updates.

## 3. Initial Registry Creation

The initial Registry Cell must be created through a controlled
initialization transaction.

The implementation must establish a unique Registry identity
and restrict initialization to the administrator.

The built-in CKB TYPE_ID mechanism may be used to establish the
Registry Type Script identity.

TYPE_ID does not independently enforce uniqueness of item IDs
stored in Registry Cell Data.

## 4. Item Registration

Registering an item consumes the current Registry Cell and
creates its successor.

The same transaction creates an AVAILABLE State Cell for the
new item.

The Registry Type Script must validate that:

- The Registry input and output are correctly matched.
- The Registry identity remains unchanged.
- Existing item IDs are preserved in the same order.
- Exactly one new item ID is appended.
- The new item ID did not previously exist.
- The registered item count is updated correctly.
- The administrator authorization requirements are satisfied.

The State Type Script must validate that a newly created State
Cell corresponds to the item registered in the same transaction.

The scripts must reject malformed or inconsistent transactions.

## 5. Lifecycle Transitions

Lifecycle transitions update an existing State Cell without
modifying the Registry.

The Registry serves as the authoritative registration record.
The State Cell stores the current status of the registered item.

The State Type Script must preserve item identity and enforce
the permitted status transitions.

## 6. Uniqueness Invariant

An item ID must be registered at most once.

Registered IDs cannot be removed or reused.

Uniqueness must be enforced by on-chain validation, not only
by frontend checks or administrator procedures.

## 7. MVP Scalability Limit

The single-Registry design has bounded practical capacity.

The implementation must define a maximum number of registered
items and reject registration when the limit is reached.

The maximum must account for Cell Data size, occupied capacity,
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

| Field | Size | Encoding |
|---|---:|---|
| schema_version | 1 byte | 0x01 |
| registry_id | 32 bytes | Fixed 32-byte identifier |
| item_count | 4 bytes | Unsigned little-endian integer |
| item_ids | 32 bytes each | Ordered list of registered item IDs |

The total Cell Data size is:

`37 + (32 * item_count)` bytes.

The item count must match the number of item IDs encoded in
the Registry Cell Data.

The decoder must reject unsupported schema versions, malformed
data, invalid lengths, duplicate IDs, and inconsistent counts.

Existing item IDs must remain unchanged and in the same order
when a new item is registered.

## 11. MVP Capacity Limit

The MVP supports at most 100 registered items per Registry Cell.

The Registry Type Script must reject registration when the
item count has reached this limit.

This is an initial engineering limit, not a CKB protocol limit.

The limit must be reviewed against occupied capacity, transaction
size, and script execution cost before production deployment.
