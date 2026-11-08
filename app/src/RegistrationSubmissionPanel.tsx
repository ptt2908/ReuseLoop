import { useState } from "react";
import { ccc } from "@ckb-ccc/ccc";
import type { AtomicRegistrationPreview } from "./domain/registryRegistrationPreview";
import type { WalletSession } from "./domain/wallet";

interface RegistrationSubmissionPanelProps {
  wallet: WalletSession | null;
  preview: AtomicRegistrationPreview;
  signedTransaction: ccc.Transaction;
  onBusyChange: (busy: boolean) => void;
}

type SubmissionState = "ready" | "submitting" | "submitted" | "confirmed" | "uncertain";

function formatCkb(shannons: string): string {
  const value = BigInt(shannons);
  const whole = value / 100_000_000n;
  const fraction = (value % 100_000_000n).toString().padStart(8, "0");
  return whole.toString() + "." + fraction;
}

export default function RegistrationSubmissionPanel({
  wallet,
  preview,
  signedTransaction,
  onBusyChange,
}: RegistrationSubmissionPanelProps) {
  const [submissionState, setSubmissionState] = useState<SubmissionState>("ready");
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState("");
  const [error, setError] = useState("");
  const [submittedHash, setSubmittedHash] = useState("");
  const [reportedHash, setReportedHash] = useState("");

  function validateSignedPreview(): string | null {
    if (!wallet) {
      return "Connect the preview owner wallet before submitting.";
    }
    if (wallet.address !== preview.ownerAddress) {
      return "The connected wallet differs from this registration preview owner.";
    }
    const expectedHash = preview.transaction.hash();
    if (signedTransaction.hash().toLowerCase() !== expectedHash.toLowerCase()) {
      return "The signed transaction no longer matches the reviewed preview. Do not submit it.";
    }
    if (!signedTransaction.witnesses.some((witness) => String(witness).toLowerCase() !== "0x")) {
      return "The signed registration transaction has no signature witness.";
    }
    return null;
  }

  async function submitSignedRegistration() {
    if (submissionState !== "ready" || busy) {
      return;
    }
    const validationError = validateSignedPreview();
    if (validationError) {
      setStatus("");
      setError(validationError);
      return;
    }

    const expectedHash = preview.transaction.hash();
    const confirmed = window.confirm(
      [
        "Submit this signed Atomic Item Registration to CKB Testnet?",
        "This consumes the current Registry and funding inputs and creates the successor Registry and AVAILABLE State cell. Once accepted, it cannot be canceled.",
        "",
        "Owner: " + preview.ownerAddress,
        "Registry ID: " + preview.registryId,
        "Item ID: " + preview.itemId,
        "Registry input: " + preview.registryCellOutPoint,
        "Registry capacity: " + formatCkb(preview.registryInputCapacity) + " → " + formatCkb(preview.registryOutputCapacity) + " CKB",
        "State output: AVAILABLE · " + formatCkb(preview.stateCapacity) + " CKB",
        ...preview.feeInputs.map((input) => "Funding input: " + input.outPoint + " · " + formatCkb(input.capacity) + " CKB"),
        "Estimated fee: " + formatCkb(preview.estimatedFee) + " CKB at " + preview.feeRate + " shannons/KB",
        "Actual fee from input/output balance: " + formatCkb(preview.actualFee) + " CKB",
        "Change: " + formatCkb(preview.change) + " CKB",
        "Raw transaction hash: " + expectedHash,
        "",
        "If the submission result is uncertain, check this hash with an explorer before taking any further action.",
      ].join("\n"),
    );
    if (!confirmed) {
      setError("");
      setStatus("Submission cancelled. The signed transaction remains local.");
      return;
    }

    setBusy(true);
    onBusyChange(true);
    setError("");
    setStatus("Submitting the signed registration to CKB Testnet...");
    setSubmissionState("submitting");
    let clientOwner: ReturnType<typeof ccc.ClientPublicTestnet.open> | undefined;
    let sendStarted = false;

    try {
      clientOwner = ccc.ClientPublicTestnet.open();
      sendStarted = true;
      const returnedHash = String(await clientOwner.value.sendTransaction(signedTransaction));
      setReportedHash(returnedHash);
      if (returnedHash.toLowerCase() !== expectedHash.toLowerCase()) {
        setSubmissionState("uncertain");
        setStatus("Submission outcome uncertain: the RPC returned a different hash. Check both hashes with a Testnet explorer before any retry.");
        setError("Expected " + expectedHash + " but the RPC returned " + returnedHash + ".");
        return;
      }

      setSubmittedHash(returnedHash);
      setSubmissionState("submitted");
      setStatus("Registration submitted to CKB Testnet; waiting for block confirmation. Do not submit it again.");
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
            "Atomic Item Registration confirmed on CKB Testnet in block " +
              String(confirmedTransaction.blockNumber) +
              ". Transaction hash: " +
              returnedHash,
          );
        } else {
          setStatus(
            "Registration was submitted, but block confirmation is not available yet. Check hash " +
              returnedHash +
              " before taking any further action.",
          );
        }
      } catch {
        setStatus(
          "Registration was submitted, but block confirmation could not be checked yet. Check hash " +
            returnedHash +
            " before taking any further action.",
        );
      }
    } catch (err) {
      if (sendStarted) {
        setSubmissionState("uncertain");
        setStatus("Submission outcome uncertain. Check the expected transaction hash with a Testnet explorer before any retry.");
        setError(err instanceof Error ? err.message : String(err));
      } else {
        setSubmissionState("ready");
        setStatus("Could not open the CKB Testnet client; the transaction was not submitted.");
        setError(err instanceof Error ? err.message : String(err));
      }
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
    setStatus("Checking block confirmation...");
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
          "Atomic Item Registration confirmed on CKB Testnet in block " +
            String(confirmedTransaction.blockNumber) +
            ". Transaction hash: " +
            submittedHash,
        );
      } else {
        setStatus(
          "Block confirmation is not available yet. Check hash " +
            submittedHash +
            " before taking any further action.",
        );
      }
    } catch (err) {
      setStatus(
        "Block confirmation could not be checked yet. Do not resubmit; check hash " +
          submittedHash +
          " with a Testnet explorer.",
      );
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

  const canSubmit = submissionState === "ready" && !busy && wallet?.address === preview.ownerAddress;

  return (
    <section style={{ border: "1px solid #888", borderRadius: 8, padding: 16, marginTop: 16 }}>
      <h3>CP11.5.3 — Submit Atomic Item Registration</h3>
      <p>
        This sends the signed transaction to CKB Testnet. It does not ask JoyID to sign again.
        Once accepted, the Registry input and funding inputs are consumed. Review this summary before continuing.
      </p>
      <p><strong>Network:</strong> CKB Testnet</p>
      <p><strong>Owner:</strong></p>
      <code style={{ overflowWrap: "anywhere" }}>{preview.ownerAddress}</code>
      <p style={{ marginTop: 8 }}><strong>Registry ID:</strong></p>
      <code style={{ overflowWrap: "anywhere" }}>{preview.registryId}</code>
      <p style={{ marginTop: 8 }}><strong>Item ID:</strong></p>
      <code style={{ overflowWrap: "anywhere" }}>{preview.itemId}</code>
      <p style={{ marginTop: 8 }}>
        Registry item count: {preview.previousItemCount} → {preview.newItemCount}
      </p>
      <p><strong>Registry input:</strong></p>
      <code style={{ overflowWrap: "anywhere" }}>{preview.registryCellOutPoint}</code>
      <p style={{ marginTop: 8 }}>
        Registry capacity: {formatCkb(preview.registryInputCapacity)} → {formatCkb(preview.registryOutputCapacity)} CKB
      </p>
      <p><strong>State output:</strong> AVAILABLE · {formatCkb(preview.stateCapacity)} CKB</p>
      <p><strong>Funding inputs:</strong> {preview.feeInputs.length}</p>
      {preview.feeInputs.map((input) => (
        <div key={input.outPoint}>
          <code style={{ overflowWrap: "anywhere" }}>{input.outPoint}</code>
          {" · "}{formatCkb(input.capacity)} CKB
        </div>
      ))}
      <p style={{ marginTop: 8 }}>
        Estimated fee: {formatCkb(preview.estimatedFee)} CKB; actual fee: {formatCkb(preview.actualFee)} CKB
      </p>
      <p>Change: {formatCkb(preview.change)} CKB</p>
      <p><strong>Raw transaction hash:</strong></p>
      <code style={{ overflowWrap: "anywhere" }}>{preview.transaction.hash()}</code>
      <p style={{ marginTop: 12 }}>
        Signed transaction hash: {signedTransaction.hash()}; signature remains local until you submit.
      </p>
      <button type="button" onClick={submitSignedRegistration} disabled={!canSubmit}>
        {busy
          ? "Submitting or checking confirmation..."
          : submissionState === "confirmed"
            ? "Registration confirmed"
            : submissionState === "submitted"
              ? "Submitted; do not resubmit"
              : submissionState === "uncertain"
                ? "Outcome uncertain; check hash before retrying"
                : "Submit signed registration to Testnet"}
      </button>
      {submissionState === "submitted" && !busy && (
        <p style={{ marginTop: 8 }}>
          <button type="button" onClick={checkConfirmation}>
            Check block confirmation
          </button>
        </p>
      )}
      {status && <p style={{ marginTop: 8 }}>{status}</p>}
      {submittedHash && (
        <p>
          RPC transaction hash: <code style={{ overflowWrap: "anywhere" }}>{submittedHash}</code>
        </p>
      )}
      {reportedHash && reportedHash.toLowerCase() !== preview.transaction.hash().toLowerCase() && (
        <p>
          RPC returned hash: <code style={{ overflowWrap: "anywhere" }}>{reportedHash}</code>
        </p>
      )}
      {error && (
        <pre style={{ whiteSpace: "pre-wrap", color: "crimson", overflowWrap: "anywhere" }}>
          {error}
        </pre>
      )}
      {submissionState === "uncertain" && (
        <p>Do not retry until the expected transaction hash is checked with a Testnet explorer.</p>
      )}
    </section>
  );
}