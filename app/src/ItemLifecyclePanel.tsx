import { useEffect, useState } from "react";
import { ccc } from "@ckb-ccc/ccc";
import type { VerifiedContractBinary } from "./domain/deploymentPreview";
import type { WalletSession } from "./domain/wallet";
import { buildLifecyclePreview, type LifecyclePreview } from "./domain/lifecyclePreview";

interface ItemLifecyclePanelProps {
  wallet: WalletSession | null;
  otherSigningBusy: boolean;
  onBusyChange: (busy: boolean) => void;
  registryArtifact: VerifiedContractBinary | null;
  stateArtifact: VerifiedContractBinary | null;
}

type SubmissionState = "ready" | "submitting" | "submitted" | "confirmed" | "uncertain";

function formatCkb(shannons: string): string {
  const value = BigInt(shannons);
  const whole = value / 100_000_000n;
  const fraction = (value % 100_000_000n).toString().padStart(8, "0");
  return whole.toString() + "." + fraction;
}

export default function ItemLifecyclePanel({
  wallet,
  otherSigningBusy,
  onBusyChange,
  registryArtifact,
  stateArtifact,
}: ItemLifecyclePanelProps) {
  const [itemId, setItemId] = useState(() =>
    new URLSearchParams(window.location.search).get("item") ?? "",
  );
  const [preview, setPreview] = useState<LifecyclePreview | null>(null);
  const [signedTransaction, setSignedTransaction] = useState<ccc.Transaction | null>(null);
  const [buildBusy, setBuildBusy] = useState(false);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState("");
  const [error, setError] = useState("");
  const [submissionState, setSubmissionState] = useState<SubmissionState>("ready");
  const [submittedHash, setSubmittedHash] = useState("");

  useEffect(() => {
    setPreview(null);
    setSignedTransaction(null);
    setStatus("");
    setError("");
    setSubmissionState("ready");
    setSubmittedHash("");
  }, [wallet?.address, registryArtifact?.codeHash, stateArtifact?.codeHash]);

  async function buildPreview() {
    if (
      buildBusy ||
      busy ||
      otherSigningBusy ||
      submissionState === "submitting" ||
      submissionState === "submitted" ||
      submissionState === "uncertain" ||
      (signedTransaction && submissionState !== "confirmed")
    ) {
      return;
    }
    if (!wallet) {
      setError("Connect the administrator JoyID wallet before preparing a lifecycle transaction.");
      return;
    }
    if (!registryArtifact || !stateArtifact) {
      setError("Verify both CP11.1 binaries in Checkpoint 2 first.");
      return;
    }

    setBuildBusy(true);
    setPreview(null);
    setSignedTransaction(null);
    setError("");
    setSubmittedHash("");
    setSubmissionState("ready");
    setStatus("Reading the current live State and Registry cells, then preparing the next lifecycle transaction...");
    let clientOwner: ReturnType<typeof ccc.ClientPublicTestnet.open> | undefined;

    try {
      clientOwner = ccc.ClientPublicTestnet.open();
      const nextPreview = await buildLifecyclePreview(
        clientOwner.value,
        wallet.address,
        itemId,
        registryArtifact,
        stateArtifact,
      );
      setPreview(nextPreview);
      const url = new URL(window.location.href);
      url.searchParams.set("item", nextPreview.itemId);
      window.history.replaceState(null, "", url.toString());
      setStatus("Lifecycle preview is ready. It has not been signed or submitted.");
    } catch (err) {
      setStatus("Could not build the lifecycle preview.");
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      if (clientOwner) {
        try {
          await clientOwner.dispose();
        } catch {
          // Cleanup errors do not change the preview result.
        }
      }
      setBuildBusy(false);
    }
  }

  async function signPreview() {
    if (!wallet || !preview || busy || buildBusy || otherSigningBusy || signedTransaction) {
      return;
    }
    if (wallet.address !== preview.ownerAddress) {
      setError("The connected wallet differs from this preview owner. Rebuild before signing.");
      return;
    }

    setBusy(true);
    onBusyChange(true);
    setError("");
    setStatus("Review the lifecycle signing request in JoyID...");
    try {
      const expectedHash = preview.transaction.hash();
      const signed = await wallet.signTransaction(preview.transaction);
      if (signed.hash().toLowerCase() !== expectedHash.toLowerCase()) {
        throw new Error("JoyID changed the reviewed lifecycle transaction fields. Rebuild and review a fresh preview.");
      }
      if (!signed.witnesses.some((witness) => String(witness).toLowerCase() !== "0x")) {
        throw new Error("JoyID returned no signature witness for this lifecycle transaction.");
      }
      setSignedTransaction(signed);
      setStatus("The signature is local. The lifecycle transaction has not been submitted.");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setStatus("Lifecycle signing failed.");
    } finally {
      setBusy(false);
      onBusyChange(false);
    }
  }

  function validateSignedTransaction(): string | null {
    if (!wallet || !preview || !signedTransaction) {
      return "Build and sign a lifecycle preview first.";
    }
    if (wallet.address !== preview.ownerAddress) {
      return "The connected wallet differs from this lifecycle preview owner.";
    }
    if (signedTransaction.hash().toLowerCase() !== preview.transaction.hash().toLowerCase()) {
      return "The signed transaction no longer matches the reviewed preview.";
    }
    if (!signedTransaction.witnesses.some((witness) => String(witness).toLowerCase() !== "0x")) {
      return "The signed lifecycle transaction has no signature witness.";
    }
    return null;
  }

  async function submitSignedTransaction() {
    if (submissionState !== "ready" || busy || !preview || !signedTransaction) {
      return;
    }
    const validationError = validateSignedTransaction();
    if (validationError) {
      setError(validationError);
      return;
    }

    const expectedHash = preview.transaction.hash();
    const confirmed = window.confirm(
      [
        "Submit this signed lifecycle transaction to CKB Testnet?",
        "The transaction consumes the current State cell and funding inputs. Once accepted, it cannot be canceled.",
        "",
        "Action: " + preview.action,
        "Item ID: " + preview.itemId,
        "Status: " + preview.fromStatus + " → " + preview.toStatus,
        "State input: " + preview.stateCellOutPoint,
        "Registry dependency: " + preview.registryCellOutPoint,
        ...preview.feeInputs.map((input) => "Funding input: " + input.outPoint + " · " + formatCkb(input.capacity) + " CKB"),
        "Estimated fee: " + formatCkb(preview.estimatedFee) + " CKB at " + preview.feeRate + " shannons/KB",
        "Actual fee: " + formatCkb(preview.actualFee) + " CKB",
        "Change: " + formatCkb(preview.change) + " CKB",
        "Transaction hash: " + expectedHash,
        "",
        "If the result is uncertain, check the hash with a Testnet explorer before any retry.",
      ].join("\n"),
    );
    if (!confirmed) {
      setStatus("Submission cancelled. The signed transaction remains local.");
      return;
    }

    setBusy(true);
    onBusyChange(true);
    setError("");
    setStatus("Checking lifecycle inputs before submission...");
    setSubmissionState("submitting");
    let clientOwner: ReturnType<typeof ccc.ClientPublicTestnet.open> | undefined;
    let sendStarted = false;

    try {
      clientOwner = ccc.ClientPublicTestnet.open();
      for (const input of signedTransaction.inputs) {
        const liveInput = await clientOwner.value.getCellLive(input.previousOutput, true);
        if (!liveInput) {
          setSubmissionState("ready");
          setStatus("A transaction input is no longer live. Nothing was submitted; rebuild the preview from current Testnet cells.");
          setError("At least one input was consumed or is not available from the Testnet indexer.");
          return;
        }
      }

      setStatus("Submitting the signed lifecycle transaction to CKB Testnet...");
      sendStarted = true;
      const returnedHash = String(await clientOwner.value.sendTransaction(signedTransaction));
      if (returnedHash.toLowerCase() !== expectedHash.toLowerCase()) {
        setSubmissionState("uncertain");
        setStatus("Submission outcome uncertain: RPC returned a different transaction hash. Check both hashes before any retry.");
        setError("Expected " + expectedHash + " but the RPC returned " + returnedHash + ".");
        return;
      }

      setSubmittedHash(returnedHash);
      setSubmissionState("submitted");
      setStatus("Lifecycle transaction submitted. Waiting for block confirmation; do not submit it again.");
      try {
        const confirmedTransaction = await clientOwner.value.waitTransaction(
          returnedHash,
          0,
          120_000,
          2_000,
        );
        if (confirmedTransaction?.blockNumber != null) {
          setSubmissionState("confirmed");
          setStatus(
            preview.action + " confirmed on CKB Testnet in block " +
              String(confirmedTransaction.blockNumber) +
              ". Transaction hash: " +
              returnedHash,
          );
        } else {
          setStatus("Submitted, but block confirmation is not available yet. Check the transaction hash before taking further action.");
        }
      } catch {
        setStatus("Submitted, but block confirmation could not be checked yet. Check the transaction hash; do not resubmit.");
      }
    } catch (err) {
      if (sendStarted) {
        setSubmissionState("uncertain");
        setStatus("Submission outcome uncertain. Check the expected transaction hash with a Testnet explorer before any retry.");
      } else {
        setSubmissionState("ready");
        setStatus("Could not preflight or open the Testnet client. The transaction was not submitted.");
      }
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      if (clientOwner) {
        try {
          await clientOwner.dispose();
        } catch {
          // Cleanup errors do not change the submission result.
        }
      }
      setBusy(false);
      onBusyChange(false);
    }
  }

  async function checkConfirmation() {
    if (submissionState !== "submitted" || !submittedHash || busy) {
      return;
    }
    setBusy(true);
    onBusyChange(true);
    setError("");
    setStatus("Checking the lifecycle transaction block confirmation...");
    let clientOwner: ReturnType<typeof ccc.ClientPublicTestnet.open> | undefined;
    try {
      clientOwner = ccc.ClientPublicTestnet.open();
      const confirmedTransaction = await clientOwner.value.waitTransaction(
        submittedHash,
        0,
        120_000,
        2_000,
      );
      if (confirmedTransaction?.blockNumber != null) {
        setSubmissionState("confirmed");
        setStatus(
          preview?.action + " confirmed on CKB Testnet in block " +
            String(confirmedTransaction.blockNumber) +
            ". Transaction hash: " +
            submittedHash,
        );
      } else {
        setStatus("Block confirmation is not available yet. Do not resubmit.");
      }
    } catch (err) {
      setStatus("Block confirmation could not be checked. Do not resubmit; check the transaction hash with a Testnet explorer.");
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      if (clientOwner) {
        try {
          await clientOwner.dispose();
        } catch {
          // Cleanup errors do not change the confirmation result.
        }
      }
      setBusy(false);
      onBusyChange(false);
    }
  }

  function clearLocalSignature() {
    if (submissionState !== "ready" || busy) return;
    setSignedTransaction(null);
    setStatus("Local signature cleared. The reviewed preview is unchanged.");
    setError("");
  }

  const canBuild = Boolean(
    wallet &&
    registryArtifact &&
    stateArtifact &&
    itemId.trim() &&
    !buildBusy &&
    !busy &&
    !otherSigningBusy &&
    (submissionState === "ready" || submissionState === "confirmed") &&
    (!signedTransaction || submissionState === "confirmed"),
  );
  const canSign = Boolean(
    wallet &&
    preview &&
    !buildBusy &&
    !busy &&
    !otherSigningBusy &&
    !signedTransaction &&
    wallet.address === preview.ownerAddress,
  );
  const canSubmit = Boolean(
    preview &&
    signedTransaction &&
    submissionState === "ready" &&
    !busy &&
    wallet?.address === preview.ownerAddress,
  );

  return (
    <section style={{ border: "1px solid #888", borderRadius: 8, padding: 16, margin: "24px 0" }}>
      <h2>CP11.7 — Borrow and Return Lifecycle</h2>
      <p>
        Prepare the next allowed transition from the live State cell. Each transition has a separate review, JoyID signing, and Testnet submission step.
      </p>
      <label htmlFor="lifecycle-item-id"><strong>Item ID</strong></label>
      <p>
        <input
          id="lifecycle-item-id"
          value={itemId}
          onChange={(event) => {
            setItemId(event.target.value);
            setPreview(null);
            setSignedTransaction(null);
            setSubmissionState("ready");
            setSubmittedHash("");
            setStatus("");
            setError("");
          }}
          disabled={
            buildBusy ||
            busy ||
            (Boolean(signedTransaction) && submissionState !== "confirmed") ||
            submissionState === "submitted" ||
            submissionState === "uncertain"
          }
          placeholder="0x + 64 hexadecimal characters"
          style={{ width: "min(100%, 620px)" }}
        />
      </p>
      <p>
        <button type="button" onClick={buildPreview} disabled={!canBuild}>
          {buildBusy
            ? "Reading live cells..."
            : submissionState === "confirmed"
              ? "Load current state and build next step"
              : "Build lifecycle preview"}
        </button>
      </p>
      {!wallet && <p>Connect the administrator JoyID wallet to prepare lifecycle transactions.</p>}
      {wallet && (!registryArtifact || !stateArtifact) && (
        <p>Verify the Registry and State binaries in Checkpoint 2 first.</p>
      )}
      {status && <p role="status">{status}</p>}
      {error && (
        <pre role="alert" style={{ whiteSpace: "pre-wrap", color: "crimson", overflowWrap: "anywhere" }}>
          {error}
        </pre>
      )}
      {preview && (
        <section style={{ border: "1px solid #aaa", borderRadius: 8, padding: 16, marginTop: 16 }}>
          <h3>{preview.action}: {preview.fromStatus} → {preview.toStatus}</h3>
          <p><strong>Network:</strong> CKB Testnet</p>
          <p><strong>Owner:</strong></p>
          <code style={{ overflowWrap: "anywhere" }}>{preview.ownerAddress}</code>
          <p style={{ marginTop: 8 }}><strong>Item ID:</strong></p>
          <code style={{ overflowWrap: "anywhere" }}>{preview.itemId}</code>
          <p style={{ marginTop: 8 }}><strong>Registry ID:</strong></p>
          <code style={{ overflowWrap: "anywhere" }}>{preview.registryId}</code>
          <p><strong>State input:</strong></p>
          <code style={{ overflowWrap: "anywhere" }}>{preview.stateCellOutPoint}</code>
          <p>State capacity preserved: {formatCkb(preview.stateCapacity)} CKB</p>
          <p><strong>Registry dependency:</strong></p>
          <code style={{ overflowWrap: "anywhere" }}>{preview.registryCellOutPoint}</code>
          <p><strong>State code dependency:</strong></p>
          <code style={{ overflowWrap: "anywhere" }}>{preview.stateCodeOutPoint}</code>
          <p><strong>Funding inputs:</strong> {preview.feeInputs.length}</p>
          {preview.feeInputs.map((input) => (
            <div key={input.outPoint}>
              <code style={{ overflowWrap: "anywhere" }}>{input.outPoint}</code>
              {" · "}{formatCkb(input.capacity)} CKB
            </div>
          ))}
          <p style={{ marginTop: 8 }}>
            Estimated fee: {formatCkb(preview.estimatedFee)} CKB at {preview.feeRate} shannons/KB
          </p>
          <p>Actual fee: {formatCkb(preview.actualFee)} CKB</p>
          <p>Change: {formatCkb(preview.change)} CKB</p>
          <p><strong>Unsigned transaction hash:</strong></p>
          <code style={{ overflowWrap: "anywhere" }}>{preview.transaction.hash()}</code>
          <details style={{ marginTop: 12 }}>
            <summary>Raw unsigned lifecycle transaction JSON</summary>
            <pre style={{ whiteSpace: "pre-wrap", overflowWrap: "anywhere", maxHeight: 420, overflow: "auto", fontSize: 12 }}>
              {JSON.stringify(JSON.parse(ccc.stringify(preview.transaction)), null, 2)}
            </pre>
          </details>
          <p style={{ marginTop: 12 }}>
            Review the Item ID, transition, live inputs, fee, and change before signing.
          </p>
          <button type="button" onClick={signPreview} disabled={!canSign}>
            {busy && submissionState === "ready" ? "Waiting for JoyID..." : "Sign lifecycle preview in JoyID"}
          </button>
          {signedTransaction && (
            <section style={{ border: "1px solid #aaa", borderRadius: 8, padding: 12, marginTop: 12 }}>
              <h4>Signed locally — not submitted</h4>
              <p>Raw transaction hash: <code style={{ overflowWrap: "anywhere" }}>{signedTransaction.hash()}</code></p>
              <p>Full transaction hash including witnesses: <code style={{ overflowWrap: "anywhere" }}>{signedTransaction.hashFull()}</code></p>
              <p>Witnesses: {signedTransaction.witnesses.length}; signature present.</p>
              <button type="button" onClick={clearLocalSignature} disabled={submissionState !== "ready" || busy}>
                Clear local signature
              </button>
            </section>
          )}
          <p>
            <button type="button" onClick={submitSignedTransaction} disabled={!canSubmit}>
              {submissionState === "confirmed"
                ? "Lifecycle transition confirmed"
                : submissionState === "submitted"
                  ? "Submitted; do not resubmit"
                  : submissionState === "uncertain"
                    ? "Outcome uncertain; check hash before retrying"
                    : busy
                      ? "Submitting or checking confirmation..."
                      : "Submit signed lifecycle transaction to Testnet"}
            </button>
          </p>
          {submissionState === "submitted" && !busy && (
            <button type="button" onClick={checkConfirmation}>Check block confirmation</button>
          )}
          {submittedHash && (
            <p>RPC transaction hash: <code style={{ overflowWrap: "anywhere" }}>{submittedHash}</code></p>
          )}
        </section>
      )}
    </section>
  );
}