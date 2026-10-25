# ReuseLoop - Architecture Draft

## 1. Logical components

1. **Frontend:** React + TypeScript + Vite; item lookup, QR, borrow/return screens, current status, history.
2. **Wallet integration:** CCC/JoyID for connection and transaction signing, subject to feasibility checks.
3. **Application services:** item queries, transaction building/submission, status mapping, and error handling.
4. **CKB:** cells and transactions represent lifecycle state; scripts validate the rules implemented by the chosen design.
5. **Optional query/cache layer:** considered only if direct RPC queries are insufficient for item discovery or history.

## 2. Data separation

### Candidate on-chain data
- Stable item identifier or identifier commitment.
- Lifecycle state.
- Issuing/receiving point identifiers as needed for validation.
- Borrower reference only if privacy implications are acceptable.
- Transaction-level transition data needed by the validation design.

### Candidate off-chain data
- Display names, images, item descriptions, and UI preferences.
- QR rendering and convenience metadata.
- Any personal data not required for on-chain validation.

The final schema must be decided after the technical spike. Do not put personal information on-chain by default.

## 3. Transaction flow (conceptual)

1. UI requests a state-changing action.
2. Service loads/validates current item state and builds a transaction.
3. Wallet signs the transaction where applicable.
4. Transaction is submitted to CKB.
5. App tracks submission and confirmation separately.
6. App refreshes current state and history after confirmation.

## 4. Authorization questions

- How are operator, issuing point, and receiving point identities represented?
- Which party is allowed to update the state?
- Can a receiving point confirm a return if it is not the original issuing point?
- What Script design can enforce these rules on-chain?

## 5. Design caveat

This document is a draft. CKB's cell model does not automatically implement the application state machine. The chosen lock/type scripts and transaction structure must enforce the required invariants.
