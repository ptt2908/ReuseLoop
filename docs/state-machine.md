
# ReuseLoop State Machine

## 1. Purpose

ReuseLoop tracks the lifecycle of reusable packaging on CKB.
The MVP uses a single demo administrator wallet to authorize
and submit all lifecycle transactions.

Each packaging item has one current state represented by a
ReuseLoop state Cell.

## 2. States

- AVAILABLE: The item is ready to be borrowed.
- BORROWED: The item has been handed over and is awaiting return.
- RETURNED: A receiving point has confirmed that the item was returned.

## 3. Valid Transitions

| Operation | Input State | Output State | Meaning |
|---|---|---|---|
| Borrow | AVAILABLE | BORROWED | Record item handoff |
| ConfirmReturn | BORROWED | RETURNED | Record return confirmation |
| MakeAvailable | RETURNED | AVAILABLE | Complete the return cycle |

Any transition not listed above must be rejected.

## 4. MVP Authorization

- A single administrator wallet signs and submits all lifecycle transactions.
- The frontend must not be treated as the security boundary.
- The on-chain validation design must enforce valid state transitions.
- The MVP does not claim that each physical location independently
  authenticates itself.
- Location identifiers are recorded as transaction data or item metadata.
  They do not prove physical handoff or receipt by themselves.

## 5. State Transition Rules

### Borrow

Preconditions:
- The current item state is AVAILABLE.
- The transaction identifies the item being borrowed.
- The administrator authorization is valid.

Result:
- Consume the previous state Cell.
- Create the next state Cell with status BORROWED.
- Record the relevant handoff metadata.

### ConfirmReturn

Preconditions:
- The current item state is BORROWED.
- The transaction identifies the item being returned.
- The administrator authorization is valid.

Result:
- Consume the previous state Cell.
- Create the next state Cell with status RETURNED.
- Record the receiving location and return metadata.

### MakeAvailable

Preconditions:
- The current item state is RETURNED.
- The transaction identifies the same item.
- The administrator authorization is valid.

Result:
- Consume the previous state Cell.
- Create the next state Cell with status AVAILABLE.

## 6. Invalid Cases

The implementation must reject:
- Borrowing an item that is not AVAILABLE.
- Confirming return for an item that is not BORROWED.
- Making an item AVAILABLE unless its current state is RETURNED.
- Changing the item identifier during a lifecycle transition.
- Creating an invalid or unauthorized state transition.
- Reusing an already-consumed input Cell.

## 7. Important Limitations

CKB's UTXO model prevents two confirmed transactions from
consuming the same input Cell. It does not automatically enforce
all ReuseLoop business rules.

The final on-chain design must define a Type Script or equivalent
validation mechanism that checks the required input/output relationship,
state transition, item identity, and authorization conditions.

An on-chain record is an authorized attestation. It does not independently
prove that a physical package was handed over or returned.

## 8. Future Extensions

Per-location authorization, DOB/Spore item identity, and Fiber-based
payments are optional extensions and are not MVP dependencies.
