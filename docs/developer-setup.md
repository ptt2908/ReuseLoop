# Developer Setup and Artifact Recovery

This guide starts the app and prepares the exact contract artifacts required by the live item lookup. The project uses the public CKB Testnet and does not require Docker.

## Prerequisites

- Node.js and npm compatible with the versions locked in `app/package-lock.json`.
- Chrome or another modern browser.
- Network access to the CKB Testnet RPC and indexer.
- A JoyID CKB Testnet signer for transaction preview/signing. Read-only lookup can use the app's configured demo address, but a connected JoyID wallet is needed for the full administrator workflow.
- Registry and State contract binary files whose SHA-256 values match the pinned values below. The app requires them for lookup and lifecycle preview as well as deployment; selecting a file with the right name is not sufficient.

## Start the frontend

From the repository root in PowerShell:

```powershell
Set-Location .\app
npm ci
npm run dev
```

Open the local URL printed by Vite, normally `http://localhost:5173`.

Expand **Admin / Registry setup** and select one file in each binary field. The app validates SHA-256 in the browser before retaining the artifact. The selected `File` data is kept in React page state, not in local storage; select both files again after a page reload.

## Pinned CP11.1 artifacts

| Artifact | SHA-256 | Current local file |
|---|---|---|
| Registry | `C45C0EDE3B95BF4A447FEC1AC18E7756175755D7F832AB964503E0EE19BC5A88` | `%TEMP%\reuseloop-cp11-verified-stripped\registry` |
| State | `376A80EE27C79ACC762AB1D828F17AA15499F41B05C2C07067F4429032418A03` | `%TEMP%\reuseloop-cp11-verified-stripped\state` |

Both files were present on this machine and matched those hashes on 2026-11-17. `%TEMP%` may be cleared by Windows, so keep a verified local backup outside the temporary directory. Do not change the hashes pinned in `app/src/domain/deploymentPreview.ts` to make a different binary pass.

You can verify the current files in PowerShell with:

```powershell
Get-FileHash -Algorithm SHA256 "$env:TEMP\reuseloop-cp11-verified-stripped\registry"
Get-FileHash -Algorithm SHA256 "$env:TEMP\reuseloop-cp11-verified-stripped\state"
```

## Rebuild from the checked-in contract sources

The contract Makefiles build the stripped binaries into `reuse-loop-contracts/build/release/registry` and `reuse-loop-contracts/build/release/state`. From PowerShell, the WSL commands are:

```powershell
wsl.exe -e bash -lc "cd /mnt/d/CKB_prj/ReuseLoop/reuse-loop-contracts && make prepare && make build"
wsl.exe -e bash -lc "cd /mnt/d/CKB_prj/ReuseLoop/reuse-loop-contracts && sha256sum build/release/registry build/release/state"
```

`make build` clears and recreates the release build directory. It needs the pinned Rust RISC-V target and the LLVM/Clang tools used by the contract Makefiles. Only use the rebuilt files if their SHA-256 values match the pinned CP11.1 values above; otherwise stop and resolve the artifact mismatch before lookup or deployment.

The WSL environment previously reported `clang --version` as 18.1.3, while the contract Makefile's helper reported that it could not find Clang 16+ during `make test`. That helper probes `llvm-strip` and related tools, so the test warning does not establish that the RISC-V build toolchain is ready. The Week 12 report records the contract tests as passing; a fresh contract-binary rebuild was not part of CP12.4 verification.

## Run and verify

From `app`:

```powershell
npm test
npm run build
```

For the existing demo, open the page with the item query parameter, select both verified artifacts, connect the administrator JoyID wallet for lifecycle preview/signing, and click **Look up item**. The query parameter pre-fills the item ID but does not trigger a lookup. After a confirmed lifecycle transaction, click **Look up item** again to refresh the displayed status and history.

Use the [Week 12 demo guide](week12-demo.md) for the current item, confirmed transaction references, read-only walkthrough, and recovery steps for delayed RPC/indexer results. Do not submit a new Testnet transaction just to reproduce the existing demo.
