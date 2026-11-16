# Week 12 Demo and Verification

Last checked: 2026-11-17 (Asia/Bangkok)

## Demo scope

This walkthrough reuses the existing CKB Testnet Registry and the confirmed Week 11 item lifecycle. It is a read-only demo: it does not initialize a second Registry, register a duplicate item, or submit another lifecycle transaction. The current live lookup returns the item as `AVAILABLE` and reports the prior lifecycle transactions as committed.

## Prerequisites

- The ReuseLoop repository and app dependencies are available.
- Chrome can open the local Vite app.
- JoyID is connected to CKB Testnet for wallet-dependent previews. Do not enter a seed phrase or private key in the app.
- CKB Testnet RPC and indexer are reachable. Lookup can take longer while the indexer catches up.
- Select both CP11.1 Registry and State binaries before lookup or lifecycle preview; the app requires them to validate the current on-chain scripts, not only for deployment. Their pinned hashes and current local paths are in [developer setup](developer-setup.md). The live Registry is already initialized, so do not deploy or initialize it again.

## Start the app

From the repository root in PowerShell:

```powershell
cd app
npm install
npm run dev
```

Open the local URL printed by Vite (normally `http://localhost:5173`). If the app is already running, reuse that instance.

## Walkthrough

1. Confirm the app shows `CKB Testnet` and the connected JoyID address.
2. Explain Registry Genesis as the one-time initialization that created the existing Registry. In this live instance, verify the current Registry ID (`0x3b1f5c7ca40ccb35c054e850185398cb2b07b0b7bbfbf1eed4ee1d6265790b60`) and item count (`2`) in the lookup result. Do not build or submit Genesis again. The current evidence captures the live Registry and its canonical cell, but not a verified Genesis transaction hash; this guide intentionally does not guess one.
3. In **CP11.6 — Item Lookup and Status**, look up item ID `0x9f2e2c5938cb346f0d23e112bdf763ff21293285c98967afd181c261f09948c2`.
4. Confirm the displayed status is `AVAILABLE`, and show the live State cell and canonical Registry cell. A successful result means the app found current live Testnet cells; it is not proof of a physical handoff. The URL query parameter only pre-fills the Item ID; click **Look up item** to fetch current data.
5. Scroll to **Recent on-chain history** and review the confirmed lifecycle in order:

   | Action | Result | Block | Transaction |
   |---|---|---:|---|
   | Item registered | AVAILABLE | 22999622 | [0x851d6f428defd2871b0124ea3893249a24a2a30a09970463c4d2953381d71660](https://pudge.explorer.nervos.org/transaction/0x851d6f428defd2871b0124ea3893249a24a2a30a09970463c4d2953381d71660) |
   | Borrow | BORROWED | 22999649 | [0xd88b93213ffcacac9ba691dae189f77c5a631380549246df2a8afb9fa4aa51a1](https://pudge.explorer.nervos.org/transaction/0xd88b93213ffcacac9ba691dae189f77c5a631380549246df2a8afb9fa4aa51a1) |
   | Confirm return | RETURNED | 22999692 | [0x3ac18f18c7831e44c369bae00282255adbfaa7f7b05e3bb572d062e0e5be2a1c](https://pudge.explorer.nervos.org/transaction/0x3ac18f18c7831e44c369bae00282255adbfaa7f7b05e3bb572d062e0e5be2a1c) |
   | Make available | AVAILABLE | 22999731 | [0x71574211cc4a674571b330ca9de6a1beb4215971d2e9911369e9e4865137a482](https://pudge.explorer.nervos.org/transaction/0x71574211cc4a674571b330ca9de6a1beb4215971d2e9911369e9e4865137a482) |

6. To demonstrate the next lifecycle step without submitting, enter the same Item ID in **CP11.7 — Borrow and Return Lifecycle**, build the preview, and review it. Stop before JoyID signing or Testnet submission. A separate confirmation is required before any new on-chain transaction.
7. Close by stating the boundary: the chain records the authorized state transition; it does not independently prove who physically handed over, returned, cleaned, or inspected the package.

Lookup and history are user-triggered and are not refreshed in the background. After a confirmed lifecycle transaction, click **Look up item** again to refresh the card and history. If lookup remains at “Looking up live cells,” wait for the RPC/indexer and retry the read-only lookup. If a transaction hash is visible but confirmation is delayed or unavailable, check that hash in the explorer before taking any further action; do not blindly resubmit. The CP12 browser review first showed a delayed lookup, then successfully loaded the live state and history.

## Evidence

- [09 — Current item status and live cells](../evidence/09_cp12_current_available_item.jpg)
- [10 — Current item and recent lifecycle history](../evidence/10_cp12_current_lifecycle_history.jpg)
- [05 — Earlier lookup failure example](../evidence/05_evidence_lookup_item_failed.png)
- [06 — Week 11 item AVAILABLE](../evidence/06_evidence_lookup_item_available.png)
- [07 — Week 11 item BORROWED](../evidence/07_evidence_lookup_item_borrowed.png)
- [08 — Week 11 item RETURNED](../evidence/08_evidence_lookup_item_returned.png)

The lookup reads through CCC `ClientPublicTestnet`, searches cells under the configured administrator lock, rechecks that candidate Registry/State cells are live, and loads up to 20 recent State transactions. An indexer delay or RPC failure can therefore delay either the current status or the history panel.

## CP12.2 verification record

| Check | Result |
|---|---|
| App suite (`npm test`) | Passed: 62 tests across 9 files. |
| App build (`npm run build`) | Passed (`tsc -b` and Vite); Vite still reports the existing large-chunk warning. |
| Contract suite (`make test` in `reuse-loop-contracts`, via WSL) | Passed: 77 tests, 0 failed. The test helper printed a Clang 16+ detection warning, but `cargo test` completed successfully. |
| Submission-state checks | Added 8 focused tests for duplicate-submit gating, transaction-hash matching, stale-input preflight, pre-send vs post-send failures, confirmed/pending outcomes, and RPC polling errors. |
| Live item evidence | The current lookup showed `AVAILABLE` and the existing lifecycle in committed blocks. No transaction was sent during CP12.2/12.3. |

The contract suite covers successful lifecycle execution and rejects invalid transitions, duplicate registration, and missing or invalid admin authorization. The new app tests exercise the submission-safety helpers used by the UI. RPC/indexer timeout behavior was checked at the helper level and an actual delayed lookup was observed; wallet rejection, stale inputs, and network timeouts were not fault-injected end to end in the browser. Treat those as test-scope limits, not as additional live Testnet results.

## Earlier deployment reference

The CP11.3 contract deployment was previously verified as committed in block `23084185`: [0xdc8890aa1b904b3037095825e02982e7dc4a5aae25fbbc06cc348945f6aced9c](https://pudge.explorer.nervos.org/transaction/0xdc8890aa1b904b3037095825e02982e7dc4a5aae25fbbc06cc348945f6aced9c). This is separate from Registry Genesis; the Genesis transaction hash is not asserted in this guide.



