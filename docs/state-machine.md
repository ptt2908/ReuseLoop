# ReuseLoop - State Machine (Draft)

## 1. Proposed states

- `AVAILABLE`: item may be borrowed.
- `BORROWED`: item has been handed to a borrower and has not yet had a valid return confirmed.
- `RETURNED`: receiving point has recorded receipt. This may be an event state or a short-lived state before the item becomes available again.

## 2. Draft transitions

| Current state | Action | Authorized actor (draft) | Next state | Validation |
|---|---|---|---|---|
| Not registered | Register item | Operator | AVAILABLE | Unique item ID and valid registration |
| AVAILABLE | Handoff | Issuing point A | BORROWED | Item is available; actor is authorized |
| BORROWED | Confirm return | Receiving point B | RETURNED | Item is borrowed; actor is authorized to receive |
| RETURNED | Make available again | Operator or receiving point, to be decided | AVAILABLE | Return is recorded; any required inspection is complete |

## 3. Important rules

- Point B does not need to equal Point A.
- A borrowed item cannot be handed out again before a valid return.
- The same consumed input Cell cannot be consumed by two accepted CKB transactions; however, this alone does not enforce all business rules.
- Script validation, transaction construction, authorization, and application error handling must be designed and tested.
- On-chain state proves that an authorized transaction satisfying the implemented rules was accepted. It does not independently prove the physical item was handed over or returned.

## 4. Open decision

For the initial MVP, the simplest option is to confirm return and make the item available again through a clearly defined transition. If an inspection stage is introduced, add `INSPECTION` only when its business rules are specified.
