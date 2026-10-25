# ReuseLoop - Implementation Plan (Draft)

## Week 9 - Scope and technical validation
- Finalize project proposal and requirements.
- Draft state machine and CKB architecture.
- Initialize repository and development environment.
- Run a small CKB connectivity/Cell transaction feasibility spike.
- Record unresolved design decisions.

## Week 10 - Core lifecycle
- Resolve Cell/state representation based on Week 9 findings.
- Implement item registration or initial item creation.
- Implement the core handoff and return transitions.
- Add authorization and invalid-transition tests.
- Define how current state and history are queried.

## Week 11 - User application and possible DOB/Spore extension
- Integrate wallet connection and signing.
- Build item lookup, QR, borrowing/returning, status and history UI.
- Stabilize end-to-end core workflow.
- Explore DOB/Spore only if it adds clear value and the core workflow is stable.

## Week 12 - Verification, release, and possible Fiber extension
- Test valid/invalid transitions, duplicate submissions, authorization failures, and RPC errors.
- Prepare demo flow and documentation.
- Explore Fiber for payment/settlement only if a real use case and funded Testnet setup are feasible.
- Document limitations and future work.

## Priority rule
The core borrow/return lifecycle takes priority over DOB/Spore and Fiber. Optional integrations must not cause the core MVP to miss its acceptance criteria.
