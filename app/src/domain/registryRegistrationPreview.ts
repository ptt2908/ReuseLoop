import { ccc } from "@ckb-ccc/ccc";
import {
  decodeRegistryData,
  decodeRegistryScriptArgs,
} from "./registryCodec";
import {
  buildAtomicRegistrationTransaction,
} from "./transactionBuilders";
import type { VerifiedContractBinary } from "./deploymentPreview";

const MAX_WALLET_CELLS = 100;
const FEE_WITNESS_RESERVE_BYTES = 4096;
const DATA1_HASH_TYPE = "data1";
const DATA1_HASH_TYPE_CODE = 0x02;

export interface AtomicRegistrationFeeInput {
  outPoint: string;
  capacity: string;
}

export interface AtomicRegistrationPreview {
  transaction: ccc.Transaction;
  ownerAddress: string;
  itemId: string;
  registryId: string;
  previousItemCount: number;
  newItemCount: number;
  registryCellOutPoint: string;
  registryInputCapacity: string;
  registryOutputCapacity: string;
  registryTypeScriptHash: string;
  stateTypeScriptHash: string;
  stateCapacity: string;
  registryCodeOutPoint: string;
  stateCodeOutPoint: string;
  feeInputs: AtomicRegistrationFeeInput[];
  estimatedFee: string;
  actualFee: string;
  feeRate: string;
  change: string;
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

function sameOutPoint(left: ccc.Cell, right: ccc.Cell): boolean {
  return outPointText(left).toLowerCase() === outPointText(right).toLowerCase();
}

function hasCodeIdentity(
  scriptLike: ccc.Script | null | undefined,
  codeHash: string,
  hashType: string,
): boolean {
  return Boolean(
    scriptLike &&
    String(scriptLike.codeHash).toLowerCase() === codeHash.toLowerCase() &&
    String(scriptLike.hashType) === hashType
  );
}

async function findLiveArtifactCell(
  client: ccc.Client,
  indexedCells: ccc.Cell[],
  artifact: VerifiedContractBinary,
): Promise<ccc.Cell> {
  const matches = indexedCells.filter((cell) =>
    cell.cellOutput.type != null &&
    String(cell.outputData).toLowerCase() === artifact.data.toLowerCase() &&
    ccc.hashCkb(cell.outputData) === artifact.codeHash
  );
  const liveMatches: ccc.Cell[] = [];
  for (const match of matches) {
    const liveCell = await client.getCellLive(match.outPoint, true);
    if (
      liveCell &&
      liveCell.cellOutput.type != null &&
      String(liveCell.outputData).toLowerCase() === artifact.data.toLowerCase() &&
      ccc.hashCkb(liveCell.outputData) === artifact.codeHash
    ) {
      liveMatches.push(liveCell);
    }
  }
  if (liveMatches.length === 0) {
    throw new Error(
      "No live " + artifact.kind + " code cell matching the verified binary was found for this wallet.",
    );
  }
  if (liveMatches.length > 1) {
    throw new Error(
      "More than one live " + artifact.kind + " code cell matches the verified binary. Resolve the duplicate before building a preview.",
    );
  }
  return liveMatches[0];
}

function validateRegistryCell(
  cell: ccc.Cell,
  ownerLock: ccc.Script,
  registryArtifact: VerifiedContractBinary,
  stateArtifact: VerifiedContractBinary,
) {
  const registryType = cell.cellOutput.type;
  if (!registryType || !hasCodeIdentity(registryType, registryArtifact.codeHash, DATA1_HASH_TYPE)) {
    throw new Error("The selected live cell is not the verified Registry Type Script.");
  }
  if (cell.cellOutput.lock.hash() !== ownerLock.hash()) {
    throw new Error("The live Registry cell is not locked by the connected wallet.");
  }

  const registryArgs = decodeRegistryScriptArgs(registryType.args);
  const registryData = decodeRegistryData(cell.outputData);
  if (registryArgs.adminLockHash !== ownerLock.hash()) {
    throw new Error("The Registry administrator Lock hash does not match the connected wallet.");
  }
  if (
    registryArgs.stateCodeHash !== stateArtifact.codeHash.toLowerCase() ||
    registryArgs.stateHashType !== DATA1_HASH_TYPE_CODE
  ) {
    throw new Error("The live Registry does not reference the verified State binary.");
  }
  if (registryData.registryId !== registryArgs.registryId) {
    throw new Error("The live Registry data ID does not match its Type Script args.");
  }

  return { registryType, registryArgs, registryData };
}

async function findLiveRegistryCell(
  client: ccc.Client,
  indexedCells: ccc.Cell[],
  ownerLock: ccc.Script,
  registryArtifact: VerifiedContractBinary,
  stateArtifact: VerifiedContractBinary,
  registryGenesisTxHash?: string,
): Promise<ccc.Cell> {
  const matches = new Map<string, ccc.Cell>();
  if (registryGenesisTxHash) {
    const genesisOutput = await client.getCellLive(
      { txHash: registryGenesisTxHash, index: 0 },
      true,
    );
    if (genesisOutput?.cellOutput.type) {
      validateRegistryCell(genesisOutput, ownerLock, registryArtifact, stateArtifact);
      matches.set(outPointText(genesisOutput).toLowerCase(), genesisOutput);
    }
  }

  const indexedRegistryCells = indexedCells.filter((cell) =>
    hasCodeIdentity(cell.cellOutput.type, registryArtifact.codeHash, DATA1_HASH_TYPE)
  );
  for (const candidate of indexedRegistryCells) {
    const liveCell = await client.getCellLive(candidate.outPoint, true);
    if (!liveCell?.cellOutput.type) {
      continue;
    }
    validateRegistryCell(liveCell, ownerLock, registryArtifact, stateArtifact);
    matches.set(outPointText(liveCell).toLowerCase(), liveCell);
  }

  if (matches.size === 0) {
    throw new Error(
      "No live Registry cell was found for this wallet. Confirm the Genesis output is live and the Testnet cell indexer has caught up.",
    );
  }
  if (matches.size > 1) {
    throw new Error(
      "More than one live Registry cell was found for this wallet. Select the canonical Registry before registering an item.",
    );
  }
  return [...matches.values()][0];
}

export async function buildAtomicRegistrationPreview(
  client: ccc.Client,
  ownerAddress: string,
  itemIdInput: string,
  registryArtifact: VerifiedContractBinary,
  stateArtifact: VerifiedContractBinary,
  registryGenesisTxHash?: string,
): Promise<AtomicRegistrationPreview> {
  if (registryArtifact.kind !== "registry" || stateArtifact.kind !== "state") {
    throw new Error("Select verified Registry and State binaries in their matching fields.");
  }
  if (registryArtifact.codeHash === stateArtifact.codeHash) {
    throw new Error("Registry and State binaries must have different code hashes.");
  }
  const itemId = normalizeItemId(itemIdInput);
  const owner = await ccc.Address.fromString(ownerAddress, client);
  const ownerLock = owner.script;
  const joyIdInfo = await client.getKnownScript(ccc.KnownScript.JoyId);
  if (
    ownerLock.codeHash !== joyIdInfo.codeHash ||
    ownerLock.hashType !== joyIdInfo.hashType
  ) {
    throw new Error("Atomic registration preview currently supports a JoyID Testnet Lock Script only.");
  }

  const indexedCells: ccc.Cell[] = [];
  for await (const cell of client.findCellsByLock(
    ownerLock,
    undefined,
    true,
    "desc",
    MAX_WALLET_CELLS,
  )) {
    indexedCells.push(cell);
  }

  const registryCell = await findLiveRegistryCell(
    client,
    indexedCells,
    ownerLock,
    registryArtifact,
    stateArtifact,
    registryGenesisTxHash,
  );
  const registryType = registryCell.cellOutput.type;
  if (!registryType) {
    throw new Error("The live Registry cell is missing its Type Script.");
  }
  const { registryData } = validateRegistryCell(
    registryCell,
    ownerLock,
    registryArtifact,
    stateArtifact,
  );
  const registryCodeCell = await findLiveArtifactCell(client, indexedCells, registryArtifact);
  const stateCodeCell = await findLiveArtifactCell(client, indexedCells, stateArtifact);
  if (
    sameOutPoint(registryCell, registryCodeCell) ||
    sameOutPoint(registryCell, stateCodeCell)
  ) {
    throw new Error("The Registry state cell cannot also be used as a code dependency.");
  }

  const feeCandidates = indexedCells
    .filter((cell) =>
      cell.cellOutput.type == null &&
      String(cell.outputData).toLowerCase() === "0x"
    )
    .sort((left, right) => {
      const leftCapacity = ccc.numFrom(left.cellOutput.capacity);
      const rightCapacity = ccc.numFrom(right.cellOutput.capacity);
      return leftCapacity > rightCapacity ? -1 : leftCapacity < rightCapacity ? 1 : 0;
    });
  const excludedOutPoints = new Set([
    outPointText(registryCell).toLowerCase(),
    outPointText(registryCodeCell).toLowerCase(),
    outPointText(stateCodeCell).toLowerCase(),
  ]);
  const liveFeeCells: ccc.Cell[] = [];
  for (const candidate of feeCandidates) {
    if (excludedOutPoints.has(outPointText(candidate).toLowerCase())) {
      continue;
    }
    const liveCell = await client.getCellLive(candidate.outPoint, true);
    if (
      liveCell &&
      liveCell.cellOutput.type == null &&
      String(liveCell.outputData).toLowerCase() === "0x"
    ) {
      liveFeeCells.push(liveCell);
    }
  }
  if (liveFeeCells.length === 0) {
    throw new Error(
      "No live empty, untyped wallet cells are available to fund the registration fee and State output.",
    );
  }

  const joyIdCellDeps = await client.getCellDeps(joyIdInfo.cellDeps);
  const registryCodeDep = ccc.CellDep.from({
    outPoint: registryCodeCell.outPoint,
    depType: "code",
  });
  const stateCodeDep = ccc.CellDep.from({
    outPoint: stateCodeCell.outPoint,
    depType: "code",
  });
  const cellDeps = [...joyIdCellDeps, registryCodeDep, stateCodeDep];
  const feeRate = ccc.numFrom(await client.getFeeRate());
  let feeInputs: ccc.Cell[] = [];
  let feeProbe: ccc.Transaction | undefined;
  let transaction: ccc.Transaction | undefined;
  let estimatedFee = 0n;

  for (const feeCell of liveFeeCells) {
    feeInputs = [...feeInputs, feeCell];
    try {
      feeProbe = buildAtomicRegistrationTransaction({
        registryCell,
        itemId,
        feeInputs,
        cellDeps,
        fee: 0n,
      });
      feeProbe.setWitness(
        0,
        ccc.WitnessArgs.from({
          lock: "0x" + "00".repeat(FEE_WITNESS_RESERVE_BYTES),
        }).toBytes(),
      );
      estimatedFee = ccc.numFrom(feeProbe.estimateFee(feeRate));
      transaction = buildAtomicRegistrationTransaction({
        registryCell,
        itemId,
        feeInputs,
        cellDeps,
        fee: estimatedFee,
      });
      break;
    } catch (err) {
      if (
        err instanceof Error &&
        err.message === "Inputs do not cover outputs and the requested fee."
      ) {
        transaction = undefined;
        continue;
      }
      throw err;
    }
  }

  if (!transaction || !feeProbe) {
    throw new Error(
      "The live empty wallet cells do not contain enough capacity to cover the State output and transaction fee.",
    );
  }

  const registryOutput = transaction.outputs[0];
  const stateOutput = transaction.outputs[1];
  if (!registryOutput?.type || !stateOutput?.type) {
    throw new Error("The registration transaction is missing the Registry or State output.");
  }

  const totalInputCapacity = [registryCell, ...feeInputs].reduce(
    (sum, cell) => sum + ccc.numFrom(cell.cellOutput.capacity),
    0n,
  );
  const totalOutputCapacity = transaction.outputs.reduce(
    (sum, output) => sum + ccc.numFrom(output.capacity),
    0n,
  );
  const actualFee = totalInputCapacity - totalOutputCapacity;
  const change = transaction.outputs[2]
    ? ccc.numFrom(transaction.outputs[2].capacity)
    : 0n;

  return {
    transaction,
    ownerAddress,
    itemId,
    registryId: registryData.registryId,
    previousItemCount: registryData.itemCount,
    newItemCount: registryData.itemCount + 1,
    registryCellOutPoint: outPointText(registryCell),
    registryInputCapacity: ccc.numFrom(registryCell.cellOutput.capacity).toString(),
    registryOutputCapacity: ccc.numFrom(registryOutput.capacity).toString(),
    registryTypeScriptHash: registryType.hash(),
    stateTypeScriptHash: stateOutput.type.hash(),
    stateCapacity: ccc.numFrom(stateOutput.capacity).toString(),
    registryCodeOutPoint: outPointText(registryCodeCell),
    stateCodeOutPoint: outPointText(stateCodeCell),
    feeInputs: feeInputs.map((cell) => ({
      outPoint: outPointText(cell),
      capacity: ccc.numFrom(cell.cellOutput.capacity).toString(),
    })),
    estimatedFee: estimatedFee.toString(),
    actualFee: actualFee.toString(),
    feeRate: feeRate.toString(),
    change: change.toString(),
  };
}