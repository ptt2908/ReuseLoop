# Week 12 Report - ReuseLoop UI Hardening, Verification, and Demo Readiness
**Reporting period:** 9 Nov - 15 Nov 2026 (final closeout: 17 Nov 2026)
**Participant:** Pham Tan Thinh
**Program:** CKB Builder Track

## 1. Courses and Topics Completed

During Week 12, I focused on improving the ReuseLoop demo workflow, hardening transaction submission feedback, verifying the app and contract suites, and preparing setup and peer-review documentation.

The main topics covered were:

- **Primary Workflow and Interface Structure**
  - Moving item lookup and lifecycle actions ahead of Admin and Diagnostics sections.
  - Keeping transaction preview, wallet signing, submission, and confirmation as separate steps.
  - Making pending, confirmed, and uncertain transaction outcomes distinguishable.
- **Submission Safety and Error Handling**
  - Preventing duplicate submission attempts while an operation is active.
  - Checking transaction inputs before broadcast and clearing a signature if an input is stale.
  - Distinguishing failures before broadcast from uncertain outcomes after broadcast.
- **Verification and Testnet Evidence**
  - Adding focused app tests for submission and confirmation behavior.
  - Reusing confirmed Week 11 Testnet transactions rather than creating duplicate transactions.
  - Capturing the current AVAILABLE lookup and lifecycle history.
- **Developer Setup and Project Limitations**
  - Documenting pinned contract artifact hashes, setup steps, and refresh behavior.
  - Clarifying the single-administrator authorization model and the boundary between on-chain state and physical-world activity.
  - Recording unresolved risks and future work for team review.

## 2. Exercises and Practical Work

### 2.1. Primary Item Workflow Redesign

Reorganized the interface so the main demo journey starts with item lookup and status, followed by the lifecycle panel. Admin / Registry setup and Cell Inspector Diagnostics are collapsed by default.

The wallet area was compacted for desktop use, and the deployment confirmation control is hidden while confirmation is being checked. The existing review, JoyID signing, and Testnet submission safeguards were preserved. Chrome review covered desktop and a narrow viewport of approximately 640 CSS pixels.

The user-requested per-cell expand/collapse control for long Cell Inspector Data hex remains deferred.

### 2.2. Submission-Safety Handling

Added a `submissionSafety` domain module and eight focused tests. The app now:

- Blocks starting a second lifecycle submission unless the state is ready and idle.
- Compares transaction hashes without case sensitivity.
- Checks signed transaction inputs before submission and clears a local signature when an input is no longer live.
- Labels a failure before broadcast as not submitted, and a failure after broadcast begins as uncertain.
- Separates confirmed, pending, and unavailable RPC confirmation results.

These checks do not authorize a transaction by themselves. JoyID signs locally, CKB Lock Scripts verify signatures, and Registry/State Type Scripts enforce on-chain invariants.

### 2.3. Existing Testnet Lifecycle and Evidence

Week 12 reused the existing Week 11 Testnet lifecycle. No new Testnet transaction was submitted during Week 12.

The reviewed item is `0x9f2e2c5938cb346f0d23e112bdf763ff21293285c98967afd181c261f09948c2`. The current live lookup returned `AVAILABLE`; the Registry ID is `0x3b1f5c7ca40ccb35c054e850185398cb2b07b0b7bbfbf1eed4ee1d6265790b60`, with item count 2.

| Operation | Result | Transaction hash | Confirmed block |
|---|---|---|---:|
| Registry Genesis | Registry initialized | [0x5ba65a14e5f393589812f2ed5dfb00db68d12dbc8498d5fcf66c98493809bd6f](https://pudge.explorer.nervos.org/transaction/0x5ba65a14e5f393589812f2ed5dfb00db68d12dbc8498d5fcf66c98493809bd6f) | 22,998,717 |
| Item registration | AVAILABLE | [0x851d6f428defd2871b0124ea3893249a24a2a30a09970463c4d2953381d71660](https://pudge.explorer.nervos.org/transaction/0x851d6f428defd2871b0124ea3893249a24a2a30a09970463c4d2953381d71660) | 22,999,622 |
| Borrow | BORROWED | [0xd88b93213ffcacac9ba691dae189f77c5a631380549246df2a8afb9fa4aa51a1](https://pudge.explorer.nervos.org/transaction/0xd88b93213ffcacac9ba691dae189f77c5a631380549246df2a8afb9fa4aa51a1) | 22,999,649 |
| Confirm Return | RETURNED | [0x3ac18f18c7831e44c369bae00282255adbfaa7f7b05e3bb572d062e0e5be2a1c](https://pudge.explorer.nervos.org/transaction/0x3ac18f18c7831e44c369bae00282255adbfaa7f7b05e3bb572d062e0e5be2a1c) | 22,999,692 |
| Make Available | AVAILABLE | [0x71574211cc4a674571b330ca9de6a1beb4215971d2e9911369e9e4865137a482](https://pudge.explorer.nervos.org/transaction/0x71574211cc4a674571b330ca9de6a1beb4215971d2e9911369e9e4865137a482) | 22,999,731 |

The Registry Genesis hash and block are recorded in the [Week 11 report](D:/CKB_prj/ckbuilder/ckb_weekly_report_w11.md) and are included here for traceability. Current UI evidence includes [the live AVAILABLE status](D:/CKB_prj/ReuseLoop/evidence/09_cp12_current_available_item.jpg) and [the recent lifecycle history](D:/CKB_prj/ReuseLoop/evidence/10_cp12_current_lifecycle_history.jpg). The read-only demo instructions are in [week12-demo.md](D:/CKB_prj/ReuseLoop/docs/week12-demo.md).

### 2.4. Setup, Architecture, and Report Updates

Updated the root README and app README with local run and verification commands. Added [developer-setup.md](D:/CKB_prj/ReuseLoop/docs/developer-setup.md) with the pinned Registry and State SHA-256 values, current local artifact paths, WSL build/checksum commands, and the requirement to reselect both files after reloading the app.

Extended [architecture.md](D:/CKB_prj/ReuseLoop/docs/architecture.md) to describe the one-administrator/JoyID model, Lock Script and Type Script responsibilities, manual lookup refresh, RPC/indexer dependencies, and current limitations. The demo walkthrough was corrected to require both verified binaries for lookup and lifecycle preview.

## 3. Verification Results

The following verification activities were completed:

| Verification | Result |
|---|---|
| Application test suite | 62 passed, 0 failed across 9 files |
| Targeted submission-safety tests | 8 added and passed |
| Application production build | Passed (`tsc -b` and Vite) |
| Contract integration suite | 77 passed, 0 failed (`make test` in WSL) |
| Live item lookup | Returned AVAILABLE and loaded the committed history after an initial delay |
| Registry Genesis and item lifecycle | Existing transactions verified in confirmed blocks listed above |
| Current artifact files | Registry and State files in `%TEMP%` matched the pinned SHA-256 hashes on 17 Nov 2026 |
| Browser fault injection | Not performed end to end for JoyID rejection, stale inputs, or RPC timeouts |
| Fresh RISC-V binary rebuild | Not performed during CP12.4; see toolchain issue below |

The contract suite covers valid lifecycle transitions, invalid transitions, duplicate registration, and valid, missing, or invalid administrator authorization. The eight app tests cover submission gating, hash matching, stale-input preflight, pre-broadcast versus uncertain outcomes, and confirmation polling results.

Vite continues to report a minified JavaScript chunk larger than its 500 kB warning threshold. The contract Makefile helper printed `Cannot find clang of version 16+!` although the WSL shell had reported Clang 18.1.3; `cargo test` nevertheless completed successfully. This test result does not prove a fresh RISC-V binary build will work.

## 4. Key Learnings

### 4.1. A Signature Is Not a Submission or Confirmation

A preview, local JoyID signature, network submission, and block confirmation are different transaction stages. An RPC error after broadcast starts must be treated as uncertain until the transaction hash is checked.

### 4.2. Lookup Is a Snapshot

The URL `?item=` parameter only pre-fills the ID. The user must click **Look up item** to query current Registry and State cells. After a confirmed lifecycle transaction, the lookup must be run again to refresh status and history. The Testnet indexer can delay the result.

### 4.3. Artifact Identity Matters

The frontend accepts the CP11 Registry and State binaries only when their SHA-256 values match the pinned hashes. The current files are in `%TEMP%`, which may be cleared, and the browser forgets the file selection after reload. A new build must match the pinned hashes before use.

### 4.4. On-Chain State Is Not Physical Proof

The single administrator's authorized transaction records the application's state change. It does not independently prove the package was handed over, returned, cleaned, inspected, or held by a particular person or store.

## 5. Challenges and Open Questions

### 5.1. Browser-Level Failure Coverage

Contract tests and app helper tests passed, but the browser did not simulate a rejected JoyID request, a consumed input, or a delayed/failed RPC response. Peer review should decide which of these flows are release-critical and should receive end-to-end coverage.

### 5.2. Artifact Recovery and Toolchain Reproducibility

The verified binaries are present in the current Temp folder, but that location is temporary and the app does not persist selected files. WSL reports Clang 18.1.3 while the contract Makefile helper still reports a Clang detection warning. The source build path is documented, but rebuilt artifacts were not produced and hash-checked during CP12.4.

### 5.3. RPC/Indexer Delay and Manual Refresh

The live lookup initially remained in a loading state and later returned successfully. Lookup and history are user-triggered and depend on the CKB Testnet RPC/indexer. The transaction screen can preserve an uncertain hash, but the user must inspect it before retrying.

### 5.4. Remaining UX and Performance Work

The Cell Inspector still renders long Data hex without a per-cell expand/collapse control. Vite's large-chunk warning also remains. Neither issue blocked the read-only demo, but both affect usability or load performance.

### 5.5. Not Completed in Week 12

- No browser-level end-to-end fault-injection suite was added.
- `npm run lint` was not recorded as run.
- A fresh RISC-V contract build was not verified against the pinned binary hashes.
- Per-store or borrower authorization, administrator recovery/rotation, and physical-world proof are not implemented.
- Fiber and DOB/Spore were not explored; they remain deferred optional extensions.

## 6. Plan for Next Week

Week 13 is planned for peer review with the team. The goal is to collect feedback before extending the MVP further.

Planned activities:

1. **Peer Review Preparation and Session**
   - Share this report, the demo guide, current screenshots, and transaction references.
   - Ask the team to review transaction safety, the one-administrator model, artifact setup, RPC/indexer behavior, and the UI's long Data display.
   - Record feedback with priority, owner, and acceptance criteria.
2. **Triage Review Findings**
   - Fix blockers and high-priority correctness or security issues first.
   - Decide whether browser-level transaction failure tests and binary build reproducibility are required before capstone completion.
3. **Possible 1–2 Week Capstone Extension**
   - Request an extension only after peer review confirms the needed scope and the program approves it.
   - If one extra week is available, prioritize review blockers and a reproducible artifact/toolchain path.
   - If two extra weeks are available, add browser-level failure coverage and then the highest-priority usability/performance improvements, such as per-cell Data collapse and bundle splitting.
4. **Keep Optional Integrations Gated**
   - Do not prioritize Fiber or DOB/Spore unless the team identifies a concrete use case and an independently verifiable plan.

------------------------------------------------------------------------

## Week 12 Final Result

Week 12 completed CP12.1 through CP12.4: the main workflow was reorganized, submission-safety handling and tests were added, the existing full Testnet lifecycle was reviewed and documented, and developer/demo instructions and the weekly report were completed. The app suite passed 62 tests, the contract suite passed 77 tests, and the production build succeeded.

The MVP remains a single-administrator Testnet prototype. Browser-level failure injection, a fresh verified RISC-V rebuild, and the per-cell Data control remain outstanding. The Testnet lookup can be delayed, the artifacts are stored in a temporary location, and the production bundle warning remains. Week 13 will begin with a team peer review; depending on its findings, a 1–2 week extension may be requested for prioritized fixes and verification.

