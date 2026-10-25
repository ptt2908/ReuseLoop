
# ReuseLoop Transaction Design

## 1. Purpose

Define the transaction structure and validation rules for the
ReuseLoop item lifecycle on CKB Testnet.

## 2. State Cell

Each registered item is represented by one State Cell.

The Cell Data schema is 34 bytes:

- Schema version: 1 byte, value 0x01.
- Item ID: 32 bytes.
- Status: 1 byte.
  - 0x00: AVAILABLE
  - 0x01: BORROWED
  - 0x02: RETURNED

The Lock Script is controlled by the administrator wallet.
The ReuseLoop Type Script validates item state transitions.

## 3. Lifecycle Transactions

### 3.1 Borrow

Input state: AVAILABLE
Output state: BORROWED

Requirements:
- Preserve the item ID.
- Preserve the State Cell Type Script identity.
- Preserve the administrator Lock Script.
- Reject any other status transition.

### 3.2 ConfirmReturn

Input state: BORROWED
Output state: RETURNED

Requirements:
- Preserve the item ID.
- Preserve the State Cell Type Script identity.
- Preserve the administrator Lock Script.
- Reject any other status transition.

### 3.3 MakeAvailable

Input state: RETURNED
Output state: AVAILABLE

Requirements:
- Preserve the item ID.
- Preserve the State Cell Type Script identity.
- Preserve the administrator Lock Script.
- Reject any other status transition.

## 4. Authorization

The MVP uses one administrator wallet to sign and submit all
lifecycle transactions.

The State Cell Lock Script requires administrator authorization
when the State Cell is consumed.

The Type Script must validate the state transition and prevent
unauthorized changes to the State Cell's identifying properties.

## 5. Initial State Creation

Initial State Cell creation requires a separate validation rule.

The implementation must define how item IDs are registered,
how duplicate item creation is prevented, and how creation
is restricted to the administrator.

## 6. Validation Invariants

- Only supported schema versions are accepted.
- Only supported status values are accepted.
- Item IDs remain unchanged during lifecycle transitions.
- Only the three defined transitions are allowed.
- A lifecycle transaction cannot silently replace the State Cell
  Lock Script or Type Script.
- Each transition consumes the previous State Cell and creates
  its successor atomically.
- Output capacity must satisfy CKB occupied-capacity rules.

## 7. Out of Scope

- Independent authorization by multiple return points.
- Automatic proof of physical packaging handoff or return.
- DOB/Spore and Fiber integration.
- A separate on-chain history registry.
