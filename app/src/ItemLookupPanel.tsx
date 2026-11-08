import { useState } from "react";
import { ccc } from "@ckb-ccc/ccc";
import { QRCodeSVG } from "qrcode.react";
import type { VerifiedContractBinary } from "./domain/deploymentPreview";
import { lookupItemById, type ItemLookupResult } from "./domain/itemLookup";
import { loadItemHistory, type ItemHistoryEntry } from "./domain/itemHistory";

interface ItemLookupPanelProps {
  ownerAddress: string | null;
  registryArtifact: VerifiedContractBinary | null;
  stateArtifact: VerifiedContractBinary | null;
}

function formatCkb(shannons: string): string {
  const value = BigInt(shannons);
  const whole = value / 100_000_000n;
  const fraction = (value % 100_000_000n).toString().padStart(8, "0");
  return whole.toString() + "." + fraction;
}

function statusColor(status: ItemLookupResult["status"]): string {
  if (status === "AVAILABLE") return "#16803c";
  if (status === "BORROWED") return "#9a6700";
  return "#0969da";
}

export default function ItemLookupPanel({
  ownerAddress,
  registryArtifact,
  stateArtifact,
}: ItemLookupPanelProps) {
  const [itemId, setItemId] = useState(() =>
    new URLSearchParams(window.location.search).get("item") ?? "",
  );
  const [result, setResult] = useState<ItemLookupResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [status, setStatus] = useState("");
  const [copyStatus, setCopyStatus] = useState("");
  const [history, setHistory] = useState<ItemHistoryEntry[]>([]);
  const [historyError, setHistoryError] = useState("");

  async function findItem() {
    if (busy) return;
    if (!ownerAddress) {
      setError("Connect the administrator wallet before looking up an item.");
      return;
    }
    if (!registryArtifact || !stateArtifact) {
      setError("Verify both CP11.1 binaries in Checkpoint 2 before looking up an item.");
      return;
    }

    setBusy(true);
    setHistory([]);
    setHistoryError("");
    setResult(null);
    setError("");
    setCopyStatus("");
    setStatus("Reading the live Registry and State cells from CKB Testnet...");
    let clientOwner: ReturnType<typeof ccc.ClientPublicTestnet.open> | undefined;

    try {
      clientOwner = ccc.ClientPublicTestnet.open();
      const found = await lookupItemById(
        clientOwner.value,
        ownerAddress,
        itemId,
        registryArtifact,
        stateArtifact,
      );
      setResult(found);
      try {
        setHistory(await loadItemHistory(clientOwner.value, found.stateType));
      } catch (historyErr) {
        setHistoryError(historyErr instanceof Error ? historyErr.message : String(historyErr));
      }
      setItemId(found.itemId);
      const url = new URL(window.location.href);
      url.searchParams.set("item", found.itemId);
      window.history.replaceState(null, "", url.toString());
      setStatus("Item status verified from the current live Testnet cells.");
    } catch (err) {
      setStatus("Item lookup failed.");
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      if (clientOwner) {
        try {
          await clientOwner.dispose();
        } catch {
          // Cleanup errors do not change the lookup result.
        }
      }
      setBusy(false);
    }
  }

  async function copyLookupLink() {
    if (!result) return;
    const url = new URL(window.location.href);
    url.searchParams.set("item", result.itemId);
    try {
      await navigator.clipboard.writeText(url.toString());
      setCopyStatus("Lookup link copied.");
    } catch (err) {
      setCopyStatus(
        "Could not copy the link. Select and copy it manually: " + url.toString(),
      );
    }
  }

  return (
    <section style={{ border: "1px solid #888", borderRadius: 8, padding: 16, margin: "24px 0" }}>
      <h2>CP11.6 — Item Lookup and Status</h2>
      <p>
        Look up an item by its 32-byte ID. Results are verified against the live Registry and State cells on CKB Testnet.
        The QR code contains the Item ID only.
      </p>
      <label htmlFor="item-lookup-id"><strong>Item ID</strong></label>
      <p>
        <input
          id="item-lookup-id"
          value={itemId}
          onChange={(event) => {
            setItemId(event.target.value);
            setHistory([]);
            setHistoryError("");
            setResult(null);
            setError("");
            setStatus("");
            setCopyStatus("");
          }}
          placeholder="0x + 64 hexadecimal characters"
          style={{ width: "min(100%, 620px)" }}
        />
      </p>
      <button
        type="button"
        onClick={findItem}
        disabled={busy || !ownerAddress || !registryArtifact || !stateArtifact || !itemId.trim()}
      >
        {busy ? "Looking up live cells..." : "Look up item"}
      </button>
      {!ownerAddress && <p>Connect the administrator wallet first.</p>}
      {ownerAddress && (!registryArtifact || !stateArtifact) && (
        <p>Verify both Registry and State binaries in Checkpoint 2 first.</p>
      )}
      {status && <p role="status">{status}</p>}
      {error && (
        <pre role="alert" style={{ whiteSpace: "pre-wrap", color: "crimson", overflowWrap: "anywhere" }}>
          {error}
        </pre>
      )}
      {result && (
        <section style={{ border: "1px solid #aaa", borderRadius: 8, padding: 16, marginTop: 16 }}>
          <h3>
            Current status: <span style={{ color: statusColor(result.status) }}>{result.status}</span>
          </h3>
          <p><strong>Item ID:</strong></p>
          <code style={{ overflowWrap: "anywhere" }}>{result.itemId}</code>
          <p style={{ marginTop: 8 }}><strong>Registry ID:</strong></p>
          <code style={{ overflowWrap: "anywhere" }}>{result.registryId}</code>
          <p style={{ marginTop: 8 }}>
            Registry item count: {result.registryItemCount}
          </p>
          <p><strong>Live State cell:</strong></p>
          <code style={{ overflowWrap: "anywhere" }}>{result.stateOutPoint}</code>
          <p>Capacity: {formatCkb(result.stateCapacity)} CKB</p>
          <p>State Type Script hash:</p>
          <code style={{ overflowWrap: "anywhere" }}>{result.stateTypeScriptHash}</code>
          <p style={{ marginTop: 8 }}><strong>Canonical Registry cell:</strong></p>
          <code style={{ overflowWrap: "anywhere" }}>{result.registryOutPoint}</code>
          <p>Capacity: {formatCkb(result.registryCapacity)} CKB</p>
          <p>Registry Type Script hash:</p>
          <code style={{ overflowWrap: "anywhere" }}>{result.registryTypeScriptHash}</code>
          <div style={{ marginTop: 16 }}>
            <QRCodeSVG value={result.itemId} size={220} level="M" includeMargin />
            <p>Scan this QR code to copy the Item ID, then paste it into ReuseLoop lookup.</p>
          </div>
          <p>
            <button type="button" onClick={copyLookupLink}>Copy lookup link</button>
          </p>
          {copyStatus && <p role="status">{copyStatus}</p>}
          <h3 style={{ marginTop: 20 }}>Recent on-chain history</h3>
          {historyError && <p role="alert">History could not be loaded: {historyError}</p>}
          {!historyError && history.length === 0 && <p>No State transactions were returned by the Testnet indexer.</p>}
          {history.map((entry) => (
            <div key={entry.txHash} style={{ borderTop: "1px solid #ddd", padding: "10px 0" }}>
              <strong>{entry.action}</strong>{entry.resultingStatus ? " → " + entry.resultingStatus : ""}
              <div>Block {entry.blockNumber} · {entry.transactionStatus}</div>
              <code style={{ overflowWrap: "anywhere" }}>{entry.txHash}</code>
            </div>
          ))}
        </section>
      )}
    </section>
  );
}