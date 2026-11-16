# CKB Weekly Report — Week 12

Reporting date: 2026-11-17 (Asia/Bangkok)
Repository: `D:\CKB_prj\ReuseLoop`

## Summary

CP12.1 through CP12.4 are complete. The Week 12 work reorganized the primary item workflow, added and verified submission-safety handling, prepared a read-only Testnet demo, and updated developer/setup/architecture documentation. The app and contract suites passed at CP12.2. CP12.4 changed documentation only; it did not change contract or frontend code and did not rerun tests.

No new Testnet transaction was submitted during Week 12. The demo uses the existing confirmed Week 11 deployment and lifecycle. Optional CP12.5 (Fiber feasibility) remains deferred because the current MVP has no recorded payment/settlement use case or funded Testnet plan.

## Checkpoint status

| Checkpoint | Result |
|---|---|
| CP12.1 — UI redesign | Complete. The lookup/lifecycle journey is primary; Admin/Registry setup and Diagnostics are collapsed by default. Preview, wallet signing, submission, confirmation, and uncertain outcomes remain separate. |
| CP12.2 — Core and failure-path verification | Complete with scope limits recorded. App tests: 62 passed across 9 files. Contract tests: 77 passed, 0 failed. Production build passed with Vite's existing large-chunk warning. |
| CP12.3 — Demo and evidence | Complete. The walkthrough reuses confirmed Testnet data, links lifecycle hashes, and includes current UI screenshots. The exact Registry Genesis transaction hash is not claimed because it was not identified in the evidence reviewed. |
| CP12.4 — Documentation closeout | Complete. Setup, artifact verification/recovery, architecture and authorization, operational limits, future work, and this report are documented. |
| CP12.5 — Optional Fiber feasibility | Deferred. Revisit only if a concrete settlement use case and a feasible funded Testnet setup are defined. DOB/Spore also remains future work. |

## Verification record

- `app`: `npm test` passed **62 tests across 9 files**, including 8 targeted submission-safety tests.
- `app`: `npm run build` passed (`tsc -b` and Vite). Vite reported that a minified JavaScript chunk is larger than 500 kB.
- `reuse-loop-contracts`: WSL `make test` passed **77 tests, 0 failed**. The Makefile helper printed `Cannot find clang of version 16+!`; the WSL shell had previously reported Clang 18.1.3, and `cargo test` still completed successfully. The helper probes LLVM strip tools, so a fresh RISC-V contract build has not been inferred from this test result.
- The lifecycle contract suite covers valid full-cycle transitions, invalid transitions, duplicate registration, and valid/missing/invalid administrator authorization.
- The added app tests cover duplicate-submit gating, transaction-hash matching, stale-input preflight, pre-send versus uncertain post-send failures, confirmed/pending confirmation, and RPC polling failure.
- Browser review first showed a delayed `Looking up live cells…` query and later loaded the live item and committed history. Browser fault injection for JoyID rejection, consumed inputs, and RPC timeouts was not performed end to end.

## Existing Testnet evidence

The reviewed item is `0x9f2e2c5938cb346f0d23e112bdf763ff21293285c98967afd181c261f09948c2`. Its live status was `AVAILABLE`; the Registry ID was `0x3b1f5c7ca40ccb35c054e850185398cb2b07b0b7bbfbf1eed4ee1d6265790b60`, with item count 2.

| Existing event | Block | Transaction |
|---|---:|---|
| CP11.3 contract deployment | 23084185 | [0xdc8890aa1b904b3037095825e02982e7dc4a5aae25fbbc06cc348945f6aced9c](https://pudge.explorer.nervos.org/transaction/0xdc8890aa1b904b3037095825e02982e7dc4a5aae25fbbc06cc348945f6aced9c) |
| Item registered → AVAILABLE | 22999622 | [0x851d6f428defd2871b0124ea3893249a24a2a30a09970463c4d2953381d71660](https://pudge.explorer.nervos.org/transaction/0x851d6f428defd2871b0124ea3893249a24a2a30a09970463c4d2953381d71660) |
| Borrow → BORROWED | 22999649 | [0xd88b93213ffcacac9ba691dae189f77c5a631380549246df2a8afb9fa4aa51a1](https://pudge.explorer.nervos.org/transaction/0xd88b93213ffcac9ba691dae189f77c5a631380549246df2a8afb9fa4aa51a1) |
| Confirm return → RETURNED | 22999692 | [0x3ac18f18c7831e44c369bae00282255adbfaa7f7b05e3bb572d062e0e5be2a1c](https://pudge.explorer.nervos.org/transaction/0x3ac18f18c7831e44c369bae00282255adbfaa7f7b05e3bb572d062e0e5be2a1c) |
| Make available → AVAILABLE | 22999731 | [0x71574211cc4a674571b330ca9de6a1beb4215971d2e9911369e9e4865137a482](https://pudge.explorer.nervos.org/transaction/0x71574211cc4a674571b330ca9de6a1beb4215971d2e9911369e9e4865137a482) |

The current lookup and history screenshots are [item status](evidence/09_cp12_current_available_item.jpg) and [lifecycle history](evidence/10_cp12_current_lifecycle_history.jpg). Earlier Week 11 screenshots 05–08 are linked from the [demo guide](docs/week12-demo.md). The current evidence identifies the live Registry ID/cell, but not the Registry Genesis transaction hash; no Genesis hash is guessed here.

## Authorization model

The MVP uses one administrator lock for Registry Genesis, item registration, and all lifecycle transitions. The frontend currently supports a JoyID CKB Testnet signer for this workflow. The CKB Lock Script verifies the administrator signature; Registry/State Type Scripts validate the configured lock hash, cell linkage, and permitted state changes. Frontend owner matching is an additional user-facing check, not the security boundary. There are no borrower/store-specific keys, multi-party authorization, or administrator recovery/rotation workflow.

## Runtime dependencies and limitations

- Both Registry and State binary files must be selected and pass their pinned SHA-256 checks for live lookup and lifecycle preview. The current files are in `%TEMP%\reuseloop-cp11-verified-stripped\registry` and `...\state`; both matched their pinned values on 2026-11-17. Browser selection is held in page memory and must be repeated after reload. See [developer setup and artifact recovery](docs/developer-setup.md).
- A source rebuild is documented in developer setup, but it was not run for CP12.4. The contract Makefile's Clang/LLVM helper warning needs resolution before treating a new build as a verified replacement.
- Live status and history depend on the public CKB Testnet RPC and indexer. Lookup is manual; `?item=` only fills the ID field. After a transaction is confirmed, click **Look up item** to refresh. The history query returns at most 20 recent State transactions.
- If submission was sent but block confirmation could not be checked, the outcome is uncertain. Preserve and inspect the transaction hash before considering any retry.
- The chain records an administrator-authorized transition. It cannot independently prove physical handoff, return, cleaning, inspection, custody, or package condition.
- The Cell Inspector renders raw Data hex; the requested per-cell expand/collapse control remains deferred.
- The production build has an existing >500 kB JavaScript chunk warning.

## Future work

1. Add the user-requested per-cell Data expand/collapse control.
2. Add browser-level tests for JoyID rejection, duplicate clicks, stale inputs, delayed indexer responses, and timeout/uncertain confirmation recovery.
3. Fix or clarify the contract build helper's Clang/LLVM tool detection; verify rebuilt binaries against the pinned hashes before use.
4. Consider frontend code splitting to reduce the large Vite chunk.
5. If the product needs multiple stores or customers to attest actions, design and test a multi-party authorization model. Treat any physical verification system as a separate feature; the chain alone cannot establish physical events.
6. Keep Fiber and DOB/Spore deferred until the core workflow is complete and a specific use case and verification plan exist.
7. Capture and independently verify the Registry Genesis transaction reference if it is needed in future evidence.

## Reproduction links

- [Developer setup](docs/developer-setup.md)
- [Week 12 demo guide](docs/week12-demo.md)
- [Architecture](docs/architecture.md)
- [Week 12 roadmap](C:/Users/ASUS/Documents/ChatGPT/CKBuilder/week-12-roadmap.md) (workspace copy)

