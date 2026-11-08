import { ccc } from "@ckb-ccc/ccc";
import {
  decodeRegistryData,
  decodeRegistryScriptArgs,
  encodeRegistryData,
  encodeRegistryScriptArgs,
  type StateHashType,
} from "./registryCodec";
import { decodeStateScriptArgs, encodeStateScriptArgs } from "./stateCodec";
import { decodeStateCellData, encodeStateCellData } from "./stateDataCodec";
import type { ItemStatus } from "./itemState";
import { deriveRegistryId } from "./typeId";

type CellLike = Parameters<typeof ccc.Cell.from>[0];
type ScriptLike = Parameters<typeof ccc.Script.from>[0];
type CellDepLike = Parameters<typeof ccc.CellDep.from>[0];
type CapacityLike = Parameters<typeof ccc.numFrom>[0];
type HashTypeLike = ScriptLike["hashType"];

interface OutputEntry {
  output: ccc.CellOutput;
  data: string;
}

interface BalancedOutputs {
  outputs: ccc.CellOutput[];
  outputsData: string[];
}

export interface RegistryGenesisOptions {
  seedCell: CellLike;
  registryCodeHash: string;
  registryHashType: HashTypeLike;
  stateCodeHash: string;
  stateHashType: StateHashType;
  adminLock: ScriptLike;
  cellDeps?: CellDepLike[];
  feeInputs?: CellLike[];
  registryCapacity?: CapacityLike;
  fee?: CapacityLike;
  changeLock?: ScriptLike;
}

export interface AtomicRegistrationOptions {
  registryCell: CellLike;
  itemId: string;
  cellDeps?: CellDepLike[];
  feeInputs?: CellLike[];
  registryCapacity?: CapacityLike;
  stateCapacity?: CapacityLike;
  fee?: CapacityLike;
  changeLock?: ScriptLike;
}

export interface LifecycleTransactionOptions {
  stateCell: CellLike;
  registryCell: CellLike;
  cellDeps?: CellDepLike[];
  feeInputs?: CellLike[];
  stateCapacity?: CapacityLike;
  fee?: CapacityLike;
  changeLock?: ScriptLike;
}

function amount(value: CapacityLike | undefined, field: string): bigint {
  const result = ccc.numFrom(value ?? 0n);
  if (result < 0n) {
    throw new Error(field + " must not be negative.");
  }
  return result;
}

function max(left: bigint, right: bigint): bigint {
  return left > right ? left : right;
}

function resolveCell(cellLike: CellLike): ccc.Cell {
  return ccc.Cell.from(cellLike);
}

function resolveCells(cellLikes: CellLike[] | undefined): ccc.Cell[] {
  return (cellLikes ?? []).map(resolveCell);
}

function outPointKey(cell: ccc.Cell): string {
  return String(cell.outPoint.txHash).toLowerCase() + ":" +
    ccc.numFrom(cell.outPoint.index).toString();
}

function depKey(dep: ccc.CellDep): string {
  return String(dep.outPoint.txHash).toLowerCase() + ":" +
    ccc.numFrom(dep.outPoint.index).toString();
}

function assertUniqueCells(cells: ccc.Cell[], field: string): void {
  const seen = new Set<string>();
  for (const cell of cells) {
    const key = outPointKey(cell);
    if (seen.has(key)) {
      throw new Error(field + " contains the same cell more than once.");
    }
    seen.add(key);
  }
}

function resolveDeps(deps: CellDepLike[] | undefined): ccc.CellDep[] {
  const resolved = (deps ?? []).map((dep) => ccc.CellDep.from(dep));
  const seen = new Set<string>();
  for (const dep of resolved) {
    const key = depKey(dep);
    if (seen.has(key)) {
      throw new Error("Cell deps must not contain duplicate outpoints.");
    }
    seen.add(key);
  }
  return resolved;
}

function assertNoCellDepOverlap(inputs: ccc.Cell[], deps: ccc.CellDep[]): void {
  const inputKeys = new Set(inputs.map(outPointKey));
  if (deps.some((dep) => inputKeys.has(depKey(dep)))) {
    throw new Error("A transaction input cannot also be a cell dep.");
  }
}

function inputFromCell(cell: ccc.Cell): ccc.CellInput {
  return ccc.CellInput.from({
    previousOutput: cell.outPoint,
    since: 0n,
    cellOutput: cell.cellOutput,
    outputData: cell.outputData,
  });
}

function scriptType(cell: ccc.Cell, field: string): ccc.Script {
  if (!cell.cellOutput.type) {
    throw new Error(field + " must have a Type Script.");
  }
  return cell.cellOutput.type;
}

function hasCodeIdentity(
  scriptLike: ScriptLike | null | undefined,
  codeHash: string,
  hashType: HashTypeLike,
): boolean {
  if (!scriptLike) {
    return false;
  }
  const script = ccc.Script.from(scriptLike);
  const expected = ccc.Script.from({ codeHash, hashType, args: "0x" });
  return script.codeHash === expected.codeHash &&
    script.hashType === expected.hashType;
}

function makeCellOutput(
  lock: ccc.Script,
  type: ccc.Script | undefined,
  data: string,
  requestedCapacity: CapacityLike | undefined,
  minimumCapacity: bigint,
  field: string,
): ccc.CellOutput {
  const occupiedCapacity = ccc.numFrom(
    ccc.CellOutput.from({ capacity: 0n, lock, type }, data).capacity,
  );
  const requiredCapacity = max(occupiedCapacity, minimumCapacity);
  const capacity = requestedCapacity === undefined
    ? requiredCapacity
    : amount(requestedCapacity, field);
  if (capacity < requiredCapacity) {
    throw new Error(field + " must be at least the occupied and preserved capacity.");
  }
  return ccc.CellOutput.from({ capacity, lock, type }, data);
}

function balanceOutputs(
  inputs: ccc.Cell[],
  primaryOutputs: OutputEntry[],
  feeValue: CapacityLike | undefined,
  changeLockLike: ScriptLike | undefined,
  defaultChangeLock: ccc.Script,
): BalancedOutputs {
  const fee = amount(feeValue, "Transaction fee");
  const inputCapacity = inputs.reduce(
    (sum, cell) => sum + ccc.numFrom(cell.cellOutput.capacity),
    0n,
  );
  const outputCapacity = primaryOutputs.reduce(
    (sum, entry) => sum + ccc.numFrom(entry.output.capacity),
    0n,
  );
  const remaining = inputCapacity - outputCapacity - fee;
  if (remaining < 0n) {
    throw new Error("Inputs do not cover outputs and the requested fee.");
  }

  const outputs = primaryOutputs.map((entry) => entry.output);
  const outputsData = primaryOutputs.map((entry) => entry.data);
  if (remaining > 0n) {
    const changeLock = changeLockLike
      ? ccc.Script.from(changeLockLike)
      : defaultChangeLock;
    const minimumChange = ccc.numFrom(
      ccc.CellOutput.from({ capacity: 0n, lock: changeLock }, "0x").capacity,
    );
    // Dust below occupied capacity becomes additional transaction fee.
    if (remaining >= minimumChange) {
      outputs.push(ccc.CellOutput.from({
        capacity: remaining,
        lock: changeLock,
      }, "0x"));
      outputsData.push("0x");
    }
  }
  return { outputs, outputsData };
}

function createTransaction(
  inputs: ccc.Cell[],
  cellDeps: ccc.CellDep[],
  balanced: BalancedOutputs,
): ccc.Transaction {
  return ccc.Transaction.from({
    cellDeps,
    inputs: inputs.map(inputFromCell),
    outputs: balanced.outputs,
    outputsData: balanced.outputsData,
    witnesses: inputs.map(() => "0x"),
  });
}

function assertNoCanonicalStateCells(
  cells: ccc.Cell[],
  stateCodeHash: string,
  stateHashType: HashTypeLike,
): void {
  if (cells.some((cell) =>
    hasCodeIdentity(cell.cellOutput.type, stateCodeHash, stateHashType)
  )) {
    throw new Error("Transaction must not contain another canonical State cell.");
  }
}

function assertRegistryCellData(
  registryType: ccc.Script,
  registryDataHex: string,
) {
  const registryArgs = decodeRegistryScriptArgs(registryType.args);
  const registryData = decodeRegistryData(registryDataHex);
  if (registryData.registryId !== registryArgs.registryId) {
    throw new Error("Registry cell data ID must match its Type Script args.");
  }
  return { registryArgs, registryData };
}

function assertLockHash(lock: ccc.Script, expectedHash: string, field: string): void {
  if (lock.hash() !== expectedHash.toLowerCase()) {
    throw new Error(field + " Lock Script hash does not match its contract args.");
  }
}

export function buildRegistryGenesisTransaction(
  options: RegistryGenesisOptions,
): ccc.Transaction {
  const seedCell = resolveCell(options.seedCell);
  const feeInputs = resolveCells(options.feeInputs);
  const inputs = [seedCell, ...feeInputs];
  assertUniqueCells(inputs, "Genesis inputs");

  const adminLock = ccc.Script.from(options.adminLock);
  const adminLockHash = adminLock.hash();
  assertLockHash(seedCell.cellOutput.lock, adminLockHash, "Genesis seed input");

  const registryCode = ccc.Script.from({
    codeHash: options.registryCodeHash,
    hashType: options.registryHashType,
    args: "0x",
  });
  const stateCode = ccc.Script.from({
    codeHash: options.stateCodeHash,
    hashType: options.stateHashType,
    args: "0x",
  });
  if (
    registryCode.codeHash === stateCode.codeHash &&
    registryCode.hashType === stateCode.hashType
  ) {
    throw new Error("Registry and State scripts must use different code identities.");
  }
  assertNoCanonicalStateCells(inputs, options.stateCodeHash, options.stateHashType);

  const registryId = deriveRegistryId({
    previousOutput: {
      txHash: String(seedCell.outPoint.txHash),
      index: ccc.numFrom(seedCell.outPoint.index).toString(),
    },
    since: 0n,
  }, 0n);
  const registryType = ccc.Script.from({
    codeHash: registryCode.codeHash,
    hashType: registryCode.hashType,
    args: encodeRegistryScriptArgs({
      registryId,
      adminLockHash,
      stateCodeHash: options.stateCodeHash,
      stateHashType: options.stateHashType,
    }),
  });
  if (inputs.some((cell) => cell.cellOutput.type?.hash() === registryType.hash())) {
    throw new Error("Registry Genesis must not consume an existing Registry cell.");
  }

  const registryData = encodeRegistryData({ registryId, itemIds: [] });
  const registryOutput = makeCellOutput(
    adminLock,
    registryType,
    registryData,
    options.registryCapacity,
    0n,
    "Registry capacity",
  );
  const cellDeps = resolveDeps(options.cellDeps);
  assertNoCellDepOverlap(inputs, cellDeps);
  const balanced = balanceOutputs(
    inputs,
    [{ output: registryOutput, data: registryData }],
    options.fee,
    options.changeLock,
    adminLock,
  );
  return createTransaction(inputs, cellDeps, balanced);
}

export function buildAtomicRegistrationTransaction(
  options: AtomicRegistrationOptions,
): ccc.Transaction {
  const registryCell = resolveCell(options.registryCell);
  const feeInputs = resolveCells(options.feeInputs);
  const inputs = [registryCell, ...feeInputs];
  assertUniqueCells(inputs, "Registration inputs");

  const registryType = scriptType(registryCell, "Registry cell");
  const { registryArgs, registryData } = assertRegistryCellData(
    registryType,
    registryCell.outputData,
  );
  assertLockHash(registryCell.cellOutput.lock, registryArgs.adminLockHash, "Registry");
  assertNoCanonicalStateCells(
    feeInputs,
    registryArgs.stateCodeHash,
    registryArgs.stateHashType,
  );
  if (feeInputs.some((cell) => cell.cellOutput.type?.hash() === registryType.hash())) {
    throw new Error("Atomic registration must consume exactly one Registry cell.");
  }

  const cellDeps = resolveDeps(options.cellDeps);
  assertNoCellDepOverlap(inputs, cellDeps);
  const nextRegistryData = decodeRegistryData(encodeRegistryData({
    registryId: registryArgs.registryId,
    itemIds: [...registryData.itemIds, options.itemId],
  }));
  const itemId = nextRegistryData.itemIds[nextRegistryData.itemIds.length - 1];
  const registryDataHex = encodeRegistryData({
    registryId: registryArgs.registryId,
    itemIds: nextRegistryData.itemIds,
  });
  const registryOutput = makeCellOutput(
    registryCell.cellOutput.lock,
    registryType,
    registryDataHex,
    options.registryCapacity,
    ccc.numFrom(registryCell.cellOutput.capacity),
    "Registry output capacity",
  );

  const stateType = ccc.Script.from({
    codeHash: registryArgs.stateCodeHash,
    hashType: registryArgs.stateHashType,
    args: encodeStateScriptArgs({
      itemId,
      trustedRegistryTypeHash: registryType.hash(),
      adminLockHash: registryArgs.adminLockHash,
    }),
  });
  const stateData = encodeStateCellData({ itemId, status: "AVAILABLE" });
  const stateOutput = makeCellOutput(
    registryCell.cellOutput.lock,
    stateType,
    stateData,
    options.stateCapacity,
    0n,
    "Initial State capacity",
  );
  const balanced = balanceOutputs(
    inputs,
    [
      { output: registryOutput, data: registryDataHex },
      { output: stateOutput, data: stateData },
    ],
    options.fee,
    options.changeLock,
    registryCell.cellOutput.lock,
  );
  return createTransaction(inputs, cellDeps, balanced);
}

function buildLifecycleTransaction(
  options: LifecycleTransactionOptions,
  fromStatus: ItemStatus,
  toStatus: ItemStatus,
  actionName: string,
): ccc.Transaction {
  const stateCell = resolveCell(options.stateCell);
  const registryCell = resolveCell(options.registryCell);
  const feeInputs = resolveCells(options.feeInputs);
  const inputs = [stateCell, ...feeInputs];
  assertUniqueCells(inputs, "Lifecycle inputs");

  const stateType = scriptType(stateCell, "State input");
  const stateArgs = decodeStateScriptArgs(stateType.args);
  const stateData = decodeStateCellData(stateCell.outputData);
  if (stateData.itemId !== stateArgs.itemId) {
    throw new Error("State cell data item ID must match its Type Script args.");
  }
  if (stateData.status !== fromStatus) {
    throw new Error(actionName + " requires a " + fromStatus + " State input.");
  }

  const registryType = scriptType(registryCell, "Trusted Registry cell dep");
  const { registryArgs, registryData } = assertRegistryCellData(
    registryType,
    registryCell.outputData,
  );
  if (registryType.hash() !== stateArgs.trustedRegistryTypeHash) {
    throw new Error("Registry cell dep does not match the trusted Type Script hash.");
  }
  assertLockHash(registryCell.cellOutput.lock, stateArgs.adminLockHash, "Registry");
  assertLockHash(stateCell.cellOutput.lock, stateArgs.adminLockHash, "State");
  if (registryArgs.adminLockHash !== stateArgs.adminLockHash) {
    throw new Error("State and Registry administrator Lock hashes must match.");
  }
  if (!hasCodeIdentity(stateType, registryArgs.stateCodeHash, registryArgs.stateHashType)) {
    throw new Error("State Type Script identity does not match the Registry args.");
  }
  if (!registryData.itemIds.includes(stateArgs.itemId)) {
    throw new Error("Trusted Registry does not contain this State item ID.");
  }
  if (inputs.some((cell) =>
    cell.cellOutput.type?.hash() === stateArgs.trustedRegistryTypeHash
  )) {
    throw new Error("Trusted Registry cell must not be a transaction input.");
  }
  assertNoCanonicalStateCells(
    feeInputs,
    registryArgs.stateCodeHash,
    registryArgs.stateHashType,
  );

  const additionalDeps = resolveDeps(options.cellDeps);
  const registryDep = ccc.CellDep.from({
    outPoint: registryCell.outPoint,
    depType: "code",
  });
  if (additionalDeps.some((dep) => depKey(dep) === depKey(registryDep))) {
    throw new Error("Trusted Registry cell dep must be included exactly once.");
  }
  assertNoCellDepOverlap(inputs, additionalDeps);

  const nextData = encodeStateCellData({
    itemId: stateArgs.itemId,
    status: toStatus,
  });
  const stateOutput = makeCellOutput(
    stateCell.cellOutput.lock,
    stateType,
    nextData,
    options.stateCapacity,
    ccc.numFrom(stateCell.cellOutput.capacity),
    "Successor State capacity",
  );
  const balanced = balanceOutputs(
    inputs,
    [{ output: stateOutput, data: nextData }],
    options.fee,
    options.changeLock,
    stateCell.cellOutput.lock,
  );
  return createTransaction(inputs, [...additionalDeps, registryDep], balanced);
}

export function buildBorrowTransaction(
  options: LifecycleTransactionOptions,
): ccc.Transaction {
  return buildLifecycleTransaction(options, "AVAILABLE", "BORROWED", "Borrow");
}

export function buildConfirmReturnTransaction(
  options: LifecycleTransactionOptions,
): ccc.Transaction {
  return buildLifecycleTransaction(options, "BORROWED", "RETURNED", "ConfirmReturn");
}

export function buildMakeAvailableTransaction(
  options: LifecycleTransactionOptions,
): ccc.Transaction {
  return buildLifecycleTransaction(options, "RETURNED", "AVAILABLE", "MakeAvailable");
}
