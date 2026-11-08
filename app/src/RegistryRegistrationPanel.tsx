import { useEffect, useState } from "react";
import { ccc } from "@ckb-ccc/ccc";
import {
  buildAtomicRegistrationPreview,
  type AtomicRegistrationPreview,
} from "./domain/registryRegistrationPreview";
import type { VerifiedContractBinary } from "./domain/deploymentPreview";
import type { WalletSession } from "./domain/wallet";

interface RegistryRegistrationPanelProps {
  wallet: WalletSession | null;
  otherSigningBusy: boolean;
  onSigningBusyChange: (busy: boolean) => void;
  walletAddress: string | null;
  registryArtifact: VerifiedContractBinary | null;
  stateArtifact: VerifiedContractBinary | null;
  registryGenesisTxHash: string | null;
}

function formatCkb(shannons: string): string {
  const value = BigInt(shannons);
  const whole = value / 100_000_000n;
  const fraction = (value % 100_000_000n).toString().padStart(8, "0");
  return whole.toString() + "." + fraction;
}

function randomItemId(): string {
  const bytes = new Uint8Array(32);
  crypto.getRandomValues(bytes);
  return "0x" + Array.from(bytes, (byte) =>
    byte.toString(16).padStart(2, "0"),
  ).join("");
}

export default function RegistryRegistrationPanel({
  wallet,
  otherSigningBusy,
  onSigningBusyChange,
  walletAddress,
  registryArtifact,
  stateArtifact,
  registryGenesisTxHash,
}: RegistryRegistrationPanelProps) {
  const [itemIdInput, setItemIdInput] = useState("");
  const [preview, setPreview] = useState<AtomicRegistrationPreview | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [status, setStatus] = useState("");
  const [signedTransaction, setSignedTransaction] = useState<ccc.Transaction | null>(null);
  const [signBusy, setSignBusy] = useState(false);
  const [signError, setSignError] = useState("");
  const [signStatus, setSignStatus] = useState("");

  useEffect(() => {
    setPreview(null);
    setSignedTransaction(null);
    setError("");
    setStatus("");
    setSignError("");
    setSignStatus("");
  }, [
    walletAddress,
    registryArtifact?.codeHash,
    stateArtifact?.codeHash,
    registryGenesisTxHash,
  ]);

  function clearLocalSignature() {
    setSignedTransaction(null);
    setSignError("");
    setSignStatus("Local signature cleared. The unsigned preview is unchanged.");
  }

  async function signPreview() {
    if (!wallet || !preview || busy || signBusy || signedTransaction || otherSigningBusy) {
      return;
    }
    if (wallet.address !== walletAddress || wallet.address !== preview.ownerAddress) {
      setSignStatus("");
      setSignError("The connected wallet differs from this preview owner. Rebuild the preview before signing.");
      return;
    }

    setSignBusy(true);
    onSigningBusyChange(true);
    setSignError("");
    setSignStatus("Review the Atomic Item Registration signing request in JoyID...");
    try {
      const expectedHash = preview.transaction.hash();
      const signed = await wallet.signTransaction(preview.transaction);
      if (signed.hash().toLowerCase() !== expectedHash.toLowerCase()) {
        throw new Error("JoyID changed the registration transaction fields. Do not use this result; rebuild and review a fresh preview.");
      }
      if (!signed.witnesses.some((witness) => String(witness).toLowerCase() !== "0x")) {
        throw new Error("JoyID returned no signature witness for Atomic Item Registration.");
      }

      setSignedTransaction(signed);
      setSignStatus("JoyID signed the reviewed registration transaction. The signature remains local and has not been submitted.");
    } catch (err) {
      setSignStatus("");
      setSignError(err instanceof Error ? err.message : String(err));
    } finally {
      setSignBusy(false);
      onSigningBusyChange(false);
    }
  }
  function generateItemId() {
    try {
      setItemIdInput(randomItemId());
      setPreview(null);
      setSignedTransaction(null);
      setSignError("");
      setSignStatus("");
      setError("");
      setStatus("Generated a random 32-byte Item ID. It has not been registered.");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function createPreview() {
    if (
      !walletAddress ||
      !registryArtifact ||
      !stateArtifact ||
      !itemIdInput.trim() ||
      busy
    ) {
      return;
    }

    setBusy(true);
    setError("");
    setStatus("Reading live Registry and wallet cells, then building an unsigned registration preview...");
    setPreview(null);
    let clientOwner: ReturnType<typeof ccc.ClientPublicTestnet.open> | undefined;

    try {
      clientOwner = ccc.ClientPublicTestnet.open();
      const nextPreview = await buildAtomicRegistrationPreview(
        clientOwner.value,
        walletAddress,
        itemIdInput,
        registryArtifact,
        stateArtifact,
        registryGenesisTxHash ?? undefined,
      );
      setPreview(nextPreview);
      setStatus("Unsigned Atomic Item Registration preview is ready. It has not been signed or submitted.");
    } catch (err) {
      setStatus("Could not build the Atomic Item Registration preview.");
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      if (clientOwner) {
        try {
          await clientOwner.dispose();
        } catch {
          // Cleanup errors do not change the preview result.
        }
      }
      setBusy(false);
    }
  }

  return (
    <section style={{ border: "1px solid #888", borderRadius: 8, padding: 16, marginBottom: 24 }}>
      <h2>CP11.5.1 — Atomic Item Registration Preview</h2>
      <p>
        Build an unsigned transaction that advances the live Registry and creates an initial AVAILABLE State cell.
        This step only reads Testnet cells; it does not sign or submit.
      </p>
      <p style={{ marginTop: 12 }}>
        <label htmlFor="registration-item-id"><strong>Item ID (32 bytes / 64 hex characters)</strong></label>
      </p>
      <p>
        <input
          id="registration-item-id"
          type="text"
          value={itemIdInput}
          onChange={(event) => {
            setItemIdInput(event.target.value);
            setPreview(null);
            setSignedTransaction(null);
            setSignError("");
            setSignStatus("");
            setError("");
            setStatus("");
          }}
          placeholder="0x + 64 hexadecimal characters"
          disabled={busy || signBusy || Boolean(signedTransaction)}
          style={{ width: "min(100%, 620px)" }}
        />
      </p>
      <p>
        <button type="button" onClick={generateItemId} disabled={busy || signBusy || Boolean(signedTransaction)}>
          Generate random 32-byte Item ID
        </button>{" "}
        <button
          type="button"
          onClick={createPreview}
          disabled={
            !walletAddress ||
            !registryArtifact ||
            !stateArtifact ||
            !itemIdInput.trim() ||
            signBusy ||
            Boolean(signedTransaction) ||
            busy
          }
        >
          {busy ? "Building registration preview..." : "Build unsigned registration preview"}
        </button>
      </p>
      {!walletAddress && <p>Connect JoyID on CKB Testnet first.</p>}
      {walletAddress && (!registryArtifact || !stateArtifact) && (
        <p>Verify both CP11.1 contract binaries in Checkpoint 2 first.</p>
      )}
      {status && <p style={{ marginTop: 8 }}>{status}</p>}
      {error && (
        <pre style={{ whiteSpace: "pre-wrap", color: "crimson", overflowWrap: "anywhere" }}>
          {error}
        </pre>
      )}
      {preview && (
        <div style={{ marginTop: 16, textAlign: "left" }}>
          <p><strong>Network:</strong> CKB Testnet</p>
          <p><strong>Owner:</strong></p>
          <code style={{ overflowWrap: "anywhere" }}>{preview.ownerAddress}</code>
          <p style={{ marginTop: 8 }}><strong>Item ID:</strong></p>
          <code style={{ overflowWrap: "anywhere" }}>{preview.itemId}</code>
          <p style={{ marginTop: 8 }}><strong>Registry ID:</strong></p>
          <code style={{ overflowWrap: "anywhere" }}>{preview.registryId}</code>
          <p style={{ marginTop: 8 }}>
            Registry item count: {preview.previousItemCount} → {preview.newItemCount}
          </p>
          <p><strong>Registry input:</strong></p>
          <code style={{ overflowWrap: "anywhere" }}>{preview.registryCellOutPoint}</code>
          <p style={{ marginTop: 8 }}>
            Registry capacity: {formatCkb(preview.registryInputCapacity)} → {formatCkb(preview.registryOutputCapacity)} CKB
          </p>
          <p style={{ marginTop: 8 }}><strong>Registry Type Script hash:</strong></p>
          <code style={{ overflowWrap: "anywhere" }}>{preview.registryTypeScriptHash}</code>
          <p style={{ marginTop: 8 }}>
            <strong>State output #1:</strong> AVAILABLE · {formatCkb(preview.stateCapacity)} CKB
          </p>
          <p><strong>State Type Script hash:</strong></p>
          <code style={{ overflowWrap: "anywhere" }}>{preview.stateTypeScriptHash}</code>
          <p style={{ marginTop: 8 }}><strong>Registry code cell dependency:</strong></p>
          <code style={{ overflowWrap: "anywhere" }}>{preview.registryCodeOutPoint}</code>
          <p style={{ marginTop: 8 }}><strong>State code cell dependency:</strong></p>
          <code style={{ overflowWrap: "anywhere" }}>{preview.stateCodeOutPoint}</code>
          <p style={{ marginTop: 8 }}><strong>Funding inputs:</strong></p>
          {preview.feeInputs.map((input) => (
            <div key={input.outPoint}>
              <code style={{ overflowWrap: "anywhere" }}>{input.outPoint}</code>
              {" · "}{formatCkb(input.capacity)} CKB
            </div>
          ))}
          <p style={{ marginTop: 8 }}>
            Estimated fee: {formatCkb(preview.estimatedFee)} CKB at {preview.feeRate} shannons/KB
          </p>
          <p>Transaction fee from input/output balance: {formatCkb(preview.actualFee)} CKB</p>
          <p>Change: {formatCkb(preview.change)} CKB</p>
          <p><strong>Unsigned transaction hash:</strong></p>
          <code style={{ overflowWrap: "anywhere" }}>{preview.transaction.hash()}</code>
          <details style={{ marginTop: 16 }}>
            <summary>Raw unsigned Atomic Item Registration transaction JSON</summary>
            <pre
              style={{
                whiteSpace: "pre-wrap",
                overflowWrap: "anywhere",
                maxHeight: 420,
                overflow: "auto",
                fontSize: 12,
              }}
            >
              {JSON.stringify(JSON.parse(ccc.stringify(preview.transaction)), null, 2)}
            </pre>
          </details>
          <p style={{ marginTop: 12 }}>
            Review this transaction before the separate signing and Testnet submission checkpoints.
          </p>
          <section style={{ border: "1px solid #888", borderRadius: 8, padding: 16, marginTop: 20 }}>
            <h3>CP11.5.2 — Sign Atomic Item Registration</h3>
            <p>
              This asks JoyID to sign the reviewed transaction. It does not submit or broadcast it.
              Verify the Item ID, Registry update, State output, fee, and change above before signing.
            </p>
            <button
              type="button"
              onClick={signPreview}
              disabled={
                !wallet ||
                wallet.address !== walletAddress ||
                wallet.address !== preview.ownerAddress ||
                busy ||
                signBusy ||
                otherSigningBusy ||
                Boolean(signedTransaction)
              }
            >
              {signBusy
                ? "Waiting for JoyID..."
                : signedTransaction
                  ? "Registration signed locally"
                  : "Sign registration preview in JoyID"}
            </button>
            {signStatus && <p style={{ marginTop: 8 }}>{signStatus}</p>}
            {signError && (
              <pre style={{ whiteSpace: "pre-wrap", color: "crimson", overflowWrap: "anywhere" }}>
                {signError}
              </pre>
            )}
            {signedTransaction && (
              <div style={{ border: "1px solid #aaa", borderRadius: 8, padding: 12, marginTop: 12 }}>
                <h4>Signed registration — not submitted</h4>
                <p>Raw transaction hash (must match the reviewed preview):</p>
                <code style={{ overflowWrap: "anywhere" }}>{signedTransaction.hash()}</code>
                <p style={{ marginTop: 8 }}>Full transaction hash including witnesses:</p>
                <code style={{ overflowWrap: "anywhere" }}>{signedTransaction.hashFull()}</code>
                <p style={{ marginTop: 8 }}>
                  Witnesses: {signedTransaction.witnesses.length}; signature present.
                </p>
                <details style={{ marginTop: 12 }}>
                  <summary>Signed Atomic Item Registration transaction JSON</summary>
                  <pre style={{ whiteSpace: "pre-wrap", overflowWrap: "anywhere", maxHeight: 420, overflow: "auto", fontSize: 12 }}>
                    {JSON.stringify(JSON.parse(ccc.stringify(signedTransaction)), null, 2)}
                  </pre>
                </details>
                <button type="button" onClick={clearLocalSignature} style={{ marginTop: 12 }}>
                  Clear local signature
                </button>
              </div>
            )}
          </section>
        </div>
      )}
    </section>
  );
}