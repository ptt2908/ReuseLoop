# ReuseLoop - Project Proposal

## 1. Summary

ReuseLoop is a reusable packaging return system prototype. It aims to record and verify the lifecycle of reusable packaging, including a cross-location return flow where an item is borrowed at Point A and returned at Point B.

## 2. Problem

When reusable packaging can be returned at a location different from where it was borrowed, participants need a consistent way to identify the item, establish its current status, confirm handoffs, and inspect its history. A record in a single client application alone may not provide a shared, verifiable transition history for multiple parties.

## 3. Proposed solution

Use CKB transactions and an appropriate state representation to record item lifecycle transitions. The application will provide item lookup, handoff/return actions, state visibility, and transaction history.

The chain can verify the rules encoded in transaction scripts and the use of cells. It cannot independently prove that a physical item was actually handed over; the application must define which authorized party attests to each event.

## 4. Target actors

- **Operator:** creates and configures items and locations.
- **Issuing point (A):** hands an available item to a borrower.
- **Borrower:** holds the item during the borrowing period.
- **Receiving point (B):** confirms receipt when the item is returned.
- In the MVP, roles may be simulated for a controlled Testnet demo.

## 5. MVP use case

1. An operator registers an item and assigns a unique item ID.
2. Point A hands the item to a borrower and records the handoff.
3. The item becomes unavailable for another borrowing while its state is `BORROWED`.
4. The borrower returns the item at Point B.
5. Point B confirms receipt using an authorized action.
6. The application displays the updated state and history.

## 6. Proposed success criteria

- Item can be identified and looked up by ID or QR.
- Valid handoff and return transitions can be demonstrated.
- Unauthorized, duplicate, and invalid transitions are rejected by the relevant validation layer.
- Current state and recorded transition history can be inspected.
- Documentation distinguishes on-chain facts, off-chain application data, and physical-world attestations.

## 7. Scope

### Included
- Item identity and QR lookup.
- Borrowing and return workflow.
- Different pickup and return locations.
- Authorization and transition validation.
- State/history display.
- Test cases for valid and invalid operations.

### Not included in the core MVP
- Real-world store deployment or integrations.
- Full POS functionality.
- NFC hardware anti-counterfeit.
- Trustless proof of physical handover.
- Mandatory deposits, token settlement, or payment channels.

## 8. Potential extensions

- **Spore/DOB:** a digital representation or collectible layer if it adds a clear user benefit.
- **Fiber:** optional payment or settlement between participants if a real payment use case and funded Testnet setup are feasible.

These extensions are planned candidates for Weeks 11-12, not MVP dependencies. Their inclusion depends on core workflow progress and technical feasibility.

## 9. Main risks

- Correctly enforcing state-transition rules on CKB.
- Designing role permissions without overstating what the chain proves.
- Reliable state/history queries.
- Testnet and wallet integration issues.
- Limited time for optional DOB/Spore and Fiber work.
