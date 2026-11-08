import { useState } from "react";
import { ccc } from "@ckb-ccc/ccc";
import {
  buildAtomicRegistrationPreview,
  type AtomicRegistrationPreview,
} from "./domain/registryRegistrationPreview";
import type { VerifiedContractBinary } from "./domain/deploymentPreview";

interface RegistryRegistrationPanelProps {
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

  function generateItemId() {
    try {
      setItemIdInput(randomItemId());
      setPreview(null);
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
            setError("");
            setStatus("");
          }}
          placeholder="0x + 64 hexadecimal characters"
          disabled={busy}
          style={{ width: "min(100%, 620px)" }}
        />
      </p>
      <p>
        <button type="button" onClick={generateItemId} disabled={busy}>
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
        </div>
      )}
    </section>
  );
}