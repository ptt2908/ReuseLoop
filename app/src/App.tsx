
import { useState } from "react";
import { connectJoyIdTestnet, type WalletSession } from "./domain/wallet";
import { ccc } from "@ckb-ccc/ccc";

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

export default function App() {
  const [cells, setCells] = useState<CellInfo[]>([]);
  const [status, setStatus] = useState("Ready");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [wallet, setWallet] = useState<WalletSession | null>(null);
  const [walletBusy, setWalletBusy] = useState(false);

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
      setStatus(`Found ${results.length} live Cell(s).`);
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
    if (!wallet) {
      return;
    }

    const session = wallet;
    setWallet(null);
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
              <button onClick={disconnectWallet} disabled={walletBusy}>
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
          key={`${cell.txHash}-${cell.index}`}
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
          <p>
            <strong>Capacity:</strong> {cell.capacity} shannons
          </p>
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
