# ReuseLoop - Reusable Packaging Return System

ReuseLoop is a CKB-based capstone prototype for recording and verifying the lifecycle of reusable packaging. The initial scenario allows an item borrowed at Point A to be returned at Point B.

## Week 9 status

This repository starter contains the initial product scope and planning documents. It is a design baseline, not evidence that on-chain functionality has been implemented.

## MVP goals

- Identify reusable packaging with a unique item ID and QR-based lookup.
- Record handoff from an issuing point to a borrower.
- Allow an authorized receiving point to confirm return, including a different point from the pickup location.
- Validate permitted state transitions and reject invalid or duplicate operations.
- Display current state and transaction history.

## Planned technology direction

- CKB Testnet for on-chain state and transaction records.
- CCC and JoyID for wallet connection and transaction signing, subject to architecture validation.
- React + TypeScript + Vite for the frontend.
- DOB/Spore and Fiber are potential extensions for Weeks 11-12, after the core workflow is stable.

## Scope boundaries

The MVP does not require real-store deployment, a full POS system, hardware NFC anti-counterfeit protection, or a generic Digital Product Passport.

## Documents

- [Project proposal](docs/project-proposal.md)
- [Requirements](docs/requirements.md)
- [State machine](docs/state-machine.md)
- [Architecture draft](docs/architecture.md)
- [Implementation plan](docs/implementation-plan.md)

## Current status

Planning and design only. Do not describe CKB transactions, scripts, wallet integration, DOB, or Fiber as implemented until they have been built and verified.
