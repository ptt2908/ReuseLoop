import { ccc } from "@ckb-ccc/ccc";
import {
  decodeRegistryData,
  decodeRegistryScriptArgs,
} from "./registryCodec";
import { decodeStateScriptArgs } from "./stateCodec";
import { decodeStateCellData } from "./stateDataCodec";
import type { VerifiedContractBinary } from "./deploymentPreview";

const MAX_OWNER_CELLS = 500;
const REGISTRY_DATA1_HASH_TYPE_CODE = 0x02;

export interface ItemLookupResult {
  itemId: string;
  status: "AVAILABLE" | "BORROWED" | "RETURNED";
  registryId: string;
  registryItemCount: number;
  registryOutPoint: string;
  registryCapacity: string;
  registryTypeScriptHash: string;
  stateOutPoint: string;
  stateCapacity: string;
  stateTypeScriptHash: string;
  stateType: ccc.Script;
}

function normalizeItemId(value: string): string {
  const raw = value.trim().toLowerCase().replace(/^0x/, "");
  if (!/^[0-9a-f]{64}$/.test(raw)) {
    throw new Error("Item ID must contain exactly 32 bytes of hexadecimal data.");
  }
  return "0x" + raw;
}

function outPointText(cell: ccc.Cell): string {
  return String(cell.outPoint.txHash) + ":" +
    ccc.numFrom(cell.outPoint.index).toString();
}

function matchesCodeIdentity(
  script: ccc.Script | null | undefined,
  codeHash: string,
  hashType: string,
): script is ccc.Script {
  return Boolean(
    script &&
    String(script.codeHash).toLowerCase() === codeHash.toLowerCase() &&
    String(script.hashType) === hashType
  );
}

export async function lookupItemById(
  client: ccc.Client,
  ownerAddress: string,
  itemIdInput: string,
  registryArtifact: VerifiedContractBinary,
  stateArtifact: VerifiedContractBinary,
): Promise<ItemLookupResult> {
  if (registryArtifact.kind !== "registry" || stateArtifact.kind !== "state") {
    throw new Error("Verify the Registry and State binaries before looking up an item.");
  }

  const itemId = normalizeItemId(itemIdInput);
  const address = await ccc.Address.fromString(ownerAddress, client);
  const ownerLock = address.script;
  const indexedCells: ccc.Cell[] = [];

  for await (const cell of client.findCellsByLock(
    ownerLock,
    undefined,
    true,
    "desc",
    MAX_OWNER_CELLS,
  )) {
    indexedCells.push(cell);
  }

  const registryCandidates = indexedCells.filter((cell) =>
    matchesCodeIdentity(
      cell.cellOutput.type,
      registryArtifact.codeHash,
      "data1",
    )
  );
  const liveRegistryCells = new Map<string, ccc.Cell>();
  for (const candidate of registryCandidates) {
    const liveCell = await client.getCellLive(candidate.outPoint, true);
    if (
      liveCell?.cellOutput.type &&
      matchesCodeIdentity(
        liveCell.cellOutput.type,
        registryArtifact.codeHash,
        "data1",
      ) &&
      liveCell.cellOutput.lock.hash() === ownerLock.hash()
    ) {
      liveRegistryCells.set(outPointText(liveCell).toLowerCase(), liveCell);
    }
  }

  if (liveRegistryCells.size === 0) {
    throw new Error("No live Registry cell for this wallet was found on CKB Testnet.");
  }
  if (liveRegistryCells.size !== 1) {
    throw new Error("Multiple live Registry cells were found. Resolve the canonical Registry before lookup.");
  }

  const registryCell = [...liveRegistryCells.values()][0];
  const registryType = registryCell.cellOutput.type;
  if (!registryType) {
    throw new Error("The live Registry cell is missing its Type Script.");
  }

  const registryArgs = decodeRegistryScriptArgs(registryType.args);
  const registryData = decodeRegistryData(registryCell.outputData);
  if (registryArgs.adminLockHash !== ownerLock.hash()) {
    throw new Error("Registry administrator Lock hash does not match the queried wallet.");
  }
  if (registryData.registryId !== registryArgs.registryId) {
    throw new Error("Registry data ID does not match the Registry Type Script args.");
  }
  if (!registryData.itemIds.includes(itemId)) {
    throw new Error("Item ID is not registered in the current live Registry.");
  }
  if (
    registryArgs.stateCodeHash.toLowerCase() !== stateArtifact.codeHash.toLowerCase() ||
    registryArgs.stateHashType !== REGISTRY_DATA1_HASH_TYPE_CODE
  ) {
    throw new Error("The live Registry does not reference the verified State binary.");
  }

  const stateCandidates = indexedCells.filter((cell) =>
    matchesCodeIdentity(
      cell.cellOutput.type,
      stateArtifact.codeHash,
      "data1",
    )
  );
  const liveStateCells = new Map<string, ccc.Cell>();
  for (const candidate of stateCandidates) {
    const candidateType = candidate.cellOutput.type;
    if (!candidateType) {
      continue;
    }
    const candidateArgs = decodeStateScriptArgs(candidateType.args);
    if (candidateArgs.itemId !== itemId) {
      continue;
    }

    const liveCell = await client.getCellLive(candidate.outPoint, true);
    if (!liveCell?.cellOutput.type) {
      continue;
    }
    const liveType = liveCell.cellOutput.type;
    if (!matchesCodeIdentity(liveType, stateArtifact.codeHash, "data1")) {
      continue;
    }

    const stateArgs = decodeStateScriptArgs(liveType.args);
    const stateData = decodeStateCellData(liveCell.outputData);
    if (stateArgs.itemId !== itemId || stateData.itemId !== itemId) {
      throw new Error("State cell Item ID does not match the requested Item ID.");
    }
    if (stateArgs.trustedRegistryTypeHash !== registryType.hash()) {
      throw new Error("State cell does not trust the current canonical Registry.");
    }
    if (stateArgs.adminLockHash !== ownerLock.hash()) {
      throw new Error("State cell administrator Lock hash does not match the queried wallet.");
    }
    if (liveCell.cellOutput.lock.hash() !== ownerLock.hash()) {
      throw new Error("State cell is not locked by the queried administrator wallet.");
    }
    liveStateCells.set(outPointText(liveCell).toLowerCase(), liveCell);
  }

  if (liveStateCells.size === 0) {
    throw new Error("The Registry contains this item, but no live State cell was found.");
  }
  if (liveStateCells.size !== 1) {
    throw new Error("Multiple live State cells were found for this Item ID.");
  }

  const stateCell = [...liveStateCells.values()][0];
  const stateType = stateCell.cellOutput.type;
  if (!stateType) {
    throw new Error("The live State cell is missing its Type Script.");
  }


  const stateData = decodeStateCellData(stateCell.outputData);
  return {
    itemId,
    status: stateData.status,
    registryId: registryData.registryId,
    registryItemCount: registryData.itemCount,
    registryOutPoint: outPointText(registryCell),
    registryCapacity: String(registryCell.cellOutput.capacity),
    registryTypeScriptHash: registryType.hash(),
    stateOutPoint: outPointText(stateCell),
    stateCapacity: String(stateCell.cellOutput.capacity),
    stateTypeScriptHash: stateType.hash(),
    stateType,
  };
}