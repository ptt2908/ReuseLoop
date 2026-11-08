import { ccc } from "@ckb-ccc/ccc";
import {
  buildBorrowTransaction,
  buildConfirmReturnTransaction,
  buildMakeAvailableTransaction,
} from "./transactionBuilders";
import type { ItemStatus } from "./itemState";
import { lookupItemById } from "./itemLookup";
import type { VerifiedContractBinary } from "./deploymentPreview";

const MAX_OWNER_CELLS = 500;
const FEE_WITNESS_RESERVE_BYTES = 4096;

export interface LifecycleFeeInput {
  outPoint: string;
  capacity: string;
}

export interface LifecyclePreview {
  transaction: ccc.Transaction;
  ownerAddress: string;
  itemId: string;
  registryId: string;
  action: string;
  fromStatus: ItemStatus;
  toStatus: ItemStatus;
  stateCellOutPoint: string;
  stateCapacity: string;
  registryCellOutPoint: string;
  stateCodeOutPoint: string;
  feeInputs: LifecycleFeeInput[];
  estimatedFee: string;
  actualFee: string;
  feeRate: string;
  change: string;
}

function outPointText(cell: ccc.Cell): string {
  return String(cell.outPoint.txHash) + ":" +
    ccc.numFrom(cell.outPoint.index).toString();
}

function outPointFromText(value: string): ccc.OutPoint {
  const separator = value.lastIndexOf(":");
  if (separator <= 0) {
    throw new Error("Cell outpoint is malformed.");
  }
  return ccc.OutPoint.from({
    txHash: value.slice(0, separator),
    index: Number(value.slice(separator + 1)),
  });
}

function hasArtifactData(cell: ccc.Cell, artifact: VerifiedContractBinary): boolean {
  return cell.cellOutput.type != null &&
    String(cell.outputData).toLowerCase() === artifact.data.toLowerCase() &&
    ccc.hashCkb(cell.outputData) === artifact.codeHash;
}

function buildAction(
  fromStatus: ItemStatus,
): {
  action: string;
  toStatus: ItemStatus;
  build: typeof buildBorrowTransaction;
} {
  if (fromStatus === "AVAILABLE") {
    return { action: "Borrow", toStatus: "BORROWED", build: buildBorrowTransaction };
  }
  if (fromStatus === "BORROWED") {
    return { action: "Confirm Return", toStatus: "RETURNED", build: buildConfirmReturnTransaction };
  }
  return { action: "Make Available", toStatus: "AVAILABLE", build: buildMakeAvailableTransaction };
}

function capacitySum(values: Array<bigint | string | number>): bigint {
  return values.reduce<bigint>((total, value) => total + ccc.numFrom(value), 0n);
}

export async function buildLifecyclePreview(
  client: ccc.Client,
  ownerAddress: string,
  itemId: string,
  registryArtifact: VerifiedContractBinary,
  stateArtifact: VerifiedContractBinary,
): Promise<LifecyclePreview> {
  if (registryArtifact.kind !== "registry" || stateArtifact.kind !== "state") {
    throw new Error("Verify the Registry and State binaries before building a lifecycle preview.");
  }

  const item = await lookupItemById(
    client,
    ownerAddress,
    itemId,
    registryArtifact,
    stateArtifact,
  );
  const stateCell = await client.getCellLive(
    outPointFromText(item.stateOutPoint),
    true,
  );
  const registryCell = await client.getCellLive(
    outPointFromText(item.registryOutPoint),
    true,
  );
  if (!stateCell || !registryCell) {
    throw new Error("The live State or Registry cell changed. Refresh the item status before building.");
  }

  const owner = await ccc.Address.fromString(ownerAddress, client);
  const ownerLock = owner.script;
  const joyIdInfo = await client.getKnownScript(ccc.KnownScript.JoyId);
  if (
    ownerLock.codeHash !== joyIdInfo.codeHash ||
    ownerLock.hashType !== joyIdInfo.hashType
  ) {
    throw new Error("Lifecycle transactions currently support a JoyID Testnet Lock Script only.");
  }

  const ownerCells: ccc.Cell[] = [];
  for await (const cell of client.findCellsByLock(
    ownerLock,
    undefined,
    true,
    "desc",
    MAX_OWNER_CELLS,
  )) {
    ownerCells.push(cell);
  }

  const codeMatches = ownerCells.filter((cell) => hasArtifactData(cell, stateArtifact));
  const liveCodeCells: ccc.Cell[] = [];
  for (const candidate of codeMatches) {
    const liveCell = await client.getCellLive(candidate.outPoint, true);
    if (liveCell && hasArtifactData(liveCell, stateArtifact)) {
      liveCodeCells.push(liveCell);
    }
  }
  if (liveCodeCells.length === 0) {
    throw new Error("No live State code cell matching the verified binary was found for this wallet.");
  }
  if (liveCodeCells.length > 1) {
    throw new Error("Multiple live State code cells match this binary. Resolve the duplicate before building.");
  }

  const stateCodeCell = liveCodeCells[0];
  const stateCodeDep = ccc.CellDep.from({
    outPoint: stateCodeCell.outPoint,
    depType: "code",
  });
  if (
    outPointText(stateCodeCell).toLowerCase() === item.stateOutPoint.toLowerCase() ||
    outPointText(stateCodeCell).toLowerCase() === item.registryOutPoint.toLowerCase()
  ) {
    throw new Error("A live state cell cannot also be used as the State code dependency.");
  }

  const feeCandidates = ownerCells
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
    item.stateOutPoint.toLowerCase(),
    item.registryOutPoint.toLowerCase(),
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
    throw new Error("No live empty wallet cells are available to pay the lifecycle transaction fee.");
  }

  const joyIdCellDeps = await client.getCellDeps(joyIdInfo.cellDeps);
  const cellDeps = [
    ...joyIdCellDeps,
    stateCodeDep,
  ];
  const feeRate = ccc.numFrom(await client.getFeeRate());
  const action = buildAction(item.status);
  let feeInputs: ccc.Cell[] = [];
  let estimatedFee = 0n;
  let transaction: ccc.Transaction | undefined;

  for (const feeCell of liveFeeCells) {
    feeInputs = [...feeInputs, feeCell];
    try {
      const feeProbe = action.build({
        stateCell,
        registryCell,
        feeInputs,
        cellDeps,
        fee: 0n,
        changeLock: ownerLock,
      });
      feeProbe.setWitness(
        0,
        ccc.WitnessArgs.from({
          lock: "0x" + "00".repeat(FEE_WITNESS_RESERVE_BYTES),
        }).toBytes(),
      );
      estimatedFee = ccc.numFrom(feeProbe.estimateFee(feeRate));
      transaction = action.build({
        stateCell,
        registryCell,
        feeInputs,
        cellDeps,
        fee: estimatedFee,
        changeLock: ownerLock,
      });
      break;
    } catch (err) {
      if (err instanceof Error && err.message === "Inputs do not cover outputs and the requested fee.") {
        transaction = undefined;
        continue;
      }
      throw err;
    }
  }

  if (!transaction) {
    throw new Error("The available wallet cells do not cover the estimated lifecycle fee and change.");
  }

  if (
    !transaction.outputs[0]?.type ||
    transaction.outputsData[0] == null
  ) {
    throw new Error("The lifecycle transaction is missing its State output.");
  }

  const totalInputCapacity = capacitySum([
    stateCell.cellOutput.capacity,
    ...feeInputs.map((cell) => cell.cellOutput.capacity),
  ]);
  const totalOutputCapacity = capacitySum(
    transaction.outputs.map((output) => output.capacity),
  );
  if (totalInputCapacity < totalOutputCapacity) {
    throw new Error("Lifecycle transaction output capacity exceeds its inputs.");
  }

  const change = transaction.outputs.reduce((total, output) => {
    if (
      output.type == null &&
      output.lock.hash() === ownerLock.hash()
    ) {
      return total + ccc.numFrom(output.capacity);
    }
    return total;
  }, 0n);

  return {
    transaction,
    ownerAddress,
    itemId: item.itemId,
    registryId: item.registryId,
    action: action.action,
    fromStatus: item.status,
    toStatus: action.toStatus,
    stateCellOutPoint: item.stateOutPoint,
    stateCapacity: item.stateCapacity,
    registryCellOutPoint: item.registryOutPoint,
    stateCodeOutPoint: outPointText(stateCodeCell),
    feeInputs: feeInputs.map((cell) => ({
      outPoint: outPointText(cell),
      capacity: String(cell.cellOutput.capacity),
    })),
    estimatedFee: estimatedFee.toString(),
    actualFee: (totalInputCapacity - totalOutputCapacity).toString(),
    feeRate: feeRate.toString(),
    change: change.toString(),
  };
}