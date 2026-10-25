# ReuseLoop - Requirements

## 1. Functional requirements

- **FR-01 Item identity:** The system shall assign each reusable item a unique identifier.
- **FR-02 Item lookup:** The system shall allow an item to be found by its identifier; a QR code may encode a lookup link or identifier.
- **FR-03 Item registration:** An authorized operator shall be able to register an item.
- **FR-04 Handoff:** An authorized issuing point shall be able to record a handoff for an item in `AVAILABLE` state.
- **FR-05 Borrower association:** The handoff record shall identify the borrower using an agreed non-sensitive identifier or wallet address.
- **FR-06 Return confirmation:** An authorized receiving point shall be able to confirm receipt of an item in `BORROWED` state.
- **FR-07 Cross-location return:** The receiving point may differ from the issuing point.
- **FR-08 State validation:** The system shall reject transitions that do not satisfy the defined state and permission rules.
- **FR-09 Duplicate handling:** The system shall handle repeated submissions without recording an invalid second transition.
- **FR-10 History:** The system shall display recorded lifecycle transitions and relevant transaction references.
- **FR-11 Current status:** The system shall show the current state of an item.
- **FR-12 Error feedback:** The system shall explain failures such as invalid state, insufficient authorization, RPC failure, or rejected transaction.

## 2. Non-functional requirements

- **NFR-01 Verifiability:** The app shall distinguish chain-verified state from off-chain display data.
- **NFR-02 Privacy:** Personal data shall not be written on-chain unless explicitly justified and reviewed.
- **NFR-03 Reliability:** Transaction submission and confirmation shall be represented as distinct states.
- **NFR-04 Maintainability:** UI logic and CKB transaction logic should be separated into modules/services.
- **NFR-05 Testability:** State transitions and invalid operations shall have repeatable tests.
- **NFR-06 Usability:** The core workflow should be demonstrable without requiring real stores or physical inventory.

## 3. Initial acceptance criteria

- **AC-01:** A registered item can be looked up by item ID.
- **AC-02:** An available item can be handed off by an authorized issuing point.
- **AC-03:** A borrowed item cannot be borrowed again before a valid return transition.
- **AC-04:** A receiving point different from the issuing point can confirm a return if authorized by the design.
- **AC-05:** An unauthorized return confirmation is rejected.
- **AC-06:** Duplicate return submission does not create a second valid return transition.
- **AC-07:** The current state and recorded history are visible.
- **AC-08:** The UI does not report a transaction as confirmed before confirmation is observed.

## 4. Open decisions

- Whether `RETURNED` is a persistent state or immediately transitions to `AVAILABLE`.
- Whether receiving points are identified by wallet address, an allowlist, or another authorization mechanism.
- Whether a custom Script is required for the chosen state model or an existing-script prototype is sufficient for the initial spike.
- How the app queries item state and transaction history.
- Which user/borrower identifier is appropriate for the demo.
