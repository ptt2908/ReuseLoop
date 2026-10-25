
# ReuseLoop Cell Linkage Design

## 1. Purpose

Define how the Registry Cell and item State Cells are linked
during item registration and lifecycle transitions.

The Registry is the authoritative record of registered item IDs.
Each State Cell represents the current lifecycle state of one item.

## 2. Registry Identity

The Registry must have a stable Type Script identity.

The implementation must use a validated Registry Type Script
configuration to identify the canonical Registry Cell.

The built-in CKB TYPE_ID mechanism may be used for Registry
identity, subject to validation of the final script design.

A matching Cell Data layout alone is not sufficient to identify
a trusted Registry Cell.

## 3. Item Registration Transaction

A registration transaction must:

1. Consume the current Registry Cell.
2. Create exactly one successor Registry Cell.
3. Preserve the Registry identity.
4. Append exactly one new item ID.
5. Preserve every previously registered ID in its original order.
6. Create one initial State Cell for the new item.
7. Initialize the new State Cell with status AVAILABLE.

The Registry Type Script must reject duplicate IDs, removed IDs,
modified existing IDs, malformed data, and invalid item counts.

The State Type Script must verify that the new State Cell's item ID
is newly registered by the Registry update in the same transaction.

The transaction must fail if either script rejects the operation.

## 4. Lifecycle Transaction

A lifecycle transaction must consume the existing item State Cell
and create its successor State Cell.

The transaction must include a trusted Registry Cell as a cell
dependency so that the State Type Script can verify registration.

The State Type Script must verify:

- The Registry Type Script identity.
- The Registry Data encoding and item count.
- The presence of the State Cell's item ID in the Registry.
- The validity of the input and output item states.
- The permitted status transition.
- The preservation of the item ID and required script identities.

Lifecycle transactions must not modify the Registry.

## 5. State Cell Creation Rules

A newly created State Cell must be associated with a valid
registration transaction.

The State Type Script must distinguish initial registration from
an ordinary lifecycle transition.

It must reject a newly created State Cell whose item ID is not
registered by a valid Registry update in the same transaction.

It must also reject duplicate State Cell creation for the same
item ID within one transaction.

## 6. Registry Cell Dependency Validation

The State Type Script must not trust a Registry Cell merely
because its Data matches the expected binary layout.

It must validate the Registry Type Script identity and parse
the Registry Data according to the agreed schema.

The final implementation must define how the canonical Registry
identity is configured and how its Cell dependency is located.

## 7. Atomicity

Registration updates the Registry and creates the initial State
Cell in one transaction.

If any required script fails, the entire transaction must fail.

Lifecycle transitions update only the relevant State Cell and
must leave the Registry unchanged.

## 8. Security Considerations

- A frontend check is not an on-chain uniqueness guarantee.
- Registry uniqueness depends on Registry Type Script validation.
- State Cell creation must be linked to a valid registration.
- A Cell dependency must be validated, not blindly trusted.
- Initial Registry creation must be controlled.
- Registry initialization and upgrade policies must be defined
  before production deployment.
