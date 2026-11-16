# ReuseLoop App

This directory contains the React, TypeScript, and Vite client. It connects directly to CKB Testnet through CCC; there is no application server.

For prerequisites, verified contract artifacts, wallet setup, and the end-to-end demo, see the repository [developer setup](../docs/developer-setup.md), [Week 12 demo guide](../docs/week12-demo.md), and [README](../README.md).

## Start locally

```powershell
npm ci
npm run dev
```

## Available scripts

```powershell
npm test
npm run build
npm run lint
```

The Week 12 record reports 62 passing app tests and a successful build. The build still emits Vite's large-chunk warning; lint is available but was not recorded as run for Week 12.

## CKB Testnet notes

- Select both verified Registry and State binaries in **Admin / Registry setup**. The UI checks the pinned SHA-256 values.
- Artifact selection is held in the current page session and must be repeated after reload.
- A URL `?item=<32-byte-id>` pre-fills the lookup field; click **Look up item** to query current cells and history.
- The app currently uses the public CKB Testnet and JoyID CKB signer path. It does not verify physical-world activity.
