import { useState } from "react";
import { ccc } from "@ckb-ccc/ccc";
import RegistryRegistrationPanel from "./RegistryRegistrationPanel";
import { connectJoyIdTestnet, type WalletSession } from "./domain/wallet";
import {
  buildDeploymentPreview,
  verifyContractBinary,
  type ContractBinaryKind,
  type DeploymentPreview,
  type VerifiedContractBinary,
} from "./domain/deploymentPreview";
import {
  buildRegistryGenesisPreview,
  type RegistryGenesisPreview,
} from "./domain/registryGenesisPreview";

const TESTNET_ADDRESS =
  "ckt1qrfrwcdnvssswdwpn3s9v8fp87emat306ctjwsm3nmlkjg8qyza2cqgqq8lu0rg7yvxngy0nwzqtvmzt2ygkgx09rqkk6css";

type CellInfo = {
  txHash: string;
  index: string;
  capacity: string;
  lock: unknown;
  type: unknown;
  data: string;
};

function formatCkb(shannons: string): string {
  const value = BigInt(shannons);
  const whole = value / 100_000_000n;
  const fraction = (value % 100_000_000n).toString().padStart(8, "0");
  return whole.toString() + "." + fraction;
}

export default function App() {
  const [cells, setCells] = useState<CellInfo[]>([]);
  const [status, setStatus] = useState("Ready");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [wallet, setWallet] = useState<WalletSession | null>(null);
  const [walletBusy, setWalletBusy] = useState(false);
  const [registryArtifact, setRegistryArtifact] =
    useState<VerifiedContractBinary | null>(null);
  const [stateArtifact, setStateArtifact] =
    useState<VerifiedContractBinary | null>(null);
  const [artifactStatus, setArtifactStatus] = useState("");
  const [artifactBusy, setArtifactBusy] = useState(false);
  const [deploymentPreview, setDeploymentPreview] =
    useState<DeploymentPreview | null>(null);
  const [deploymentBusy, setDeploymentBusy] = useState(false);
  const [deploymentError, setDeploymentError] = useState("");
  const [genesisPreview, setGenesisPreview] =
    useState<RegistryGenesisPreview | null>(null);
  const [genesisBusy, setGenesisBusy] = useState(false);
  const [genesisError, setGenesisError] = useState("");
  const [genesisStatus, setGenesisStatus] = useState("");
  const [genesisSignedTransaction, setGenesisSignedTransaction] =
    useState<ccc.Transaction | null>(null);
  const [genesisSigningBusy, setGenesisSigningBusy] = useState(false);
  const [genesisSigningError, setGenesisSigningError] = useState("");
  const [genesisSigningStatus, setGenesisSigningStatus] = useState("");
  const [signedTransaction, setSignedTransaction] = useState<ccc.Transaction | null>(null);
  const [signingBusy, setSigningBusy] = useState(false);
  const [signingError, setSigningError] = useState("");
  const [signingStatus, setSigningStatus] = useState("");
  const [confirmationBusy, setConfirmationBusy] = useState(false);

  async function queryCells() {
    setLoading(true);
    setError("");
    setCells([]);
    setStatus("Querying CKB Testnet...");

    const owner = ccc.ClientPublicTestnet.open();

    try {
      const client = owner.value;
      const address = await ccc.Address.fromString(
        wallet?.address ?? TESTNET_ADDRESS,
        client,
      );

      const results: CellInfo[] = [];

      for await (const cell of client.findCellsByLock(
        address.script,
        undefined,
        true,
        "desc",
        10,
      )) {
        results.push({
          txHash: cell.outPoint.txHash,
          index: String(cell.outPoint.index),
          capacity: String(cell.cellOutput.capacity),
          lock: cell.cellOutput.lock,
          type: cell.cellOutput.type ?? null,
          data: cell.outputData,
        });
      }

      setCells(results);
      setStatus("Found " + results.length + " live Cell(s).");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setStatus("Query failed");
    } finally {
      await owner.dispose();
      setLoading(false);
    }
  }

  async function connectWallet() {
    setWalletBusy(true);
    setError("");
    setStatus("Connecting JoyID on CKB Testnet...");

    try {
      const session = await connectJoyIdTestnet();
      setWallet(session);
      setStatus("Wallet connected.");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setStatus("Wallet connection failed");
    } finally {
      setWalletBusy(false);
    }
  }

  async function disconnectWallet() {
    if (!wallet || signingBusy || genesisSigningBusy) {
      return;
    }

    const session = wallet;
    setWallet(null);
    setDeploymentPreview(null);
    setGenesisPreview(null);
    setGenesisStatus("");
    setGenesisError("");
    setGenesisSignedTransaction(null);
    setGenesisSigningStatus("");
    setGenesisSigningError("");
    setSignedTransaction(null);
    setSigningStatus("");
    setSigningError("");
    setWalletBusy(true);
    setError("");

    try {
      await session.disconnect();
      setStatus("Wallet disconnected.");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setStatus("Wallet disconnect failed");
    } finally {
      setWalletBusy(false);
    }
  }

  async function selectArtifact(
    kind: ContractBinaryKind,
    file: File | undefined,
  ) {
    if (
      genesisSigningBusy ||
      genesisSignedTransaction ||
      genesisSubmissionComplete ||
      genesisSubmissionOutcomeUncertain
    ) {
      return;
    }
    setDeploymentPreview(null);
    setGenesisPreview(null);
    setGenesisStatus("");
    setGenesisError("");
    setGenesisSignedTransaction(null);
    setGenesisSigningStatus("");
    setGenesisSigningError("");
    setSignedTransaction(null);
    setSigningStatus("");
    setSigningError("");
    setDeploymentError("");

    if (!file) {
      return;
    }

    if (kind === "registry") {
      setRegistryArtifact(null);
    } else {
      setStateArtifact(null);
    }
    setArtifactBusy(true);
    setArtifactStatus("Checking " + kind + " binary SHA-256...");
    try {
      const verified = await verifyContractBinary(file, kind);
      if (kind === "registry") {
        setRegistryArtifact(verified);
      } else {
        setStateArtifact(verified);
      }
      setArtifactStatus(kind + " binary verified against CP11.1.");
    } catch (err) {
      if (kind === "registry") {
        setRegistryArtifact(null);
      } else {
        setStateArtifact(null);
      }
      setArtifactStatus("");
      setDeploymentError(err instanceof Error ? err.message : String(err));
    } finally {
      setArtifactBusy(false);
    }
  }

  async function createDeploymentPreview() {
    if (!wallet || !registryArtifact || !stateArtifact) {
      return;
    }

    setDeploymentBusy(true);
    setDeploymentError("");
    setDeploymentPreview(null);
    setSignedTransaction(null);
    setSigningStatus("");
    setSigningError("");
    const owner = ccc.ClientPublicTestnet.open();

    try {
      const preview = await buildDeploymentPreview(
        owner.value,
        wallet.address,
        registryArtifact,
        stateArtifact,
      );
      setDeploymentPreview(preview);
      setArtifactStatus("Unsigned Testnet deployment preview is ready.");
    } catch (err) {
      setDeploymentError(err instanceof Error ? err.message : String(err));
      setArtifactStatus("Could not build the deployment preview.");
    } finally {
      await owner.dispose();
      setDeploymentBusy(false);
    }
  }

  async function createRegistryGenesisPreview() {
    if (
      !wallet ||
      !registryArtifact ||
      !stateArtifact ||
      genesisBusy ||
      genesisSigningBusy ||
      genesisSignedTransaction ||
      genesisSubmissionComplete ||
      genesisSubmissionOutcomeUncertain
    ) {
      return;
    }

    setGenesisBusy(true);
    setGenesisError("");
    setGenesisPreview(null);
    setGenesisSignedTransaction(null);
    setGenesisSigningStatus("");
    setGenesisSigningError("");
    setGenesisStatus("Building an unsigned Registry Genesis preview from live Testnet cells...");
    const owner = ccc.ClientPublicTestnet.open();

    try {
      const preview = await buildRegistryGenesisPreview(
        owner.value,
        wallet.address,
        registryArtifact,
        stateArtifact,
      );
      setGenesisPreview(preview);
      setGenesisStatus("Unsigned Registry Genesis preview is ready. It has not been signed or submitted.");
    } catch (err) {
      setGenesisError(err instanceof Error ? err.message : String(err));
      setGenesisStatus("Could not build the Registry Genesis preview.");
    } finally {
      try {
        await owner.dispose();
      } catch {
        // Client cleanup does not change the preview result.
      }
      setGenesisBusy(false);
    }
  }

  async function signRegistryGenesisPreview() {
    if (
      !wallet ||
      !genesisPreview ||
      genesisSignedTransaction ||
      genesisSigningBusy ||
      genesisBusy
    ) {
      return;
    }
    if (wallet.address !== genesisPreview.ownerAddress) {
      setGenesisSigningStatus("");
      setGenesisSigningError(
        "The connected wallet differs from the Registry Genesis preview owner. Rebuild the preview first.",
      );
      return;
    }

    setGenesisSigningBusy(true);
    setGenesisSigningError("");
    setGenesisSigningStatus("Review the Registry Genesis signing request in JoyID...");
    const rawTransactionHash = genesisPreview.transaction.hash();

    try {
      const signed = await wallet.signTransaction(genesisPreview.transaction);
      if (signed.hash().toLowerCase() !== rawTransactionHash.toLowerCase()) {
        throw new Error(
          "JoyID changed the Registry Genesis transaction fields. Do not use this signed result; rebuild and review a fresh preview.",
        );
      }
      if (!signed.witnesses.some((witness) => String(witness).toLowerCase() !== "0x")) {
        throw new Error("JoyID returned no signature witness for Registry Genesis.");
      }

      setGenesisSignedTransaction(signed);
      setGenesisSigningStatus(
        "JoyID signed the Registry Genesis transaction. It remains local and has not been submitted.",
      );
    } catch (err) {
      setGenesisSigningStatus("");
      setGenesisSigningError(err instanceof Error ? err.message : String(err));
    } finally {
      setGenesisSigningBusy(false);
    }
  }

  async function submitSignedRegistryGenesis() {
    if (
      !wallet ||
      !genesisPreview ||
      !genesisSignedTransaction ||
      genesisBusy ||
      genesisSigningBusy ||
      genesisSubmissionInProgress ||
      genesisSubmissionOutcomeUncertain ||
      genesisSubmissionComplete
    ) {
      return;
    }

    if (wallet.address !== genesisPreview.ownerAddress) {
      setGenesisSigningStatus("");
      setGenesisSigningError(
        "The connected wallet differs from the Registry Genesis preview owner. Rebuild and sign a fresh preview.",
      );
      return;
    }

    const expectedHash = genesisPreview.transaction.hash();
    if (genesisSignedTransaction.hash().toLowerCase() !== expectedHash.toLowerCase()) {
      setGenesisSigningStatus("");
      setGenesisSigningError(
        "The signed Registry Genesis transaction no longer matches the reviewed preview. Do not submit it.",
      );
      return;
    }

    if (
      !genesisSignedTransaction.witnesses.some(
        (witness) => String(witness).toLowerCase() !== "0x",
      )
    ) {
      setGenesisSigningStatus("");
      setGenesisSigningError(
        "The signed Registry Genesis transaction has no signature witness. Sign the preview again before submitting.",
      );
      return;
    }

    setGenesisSigningError("");
    const confirmed = window.confirm(
      [
        "Submit this signed Registry Genesis transaction to CKB Testnet?",
        "This sends the transaction as reviewed and does not request another JoyID signature.",
        "",
        "Registry ID: " + genesisPreview.registryId,
        "Registry Type Script hash: " + genesisPreview.registryTypeScriptHash,
        "Seed input: " + genesisPreview.seedOutPoint,
        "Seed capacity: " + formatCkb(genesisPreview.seedCapacity) + " CKB",
        "Registry cell capacity: " + formatCkb(genesisPreview.registryCapacity) + " CKB",
        "Estimated fee: " + formatCkb(genesisPreview.fee) + " CKB at " + genesisPreview.feeRate + " shannons/KB",
        "Change: " + formatCkb(genesisPreview.change) + " CKB",
        "Raw transaction hash: " + expectedHash,
        "",
        "Once accepted by a Testnet node, this transaction cannot be canceled.",
        "Confirm the Registry ID and amounts before continuing.",
      ].join("\n"),
    );

    if (!confirmed) {
      setGenesisSigningStatus(
        "Submission cancelled. The signed Registry Genesis transaction remains local.",
      );
      return;
    }

    setGenesisSigningBusy(true);
    setGenesisSigningStatus("Submitting Registry Genesis to CKB Testnet...");
    let clientOwner: ReturnType<typeof ccc.ClientPublicTestnet.open> | undefined;

    try {
      clientOwner = ccc.ClientPublicTestnet.open();
      const returnedHash = String(
        await clientOwner.value.sendTransaction(genesisSignedTransaction),
      );

      if (returnedHash.toLowerCase() !== expectedHash.toLowerCase()) {
        setGenesisSigningStatus(
          "Registry Genesis submission outcome uncertain. RPC returned " +
            returnedHash +
            " instead of " +
            expectedHash +
            ". Check both hashes before retrying.",
        );
        return;
      }

      setGenesisSigningStatus(
        "Registry Genesis submitted to CKB Testnet; awaiting block confirmation. RPC transaction hash: " +
          returnedHash,
      );
      try {
        const confirmedTransaction = await clientOwner.value.waitTransaction(
          returnedHash,
          0,
          120_000,
          2_000,
        );
        if (confirmedTransaction?.blockNumber != null) {
          setGenesisSigningStatus(
            "Registry Genesis confirmed on CKB Testnet in block " +
              String(confirmedTransaction.blockNumber) +
              ". RPC transaction hash: " +
              returnedHash,
          );
        } else {
          setGenesisSigningStatus(
            "Registry Genesis submitted to CKB Testnet, but confirmation is not available yet. Check transaction hash " +
              returnedHash +
              " before retrying.",
          );
        }
      } catch {
        setGenesisSigningStatus(
          "Registry Genesis submitted to CKB Testnet, but confirmation could not be verified yet. Check transaction hash " +
            returnedHash +
            " before retrying.",
        );
      }
    } catch (err) {
      setGenesisSigningStatus(
        "Registry Genesis submission outcome uncertain. Check transaction hash " +
          expectedHash +
          " before retrying.",
      );
      setGenesisSigningError(err instanceof Error ? err.message : String(err));
    } finally {
      if (clientOwner) {
        try {
          await clientOwner.dispose();
        } catch {
          // Cleanup errors do not change the node's submission result.
        }
      }
      setGenesisSigningBusy(false);
    }
  }

  async function checkRegistryGenesisConfirmation() {
    if (
      !genesisSignedTransaction ||
      !genesisSubmissionComplete ||
      genesisSubmissionConfirmed ||
      genesisSigningBusy
    ) {
      return;
    }

    const txHash = genesisSignedTransaction.hash();
    let clientOwner: ReturnType<typeof ccc.ClientPublicTestnet.open> | undefined;
    setGenesisSigningBusy(true);
    setGenesisSigningError("");
    setGenesisSigningStatus("Checking Registry Genesis block confirmation...");

    try {
      clientOwner = ccc.ClientPublicTestnet.open();
      const confirmedTransaction = await clientOwner.value.waitTransaction(
        txHash,
        0,
        120_000,
        2_000,
      );
      if (confirmedTransaction?.blockNumber != null) {
        setGenesisSigningStatus(
          "Registry Genesis confirmed on CKB Testnet in block " +
            String(confirmedTransaction.blockNumber) +
            ". RPC transaction hash: " +
            txHash,
        );
      } else {
        setGenesisSigningStatus(
          "Registry Genesis submitted to CKB Testnet, but confirmation is not available yet. Check transaction hash " +
            txHash +
            " before retrying.",
        );
      }
    } catch {
      setGenesisSigningStatus(
        "Registry Genesis submitted to CKB Testnet, but confirmation could not be verified yet. Check transaction hash " +
          txHash +
          " before retrying.",
      );
    } finally {
      setGenesisSigningBusy(false);
      if (clientOwner) {
        try {
          await clientOwner.dispose();
        } catch {
          // Cleanup errors do not change the confirmation result.
        }
      }
    }
  }

  const rawTransactionJson = deploymentPreview
    ? JSON.stringify(JSON.parse(ccc.stringify(deploymentPreview.transaction)), null, 2)
    : "";
  async function signDeploymentPreview() {
    if (!wallet || !deploymentPreview || signedTransaction || signingBusy) {
      return;
    }
    if (wallet.address !== deploymentPreview.ownerAddress) {
      setSigningError("The connected wallet differs from the preview owner. Build a fresh preview first.");
      return;
    }

    setSigningBusy(true);
    setSigningError("");
    setSigningStatus("Review the signing request in JoyID...");
    const rawTransactionHash = deploymentPreview.transaction.hash();

    try {
      const signed = await wallet.signTransaction(deploymentPreview.transaction);
      if (signed.hash() !== rawTransactionHash) {
        throw new Error(
          "JoyID changed the raw transaction fields. Do not use this signed result; rebuild and review a fresh preview.",
        );
      }
      if (!signed.witnesses.some((witness) => String(witness).toLowerCase() !== "0x")) {
        throw new Error("JoyID returned no signature witness.");
      }

      setSignedTransaction(signed);
      setSigningStatus("JoyID returned a signed transaction. It remains local and has not been sent.");
    } catch (err) {
      setSigningStatus("");
      setSigningError(err instanceof Error ? err.message : String(err));
    } finally {
      setSigningBusy(false);
    }
  }
  async function submitSignedDeployment() {
    if (
      !wallet ||
      !deploymentPreview ||
      !signedTransaction ||
      signingBusy ||
      confirmationBusy ||
      submissionInProgress ||
      submissionOutcomeUncertain ||
      submissionComplete
    ) {
      return;
    }

    if (wallet.address !== deploymentPreview.ownerAddress) {
      setSigningStatus("");
      setSigningError("The connected wallet differs from the preview owner. Rebuild and sign a fresh preview.");
      return;
    }

    const expectedHash = deploymentPreview.transaction.hash();
    if (signedTransaction.hash() !== expectedHash) {
      setSigningStatus("");
      setSigningError("The signed transaction no longer matches the reviewed preview. Rebuild and sign a fresh preview.");
      return;
    }

    if (!signedTransaction.witnesses.some((witness) => String(witness).toLowerCase() !== "0x")) {
      setSigningStatus("");
      setSigningError("The signed transaction has no signature witness. Sign the preview again before submitting.");
      return;
    }

    setSigningError("");
    const confirmed = window.confirm(
      [
        "Submit this already-signed transaction to CKB Testnet?",
        "No additional JoyID signing will occur.",
        "",
        "Owner: " + deploymentPreview.ownerAddress,
        "Registry code cell: " + formatCkb(deploymentPreview.registry.capacity) + " CKB",
        "State code cell: " + formatCkb(deploymentPreview.state.capacity) + " CKB",
        "Estimated fee: " + formatCkb(deploymentPreview.fee) + " CKB",
        "Change: " + formatCkb(deploymentPreview.change) + " CKB",
        "Funding inputs: " + deploymentPreview.inputs.map((input) => input.outPoint).join(", "),
        "Raw transaction hash: " + expectedHash,
        "",
        "Once accepted by the Testnet node, this transaction cannot be canceled.",
        "Continue?",
      ].join("\n"),
    );

    if (!confirmed) {
      setSigningStatus("Submission cancelled. The signed transaction remains local.");
      return;
    }

    setSigningStatus("Submitting signed transaction to CKB Testnet...");
    const clientOwner = ccc.ClientPublicTestnet.open();

    try {
      const returnedHash = await clientOwner.value.sendTransaction(signedTransaction);
      const rpcHash = String(returnedHash);
      if (rpcHash.toLowerCase() !== expectedHash.toLowerCase()) {
        setSigningStatus(
          "Submitted to CKB Testnet, but the returned hash differs. RPC hash: " +
            rpcHash +
            ". Expected: " +
            expectedHash +
            ". Do not submit again.",
        );
        return;
      }

      setSigningStatus(
        "Submitted to CKB Testnet. Awaiting block confirmation. RPC transaction hash: " +
          rpcHash
      );
      setConfirmationBusy(true);
      try {
        const confirmedTransaction = await clientOwner.value.waitTransaction(
          rpcHash,
          0,
          120_000,
          2_000,
        );
        if (confirmedTransaction?.blockNumber != null) {
          setSigningStatus(
            "Confirmed on CKB Testnet in block " +
              String(confirmedTransaction.blockNumber) +
              ". RPC transaction hash: " +
              rpcHash +
              ".",
          );
        } else {
          setSigningStatus(
            "Submitted to CKB Testnet, but confirmation is not available yet. Check transaction hash " +
              rpcHash +
              " before retrying.",
          );
        }
      } catch {
        setSigningStatus(
          "Submitted to CKB Testnet, but confirmation could not be verified yet. Check transaction hash " +
            rpcHash +
            " before retrying.",
        );
      } finally {
        setConfirmationBusy(false);
      }
    } catch (err) {
      setSigningStatus(
        "Submission outcome uncertain. Check transaction hash " +
          expectedHash +
          " before retrying.",
      );
      setSigningError(err instanceof Error ? err.message : String(err));
    } finally {
      try {
        await clientOwner.dispose();
      } catch {
        // Cleanup errors do not change the node's submission result.
      }
    }
  }

  async function checkSubmissionConfirmation() {
    if (!signedTransaction || !submissionComplete || submissionConfirmed || confirmationBusy) {
      return;
    }

    const txHash = signedTransaction.hash();
    const clientOwner = ccc.ClientPublicTestnet.open();
    setSigningError("");
    setConfirmationBusy(true);
    setSigningStatus(
      "Submitted to CKB Testnet. Checking block confirmation. RPC transaction hash: " +
        txHash,
    );

    try {
      const confirmedTransaction = await clientOwner.value.waitTransaction(
        txHash,
        0,
        120_000,
        2_000,
      );
      if (confirmedTransaction?.blockNumber != null) {
        setSigningStatus(
          "Confirmed on CKB Testnet in block " +
            String(confirmedTransaction.blockNumber) +
            ". RPC transaction hash: " +
            txHash +
            ".",
        );
      } else {
        setSigningStatus(
          "Submitted to CKB Testnet, but confirmation is not available yet. Check transaction hash " +
            txHash +
            " before retrying.",
        );
      }
    } catch {
      setSigningStatus(
        "Submitted to CKB Testnet, but confirmation could not be verified yet. Check transaction hash " +
          txHash +
          " before retrying.",
      );
    } finally {
      setConfirmationBusy(false);
      try {
        await clientOwner.dispose();
      } catch {
        // Cleanup errors do not change the node's submission result.
      }
    }
  }

  const signedTransactionJson = signedTransaction
    ? JSON.stringify(JSON.parse(ccc.stringify(signedTransaction)), null, 2)
    : "";
  const submissionInProgress = signingStatus.startsWith("Submitting signed transaction");
  const submissionOutcomeUncertain = signingStatus.startsWith("Submission outcome uncertain");
  const submissionConfirmed = signingStatus.startsWith("Confirmed on CKB Testnet in block");
  const submissionComplete =
    signingStatus.startsWith("Submitted to CKB Testnet") || submissionConfirmed;
  const genesisSubmissionInProgress =
    genesisSigningStatus.startsWith("Submitting Registry Genesis to CKB Testnet") ||
    genesisSigningStatus.startsWith("Checking Registry Genesis block confirmation") ||
    genesisSigningStatus.startsWith("Registry Genesis submitted to CKB Testnet; awaiting block confirmation");
  const genesisSubmissionOutcomeUncertain =
    genesisSigningStatus.startsWith("Registry Genesis submission outcome uncertain");
  const genesisSubmissionConfirmed =
    genesisSigningStatus.startsWith("Registry Genesis confirmed on CKB Testnet in block");
  const genesisSubmissionComplete =
    genesisSigningStatus.startsWith("Registry Genesis submitted to CKB Testnet") ||
    genesisSubmissionConfirmed;

  return (
    <main style={{ maxWidth: 1000, margin: "40px auto", padding: 24 }}>
      <h1>ReuseLoop</h1>
      <section style={{ border: "1px solid #888", borderRadius: 8, padding: 16, marginBottom: 24 }}>
        <h2>JoyID Wallet</h2>
        <p>Network: CKB Testnet</p>
        {wallet ? (
          <>
            <p><strong>Connected address:</strong></p>
            <code style={{ overflowWrap: "anywhere" }}>{wallet.address}</code>
            <p>
              <button
                onClick={disconnectWallet}
                disabled={walletBusy || signingBusy || genesisSigningBusy}
              >
                {walletBusy ? "Disconnecting..." : "Disconnect"}
              </button>
            </p>
          </>
        ) : (
          <p>
            <button onClick={connectWallet} disabled={walletBusy}>
              {walletBusy ? "Connecting..." : "Connect JoyID"}
            </button>
          </p>
        )}
      </section>

      <section style={{ border: "1px solid #888", borderRadius: 8, padding: 16, marginBottom: 24 }}>
        <h2>Checkpoint 2 — Deployment Preview</h2>
        <p>
          Select the CP11.1 Registry and State binaries. Each file is checked
          against its verified SHA-256 before it can be used.
        </p>
        <p style={{ marginTop: 12 }}>
          <label>
            Registry binary{" "}
            <input
              type="file"
              onChange={(event) => {
                const file = event.currentTarget.files?.[0];
                void selectArtifact("registry", file);
                event.currentTarget.value = "";
              }}
            />
          </label>
        </p>
        {registryArtifact && (
          <p style={{ marginTop: 8, overflowWrap: "anywhere" }}>
            Verified: {registryArtifact.fileName} · {registryArtifact.byteLength} bytes · SHA-256{" "}
            <code>{registryArtifact.sha256}</code>
          </p>
        )}
        <p style={{ marginTop: 12 }}>
          <label>
            State binary{" "}
            <input
              type="file"
              onChange={(event) => {
                const file = event.currentTarget.files?.[0];
                void selectArtifact("state", file);
                event.currentTarget.value = "";
              }}
            />
          </label>
        </p>
        {stateArtifact && (
          <p style={{ marginTop: 8, overflowWrap: "anywhere" }}>
            Verified: {stateArtifact.fileName} · {stateArtifact.byteLength} bytes · SHA-256{" "}
            <code>{stateArtifact.sha256}</code>
          </p>
        )}
        <p style={{ marginTop: 12 }}>{artifactStatus}</p>
        <p style={{ marginTop: 12 }}>
          <button
            onClick={createDeploymentPreview}
            disabled={!wallet || !registryArtifact || !stateArtifact || artifactBusy || deploymentBusy}
          >
            {artifactBusy ? "Verifying artifact..." : deploymentBusy ? "Building preview..." : "Build unsigned preview"}
          </button>
        </p>
        {!wallet && (
          <p style={{ marginTop: 8 }}>
            Connect JoyID to select the transaction owner and its Testnet CKB cells.
          </p>
        )}
        <p style={{ marginTop: 8 }}>
          Signing keeps the transaction local. Checkpoint 4 is a separate, user-confirmed Testnet submission step.
        </p>
        {deploymentError && (
          <pre style={{ whiteSpace: "pre-wrap", color: "crimson", overflowWrap: "anywhere" }}>
            {deploymentError}
          </pre>
        )}
        {deploymentPreview && (
          <div style={{ marginTop: 20, textAlign: "left" }}>
            <h3>Transaction summary</h3>
            <p><strong>Network:</strong> CKB Testnet</p>
            <p><strong>Lock owner:</strong> {deploymentPreview.ownerAddress}</p>
            <p><strong>Unsigned transaction hash:</strong></p>
            <code style={{ overflowWrap: "anywhere" }}>
              {deploymentPreview.transaction.hash()}
            </code>
            <p style={{ marginTop: 8 }}>
              <strong>Estimated fee:</strong> {formatCkb(deploymentPreview.fee)} CKB
              ({deploymentPreview.fee} shannons) at {deploymentPreview.feeRate} shannons/KB,
              with a 4 KiB witness-size reserve
            </p>
            <p>
              <strong>Change:</strong> {formatCkb(deploymentPreview.change)} CKB
            </p>
            <p>
              <strong>Type ID code hash:</strong>{" "}
              <code style={{ overflowWrap: "anywhere" }}>
                {deploymentPreview.typeIdCodeHash}
              </code>
            </p>
            <h4>Deployment outputs</h4>
            {[deploymentPreview.registry, deploymentPreview.state].map((output) => (
              <section
                key={output.kind}
                style={{ border: "1px solid #aaa", borderRadius: 8, padding: 12, marginTop: 12 }}
              >
                <h4>Output {output.outputIndex}: {output.kind} code cell</h4>
                <p>Artifact: {output.fileName} · {output.byteLength} bytes</p>
                <p>Capacity: {formatCkb(output.capacity)} CKB ({output.capacity} shannons)</p>
                <p>Code hash (data1):</p>
                <code style={{ overflowWrap: "anywhere" }}>{output.codeHash}</code>
                <p style={{ marginTop: 8 }}>Type ID args:</p>
                <code style={{ overflowWrap: "anywhere" }}>{output.typeId}</code>
                <p style={{ marginTop: 8 }}>Type Script hash:</p>
                <code style={{ overflowWrap: "anywhere" }}>{output.typeScriptHash}</code>
                <p style={{ marginTop: 8 }}>Artifact SHA-256:</p>
                <code style={{ overflowWrap: "anywhere" }}>{output.sha256}</code>
              </section>
            ))}
            <h4 style={{ marginTop: 16 }}>Funding inputs</h4>
            {deploymentPreview.inputs.map((input) => (
              <section
                key={input.outPoint}
                style={{ border: "1px solid #aaa", borderRadius: 8, padding: 12, marginTop: 8 }}
              >
                <p>OutPoint:</p>
                <code style={{ overflowWrap: "anywhere" }}>{input.outPoint}</code>
                <p style={{ marginTop: 8 }}>Capacity: {formatCkb(input.capacity)} CKB</p>
              </section>
            ))}
            <details style={{ marginTop: 16 }}>
              <summary>Raw unsigned transaction JSON</summary>
              <pre
                style={{
                  whiteSpace: "pre-wrap",
                  overflowWrap: "anywhere",
                  maxHeight: 420,
                  overflow: "auto",
                  fontSize: 12,
                }}
              >
                {rawTransactionJson}
              </pre>
            </details>
            <section style={{ border: "1px solid #888", borderRadius: 8, padding: 16, marginTop: 20 }}>
              <h4>CP11.3 — Sign contract code deployment</h4>
              <p>
                This signs only the code-cell deployment preview above, not Registry Genesis. Use it only if you
                intend to create another contract code deployment. The signature remains local.
              </p>
              <p style={{ marginTop: 12 }}>
                <button
                  onClick={signDeploymentPreview}
                  disabled={!wallet || signingBusy || deploymentBusy || Boolean(signedTransaction)}
                >
                  {signingBusy
                    ? "Waiting for JoyID..."
                    : signedTransaction
                      ? "Code deployment signed locally"
                      : "Sign code deployment preview in JoyID"}
                </button>
              </p>
              {signingStatus && !signedTransaction && <p style={{ marginTop: 8 }}>{signingStatus}</p>}
              {signingError && !signedTransaction && (
                <pre style={{ whiteSpace: "pre-wrap", color: "crimson", overflowWrap: "anywhere" }}>
                  {signingError}
                </pre>
              )}
              {signedTransaction && (
                <section style={{ border: "1px solid #aaa", borderRadius: 8, padding: 12, marginTop: 12 }}>
                  <h5>
                    {submissionConfirmed
                      ? "Confirmed transaction"
                      : submissionComplete
                        ? "Submitted transaction"
                        : "Signed transaction — not submitted"}
                  </h5>
                  <p>Raw transaction hash (must match the reviewed preview):</p>
                  <code style={{ overflowWrap: "anywhere" }}>{signedTransaction.hash()}</code>
                  <p style={{ marginTop: 8 }}>Full transaction hash including witnesses:</p>
                  <code style={{ overflowWrap: "anywhere" }}>{signedTransaction.hashFull()}</code>
                  <p style={{ marginTop: 8 }}>
                    Witnesses: {signedTransaction.witnesses.length}; signature present.
                  </p>
                  <details style={{ marginTop: 12 }}>
                    <summary>Signed transaction JSON</summary>
                    <pre
                      style={{
                        whiteSpace: "pre-wrap",
                        overflowWrap: "anywhere",
                        maxHeight: 420,
                        overflow: "auto",
                        fontSize: 12,
                      }}
                    >
                      {signedTransactionJson}
                    </pre>
                  </details>
                </section>
              )}
            </section>
            <section style={{ border: "1px solid #888", borderRadius: 8, padding: 16, marginTop: 16 }}>
              <h4>CP11.3 — Submit contract code deployment</h4>
              <p>
                This sends the already-signed transaction through the CKB Testnet client. It will not request another JoyID signature.
              </p>
              {signedTransaction ? (
                <>
                  <p><strong>Network:</strong> CKB Testnet</p>
                  <p><strong>Owner:</strong> {deploymentPreview.ownerAddress}</p>
                  <p><strong>Registry code cell:</strong> {formatCkb(deploymentPreview.registry.capacity)} CKB</p>
                  <p><strong>State code cell:</strong> {formatCkb(deploymentPreview.state.capacity)} CKB</p>
                  <p><strong>Estimated fee:</strong> {formatCkb(deploymentPreview.fee)} CKB</p>
                  <p><strong>Change:</strong> {formatCkb(deploymentPreview.change)} CKB</p>
                  <p><strong>Funding inputs:</strong> {deploymentPreview.inputs.length}</p>
                  <p><strong>Raw transaction hash:</strong></p>
                  <code style={{ overflowWrap: "anywhere" }}>{signedTransaction.hash()}</code>
                  <p style={{ marginTop: 12 }}>
                    After you select Submit, a final confirmation will summarize this transaction. Once the node accepts it, it cannot be canceled.
                  </p>
                  <button
                    onClick={submitSignedDeployment}
                    disabled={
                      !wallet ||
                      signingBusy ||
                      confirmationBusy ||
                      submissionInProgress ||
                      submissionOutcomeUncertain ||
                      submissionComplete
                    }
                  >
                    {confirmationBusy
                      ? "Checking block confirmation..."
                      : submissionInProgress
                      ? "Submitting to Testnet..."
                      : submissionConfirmed
                          ? "Confirmed on Testnet"
                          : submissionComplete
                        ? "Submitted to Testnet"
                        : submissionOutcomeUncertain
                          ? "Check transaction status before retrying"
                          : "Submit signed transaction to Testnet"}
                  </button>
                  {submissionComplete && !submissionConfirmed && (
                    <p style={{ marginTop: 8 }}>
                      <button onClick={checkSubmissionConfirmation} disabled={confirmationBusy}>
                        {confirmationBusy ? "Checking block confirmation..." : "Check block confirmation"}
                      </button>
                    </p>
                  )}
                  {signingStatus && <p style={{ marginTop: 8 }}>{signingStatus}</p>}
                  {signingError && (
                    <pre style={{ whiteSpace: "pre-wrap", color: "crimson", overflowWrap: "anywhere" }}>
                      {signingError}
                    </pre>
                  )}
                  {submissionOutcomeUncertain && (
                    <p>
                      Do not submit again until you check this hash with a Testnet explorer or RPC.
                    </p>
                  )}
                </>
              ) : (
                <p>Sign the reviewed transaction in Checkpoint 3 to enable this step.</p>
              )}
            </section>
          </div>
        )}
      </section>

      <section style={{ border: "1px solid #888", borderRadius: 8, padding: 16, marginBottom: 24 }}>
        <h2>CP11.4.1 — Registry Genesis Preview</h2>
        <p>
          Build the first Registry cell from the deployed Registry code cell and a live empty wallet cell.
          This step only reads Testnet data and creates an unsigned preview.
        </p>
        <p style={{ marginTop: 12 }}>
          <button
            onClick={createRegistryGenesisPreview}
            disabled={
              !wallet ||
              !registryArtifact ||
              !stateArtifact ||
              genesisBusy ||
              genesisSigningBusy ||
              Boolean(genesisSignedTransaction) ||
              genesisSubmissionComplete ||
              genesisSubmissionOutcomeUncertain
            }
          >
            {genesisBusy ? "Building Registry preview..." : "Build unsigned Registry Genesis preview"}
          </button>
        </p>
        {!wallet && <p>Connect JoyID on CKB Testnet first.</p>}
        {wallet && (!registryArtifact || !stateArtifact) && (
          <p>Verify both CP11.1 contract binaries in Checkpoint 2 first.</p>
        )}
        {genesisStatus && <p style={{ marginTop: 8 }}>{genesisStatus}</p>}
        {genesisError && (
          <pre style={{ whiteSpace: "pre-wrap", color: "crimson", overflowWrap: "anywhere" }}>
            {genesisError}
          </pre>
        )}
        {genesisPreview && (
          <div style={{ marginTop: 16, textAlign: "left" }}>
            <p><strong>Network:</strong> CKB Testnet</p>
            <p><strong>Registry ID:</strong></p>
            <code style={{ overflowWrap: "anywhere" }}>{genesisPreview.registryId}</code>
            <p style={{ marginTop: 8 }}><strong>Registry Type Script hash:</strong></p>
            <code style={{ overflowWrap: "anywhere" }}>{genesisPreview.registryTypeScriptHash}</code>
            <p style={{ marginTop: 8 }}>
              <strong>Registry cell capacity:</strong> {formatCkb(genesisPreview.registryCapacity)} CKB
            </p>
            <p style={{ marginTop: 8 }}><strong>Seed input:</strong></p>
            <code style={{ overflowWrap: "anywhere" }}>{genesisPreview.seedOutPoint}</code>
            <p>Seed capacity: {formatCkb(genesisPreview.seedCapacity)} CKB</p>
            <p style={{ marginTop: 8 }}><strong>Registry code cell dependency:</strong></p>
            <code style={{ overflowWrap: "anywhere" }}>{genesisPreview.registryCodeOutPoint}</code>
            <p style={{ marginTop: 8 }}><strong>State binary cell verified:</strong></p>
            <code style={{ overflowWrap: "anywhere" }}>{genesisPreview.stateCodeOutPoint}</code>
            <p style={{ marginTop: 8 }}>
              <strong>Estimated fee:</strong> {formatCkb(genesisPreview.fee)} CKB
              ({genesisPreview.fee} shannons) at {genesisPreview.feeRate} shannons/KB
            </p>
            <p><strong>Change:</strong> {formatCkb(genesisPreview.change)} CKB</p>
            <p><strong>Unsigned transaction hash:</strong></p>
            <code style={{ overflowWrap: "anywhere" }}>{genesisPreview.transaction.hash()}</code>
            <details style={{ marginTop: 16 }}>
              <summary>Raw unsigned Registry Genesis transaction JSON</summary>
              <pre
                style={{
                  whiteSpace: "pre-wrap",
                  overflowWrap: "anywhere",
                  maxHeight: 420,
                  overflow: "auto",
                  fontSize: 12,
                }}
              >
                {JSON.stringify(
                  JSON.parse(ccc.stringify(genesisPreview.transaction)),
                  null,
                  2,
                )}
              </pre>
            </details>
            <section style={{ border: "1px solid #888", borderRadius: 8, padding: 16, marginTop: 20 }}>
              <h3>CP11.4.2 — Sign Registry Genesis</h3>
              <p>
                This asks JoyID to sign the Registry Genesis preview above. The signature remains local;
                this action does not submit or broadcast the transaction.
              </p>
              <p style={{ marginTop: 12 }}>
                <button
                  onClick={signRegistryGenesisPreview}
                  disabled={
                    !wallet ||
                    !genesisPreview ||
                    genesisBusy ||
                    genesisSigningBusy ||
                    Boolean(genesisSignedTransaction)
                  }
                >
                  {genesisSigningBusy
                    ? genesisSubmissionInProgress
                      ? "Submitting or checking confirmation..."
                      : "Waiting for JoyID..."
                    : genesisSignedTransaction
                      ? "Registry Genesis signed locally"
                      : "Sign Registry Genesis in JoyID"}
                </button>
              </p>
              {genesisSigningStatus && !genesisSignedTransaction && (
                <p style={{ marginTop: 8 }}>{genesisSigningStatus}</p>
              )}
              {genesisSigningError && (
                <pre style={{ whiteSpace: "pre-wrap", color: "crimson", overflowWrap: "anywhere" }}>
                  {genesisSigningError}
                </pre>
              )}
              {genesisSignedTransaction && (
                <section style={{ border: "1px solid #aaa", borderRadius: 8, padding: 12, marginTop: 12 }}>
                  <h4>
                    {genesisSubmissionConfirmed
                      ? "Registry Genesis confirmed"
                      : genesisSubmissionComplete
                        ? "Registry Genesis submitted"
                        : genesisSubmissionOutcomeUncertain
                          ? "Registry Genesis submission status uncertain"
                          : "Signed Registry Genesis — not submitted"}
                  </h4>
                  <p>Raw transaction hash (must match the reviewed preview):</p>
                  <code style={{ overflowWrap: "anywhere" }}>{genesisSignedTransaction.hash()}</code>
                  <p style={{ marginTop: 8 }}>Full transaction hash including witnesses:</p>
                  <code style={{ overflowWrap: "anywhere" }}>{genesisSignedTransaction.hashFull()}</code>
                  <p style={{ marginTop: 8 }}>
                    Witnesses: {genesisSignedTransaction.witnesses.length}; signature present.
                  </p>
                  <details style={{ marginTop: 12 }}>
                    <summary>Signed Registry Genesis transaction JSON</summary>
                    <pre
                      style={{
                        whiteSpace: "pre-wrap",
                        overflowWrap: "anywhere",
                        maxHeight: 420,
                        overflow: "auto",
                        fontSize: 12,
                      }}
                    >
                      {JSON.stringify(
                        JSON.parse(ccc.stringify(genesisSignedTransaction)),
                        null,
                        2,
                      )}
                    </pre>
                  </details>
                </section>
              )}
            </section>
            <section style={{ border: "1px solid #888", borderRadius: 8, padding: 16, marginTop: 16 }}>
              <h4>CP11.4.3 — Submit Registry Genesis</h4>
              <p>
                This broadcasts the already-signed Registry Genesis transaction to CKB Testnet.
                It does not request another signature. Review the summary and final browser confirmation first.
              </p>
              {genesisSignedTransaction ? (
                <>
                  <p><strong>Network:</strong> CKB Testnet</p>
                  <p><strong>Registry ID:</strong></p>
                  <code style={{ overflowWrap: "anywhere" }}>{genesisPreview.registryId}</code>
                  <p style={{ marginTop: 8 }}>
                    <strong>Registry cell:</strong> {formatCkb(genesisPreview.registryCapacity)} CKB
                  </p>
                  <p><strong>Seed input:</strong></p>
                  <code style={{ overflowWrap: "anywhere" }}>{genesisPreview.seedOutPoint}</code>
                  <p style={{ marginTop: 8 }}>
                    <strong>Estimated fee:</strong> {formatCkb(genesisPreview.fee)} CKB
                    ({genesisPreview.fee} shannons) at {genesisPreview.feeRate} shannons/KB
                  </p>
                  <p><strong>Change:</strong> {formatCkb(genesisPreview.change)} CKB</p>
                  <p><strong>Raw transaction hash:</strong></p>
                  <code style={{ overflowWrap: "anywhere" }}>{genesisSignedTransaction.hash()}</code>
                  <p style={{ marginTop: 12 }}>
                    Once a Testnet node accepts this transaction, its seed input is consumed and the Registry cell is created.
                    Do not retry if the submission result is uncertain; check the hash with an explorer first.
                  </p>
                  <button
                    onClick={submitSignedRegistryGenesis}
                    disabled={
                      !wallet ||
                      genesisSigningBusy ||
                      genesisSubmissionInProgress ||
                      genesisSubmissionOutcomeUncertain ||
                      genesisSubmissionComplete
                    }
                  >
                    {genesisSubmissionConfirmed
                      ? "Registry Genesis confirmed"
                      : genesisSubmissionInProgress
                        ? "Submitting or checking confirmation..."
                        : genesisSubmissionComplete
                          ? "Submitted; check block status"
                          : genesisSubmissionOutcomeUncertain
                            ? "Check transaction status before retrying"
                            : "Submit signed Registry Genesis to Testnet"}
                  </button>
                  {genesisSigningStatus && (
                    <p style={{ marginTop: 8 }}>{genesisSigningStatus}</p>
                  )}
                  {genesisSigningError && (
                    <pre style={{ whiteSpace: "pre-wrap", color: "crimson", overflowWrap: "anywhere" }}>
                      {genesisSigningError}
                    </pre>
                  )}
                  {genesisSubmissionComplete && !genesisSubmissionConfirmed && (
                    <p style={{ marginTop: 8 }}>
                      <button
                        onClick={checkRegistryGenesisConfirmation}
                        disabled={genesisSigningBusy}
                      >
                        {genesisSigningBusy
                          ? "Checking block confirmation..."
                          : "Check Registry Genesis block confirmation"}
                      </button>
                    </p>
                  )}
                  {genesisSubmissionOutcomeUncertain && (
                    <p>
                      Do not submit again until you check the transaction hash with a Testnet explorer or RPC.
                    </p>
                  )}
                </>
              ) : (
                <p>Sign the reviewed Registry Genesis transaction above to enable submission.</p>
              )}
            </section>
          </div>
        )}
      </section>

      <RegistryRegistrationPanel
        walletAddress={wallet?.address ?? null}
        registryArtifact={registryArtifact}
        stateArtifact={stateArtifact}
        registryGenesisTxHash={genesisPreview?.transaction.hash() ?? null}
      />

      <h2>CKB Testnet - Cell Inspector</h2>
      <p><strong>Inspector address:</strong></p>
      <code style={{ overflowWrap: "anywhere" }}>
        {wallet?.address ?? TESTNET_ADDRESS}
      </code>
      <p>
        <button onClick={queryCells} disabled={loading}>
          {loading ? "Querying..." : "Inspect Cells"}
        </button>
      </p>
      <p>{status}</p>
      {error && (
        <pre style={{ whiteSpace: "pre-wrap", color: "crimson" }}>
          {error}
        </pre>
      )}
      {cells.map((cell) => (
        <section
          key={cell.txHash + "-" + cell.index}
          style={{
            border: "1px solid #888",
            borderRadius: 8,
            padding: 16,
            marginTop: 16,
          }}
        >
          <h3>OutPoint</h3>
          <p><strong>Transaction:</strong></p>
          <code style={{ overflowWrap: "anywhere" }}>
            {cell.txHash}
          </code>
          <p><strong>Index:</strong> {cell.index}</p>
          <p><strong>Capacity:</strong> {cell.capacity} shannons</p>
          <p>
            <strong>Capacity:</strong>{" "}
            {(Number(cell.capacity) / 100_000_000).toFixed(8)} CKB
          </p>
          <h3>Lock Script</h3>
          <pre style={{ whiteSpace: "pre-wrap", overflowWrap: "anywhere" }}>
            {JSON.stringify(cell.lock, null, 2)}
          </pre>
          <h3>Type Script</h3>
          <pre style={{ whiteSpace: "pre-wrap", overflowWrap: "anywhere" }}>
            {JSON.stringify(cell.type, null, 2)}
          </pre>
          <h3>Data</h3>
          <pre style={{ whiteSpace: "pre-wrap", overflowWrap: "anywhere" }}>
            {cell.data}
          </pre>
        </section>
      ))}
    </main>
  );
}
