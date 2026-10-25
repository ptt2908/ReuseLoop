
# ReuseLoop Architecture

## 1. Overview

ReuseLoop is a reusable packaging lifecycle application built on CKB.

The MVP records the lifecycle of an item from availability, through
borrowing and return confirmation, to availability for reuse.

## 2. Components

### Frontend
- React
- TypeScript
- Vite
- CCC SDK for CKB interaction
- Displays item state and transaction history
- Initiates lifecycle operations through the administrator wallet

### Administrator Wallet
- One demo wallet authorizes and signs lifecycle transactions.
- The wallet is an MVP simplification, not a multi-party authorization model.
- Private keys and recovery phrases must never be stored in frontend source code.

### CKB
- Stores the current state in a ReuseLoop state Cell.
- Records lifecycle changes as transactions.
- Uses Script validation to enforce the state transition rules.

### Optional Off-chain Data
- Item display names, images, and supplementary metadata may be stored off-chain.
- Off-chain data must not replace on-chain validation of critical state transitions.
- Transaction history may be indexed for convenient frontend queries.

## 3. State Cell Model

Each item has one current ReuseLoop state Cell.

The proposed logical fields are:

- schema_version
- item_id
- status
- current_holder, when required by the MVP
- last_location, if included in the chosen encoding

The final binary encoding and field sizes must be specified before
implementing the on-chain validator.

### Lock Script

Controls who can consume the Cell according to the chosen authorization
mechanism. The exact Lock Script and administrator signing flow must
be selected during implementation.

### Type Script

Validates the ReuseLoop state Cell and permitted lifecycle transitions.
The implementation must check the relationship between input and output
Cells, including item identity and state transition.

### Data

Contains the encoded ReuseLoop item state. The encoding must be
deterministic and validated by the Type Script.

## 4. Transaction Flow

### Borrow
AVAILABLE Cell -> BORROWED Cell

### Confirm Return
BORROWED Cell -> RETURNED Cell

### Make Available
RETURNED Cell -> AVAILABLE Cell

Each transition consumes the previous state Cell and creates its successor.
The previous Cell cannot be consumed again after it has been spent in a
confirmed transaction.

## 5. Validation Requirements

The implementation must define checks for:
- Correct input state.
- Correct output state.
- Unchanged item identifier.
- Valid administrator authorization.
- Valid state data encoding.
- Correct handling of transactions that attempt invalid transitions.

The exact mechanism for validating administrator authorization must be
confirmed in the Script design. A wallet signing a transaction alone
does not automatically prove that a custom Type Script has validated
the administrator's identity.

## 6. MVP Scope

Included:
- Query item state.
- Record borrowing.
- Confirm return.
- Mark returned items available.
- Query transaction history.
- Test valid and invalid transitions.

Not required for MVP:
- Multi-store independent authorization.
- DOB/Spore integration.
- Fiber payment integration.
- Production deployment.

## 7. Physical-World Limitation

The blockchain records authorized statements about item handoff and return.
It cannot independently verify that the physical package changed hands.
