import { ccc } from "@ckb-ccc/ccc";
import { decodeStateCellData } from "./stateDataCodec";
import type { ItemStatus } from "./itemState";

export interface ItemHistoryEntry {
  txHash: string;
  blockNumber: string;
  transactionStatus: string;
  action: string;
  resultingStatus: ItemStatus | null;
}

const MAX_HISTORY_ENTRIES = 20;

function describeAction(
  hasStateInput: boolean,
  resultingStatus: ItemStatus | null,
): string {
  if (!hasStateInput && resultingStatus === "AVAILABLE") {
    return "Item registered";
  }
  if (resultingStatus === "BORROWED") {
    return "Borrow";
  }
  if (resultingStatus === "RETURNED") {
    return "Confirm return";
  }
  if (hasStateInput && resultingStatus === "AVAILABLE") {
    return "Make available";
  }
  return "State transaction";
}

export async function loadItemHistory(
  client: ccc.Client,
  stateType: ccc.Script,
): Promise<ItemHistoryEntry[]> {
  const records: Array<{
    txHash: string;
    blockNumber: string;
    cells: Array<{ isInput: boolean }>;
  }> = [];

  for await (const record of client.findTransactionsByType(
    stateType,
    true,
    "desc",
    MAX_HISTORY_ENTRIES,
  )) {
    records.push({
      txHash: String(record.txHash),
      blockNumber: String(record.blockNumber),
      cells: record.cells.map((cell) => ({
        isInput: cell.isInput,

      })),
    });
    if (records.length >= MAX_HISTORY_ENTRIES) {
      break;
    }
  }

  const entries: ItemHistoryEntry[] = [];
  for (const record of records) {
    const response = await client.getTransaction(record.txHash);
    if (!response) {
      entries.push({
        txHash: record.txHash,
        blockNumber: record.blockNumber,
        transactionStatus: "unavailable",
        action: "Transaction details unavailable from RPC",
        resultingStatus: null,
      });
      continue;
    }

    const transaction = response.transaction;
    const stateOutputIndex = transaction.outputs.findIndex((output) =>
      output.type?.hash() === stateType.hash()
    );
    let resultingStatus: ItemStatus | null = null;
    if (stateOutputIndex >= 0) {
      try {
        resultingStatus = decodeStateCellData(
          String(transaction.outputsData[stateOutputIndex]),
        ).status;
      } catch {
        resultingStatus = null;
      }
    }

    const hasStateInput = record.cells.some((cell) => cell.isInput);
    entries.push({
      txHash: record.txHash,
      blockNumber: String(response.blockNumber ?? record.blockNumber),
      transactionStatus: String(response.status),
      action: describeAction(hasStateInput, resultingStatus),
      resultingStatus,
    });
  }

  return entries;
}