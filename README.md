# ReuseLoop

ReuseLoop is a CKB Testnet prototype for registering reusable packaging items and recording their `AVAILABLE → BORROWED → RETURNED → AVAILABLE` lifecycle. CKB scripts enforce Registry membership and permitted transitions; the app presents live cell state and transaction history.

## Run the app

Requirements: Node.js/npm compatible with the locked Vite dependencies, a browser, and network access to CKB Testnet RPC/indexer. Connect a JoyID CKB Testnet signer to prepare and sign transactions. The app does not need Docker.

From the repository root in PowerShell:

```powershell
Set-Location .\app
npm ci
npm run dev
```

Open the local URL printed by Vite, normally `http://localhost:5173`.

For the live lookup and lifecycle preview, open **Admin / Registry setup** and select the verified Registry and State binaries. The UI checks each SHA-256 before enabling lookup. The selected files stay in page memory and must be selected again after a reload. See [developer setup and artifact recovery](docs/developer-setup.md) for the current verified files, hashes, and WSL build recipe.

Do not initialize a second Registry or re-register the demo item on the existing Testnet deployment. Follow the [Week 12 demo guide](docs/week12-demo.md) to inspect the existing confirmed lifecycle without submitting a new transaction.

## Verification commands

Run these from `app`:

```powershell
npm test
npm run build
npm run lint
```

The Week 12 verified results were 62 app tests across 9 files and a successful production build. Vite reports an existing large JavaScript chunk warning. `npm run lint` is available but was not recorded as a Week 12 verification result.

The contract tests run in WSL from the contracts directory:

```powershell
wsl.exe -e bash -lc "cd /mnt/d/CKB_prj/ReuseLoop/reuse-loop-contracts && make test"
```

The Week 12 contract suite passed 77 tests. The Makefile helper printed a Clang detection warning, but `cargo test` completed successfully. See [the Week 12 report](ckb_weekly_report_w12.md) for exact scope and limits.

## Current authorization and scope

The MVP uses one administrator lock for Registry Genesis, item registration, and lifecycle transitions. JoyID signs transactions for the configured administrator; CKB Lock Scripts verify signatures and the Type Scripts enforce cell and state invariants. There are no separate borrower/store keys or multi-party authorization rules.

The chain records authorized state transitions. It does not prove physical handoff, return, cleaning, or inspection. Lookups use the CKB Testnet RPC/indexer, are user-triggered, and may be delayed while indexer data catches up. Fiber and DOB/Spore remain deferred extensions.

## Documents

- [Developer setup and artifact recovery](docs/developer-setup.md)
- [Week 12 demo and verification guide](docs/week12-demo.md)
- [Week 12 report](ckb_weekly_report_w12.md)
- [Architecture](docs/architecture.md)
- [Requirements](docs/requirements.md)
- [State machine](docs/state-machine.md)
- [Transaction design](docs/transaction-design.md)
- [Project proposal](docs/project-proposal.md)
