import { ccc } from "@ckb-ccc/ccc";

export type ContractBinaryKind = "registry" | "state";

const EXPECTED_BINARY_SHA256: Record<ContractBinaryKind, string> = {
  registry: "C45C0EDE3B95BF4A447FEC1AC18E7756175755D7F832AB964503E0EE19BC5A88",
  state: "376A80EE27C79ACC762AB1D828F17AA15499F41B05C2C07067F4429032418A03",
};

export interface VerifiedContractBinary {
  kind: ContractBinaryKind;
  fileName: string;
  byteLength: number;
  sha256: string;
  codeHash: string;
  data: string;
}

export interface DeploymentOutputPreview {
  kind: ContractBinaryKind;
  outputIndex: number;
  typeId: string;
  typeScriptHash: string;
  capacity: string;
  codeHash: string;
  byteLength: number;
  sha256: string;
  fileName: string;
}

export interface DeploymentInputPreview {
  outPoint: string;
  capacity: string;
}

export interface DeploymentPreview {
  transaction: ccc.Transaction;
  ownerAddress: string;
  registry: DeploymentOutputPreview;
  state: DeploymentOutputPreview;
  inputs: DeploymentInputPreview[];
  typeIdCodeHash: string;
  fee: string;
  feeRate: string;
  change: string;
}

const FEE_WITNESS_RESERVE_BYTES = 4096;
const MAX_FUNDING_CELLS = 100;

function toHex(bytes: Uint8Array): string {
  let result = "0x";
  for (const byte of bytes) {
    result += byte.toString(16).padStart(2, "0");
  }
  return result;
}

function formatOutPoint(cell: ccc.Cell): string {
  return String(cell.outPoint.txHash) + ":" +
    ccc.numFrom(cell.outPoint.index).toString();
}

export async function verifyContractBinary(
  file: File,
  kind: ContractBinaryKind,
): Promise<VerifiedContractBinary> {
  if (file.size === 0) {
    throw new Error("The selected " + kind + " binary is empty.");
  }

  const bytes = new Uint8Array(await file.arrayBuffer());
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  const sha256 = Array.from(new Uint8Array(digest))
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("")
    .toUpperCase();
  const expected = EXPECTED_BINARY_SHA256[kind];

  if (sha256 !== expected) {
    throw new Error(
      "The selected " + kind + " binary does not match the verified CP11 artifact. " +
      "Expected SHA-256 " + expected + " but received " + sha256 + ".",
    );
  }

  const data = toHex(bytes);
  return {
    kind,
    fileName: file.name,
    byteLength: bytes.length,
    sha256,
    codeHash: ccc.hashCkb(data),
    data,
  };
}

function makeInput(cell: ccc.Cell): ccc.CellInput {
  return ccc.CellInput.from({
    previousOutput: cell.outPoint,
    since: 0n,
  });
}

function makeTransaction(
  inputs: ccc.Cell[],
  cellDeps: ccc.CellDep[],
  registryOutput: ccc.CellOutput,
  registryData: string,
  stateOutput: ccc.CellOutput,
  stateData: string,
  changeLock: ccc.Script,
  changeCapacity: bigint,
): ccc.Transaction {
  return ccc.Transaction.from({
    cellDeps,
    inputs: inputs.map(makeInput),
    outputs: [
      registryOutput,
      stateOutput,
      ccc.CellOutput.from(
        { capacity: changeCapacity, lock: changeLock },
        "0x",
      ),
    ],
    outputsData: [registryData, stateData, "0x"],
    witnesses: inputs.map(() => "0x"),
  });
}

function makeOutputPreview(
  kind: ContractBinaryKind,
  outputIndex: number,
  typeId: string,
  output: ccc.CellOutput,
  artifact: VerifiedContractBinary,
): DeploymentOutputPreview {
  if (!output.type) {
    throw new Error("A deployment output is missing its Type ID script.");
  }

  return {
    kind,
    outputIndex,
    typeId,
    typeScriptHash: output.type.hash(),
    capacity: ccc.numFrom(output.capacity).toString(),
    codeHash: artifact.codeHash,
    byteLength: artifact.byteLength,
    sha256: artifact.sha256,
    fileName: artifact.fileName,
  };
}

export async function buildDeploymentPreview(
  client: ccc.Client,
  ownerAddress: string,
  registry: VerifiedContractBinary,
  state: VerifiedContractBinary,
): Promise<DeploymentPreview> {
  if (registry.kind !== "registry" || state.kind !== "state") {
    throw new Error("Select the verified Registry and State binaries in their matching fields.");
  }

  const address = await ccc.Address.fromString(ownerAddress, client);
  const ownerLock = address.script;
  const typeIdInfo = await client.getKnownScript(ccc.KnownScript.TypeId);
  const typeIdCellDeps = await client.getCellDeps(typeIdInfo.cellDeps);
  const joyIdInfo = await client.getKnownScript(ccc.KnownScript.JoyId);
  if (
    ownerLock.codeHash !== joyIdInfo.codeHash ||
    ownerLock.hashType !== joyIdInfo.hashType
  ) {
    throw new Error("This preview currently supports a connected JoyID Testnet Lock Script only.");
  }
  const joyIdCellDeps = await client.getCellDeps(joyIdInfo.cellDeps);
  const transactionCellDeps = [...typeIdCellDeps, ...joyIdCellDeps];
  const candidates: ccc.Cell[] = [];

  for await (const cell of client.findCellsByLock(
    ownerLock,
    undefined,
    true,
    "desc",
    MAX_FUNDING_CELLS,
  )) {
    if (!cell.cellOutput.type && String(cell.outputData).toLowerCase() === "0x") {
      candidates.push(cell);
    }
  }

  if (candidates.length === 0) {
    throw new Error(
      "No empty, untyped CKB cells were found for this wallet. " +
      "The preview only uses those cells so it does not select token or contract cells.",
    );
  }

  const firstInput = makeInput(candidates[0]);
  const registryTypeId = ccc.hashTypeId(firstInput, 0n);
  const stateTypeId = ccc.hashTypeId(firstInput, 1n);
  const registryType = ccc.Script.from({
    codeHash: typeIdInfo.codeHash,
    hashType: typeIdInfo.hashType,
    args: registryTypeId,
  });
  const stateType = ccc.Script.from({
    codeHash: typeIdInfo.codeHash,
    hashType: typeIdInfo.hashType,
    args: stateTypeId,
  });
  const registryOutput = ccc.CellOutput.from(
    { lock: ownerLock, type: registryType },
    registry.data,
  );
  const stateOutput = ccc.CellOutput.from(
    { lock: ownerLock, type: stateType },
    state.data,
  );
  const registryCapacity = ccc.numFrom(registryOutput.capacity);
  const stateCapacity = ccc.numFrom(stateOutput.capacity);
  const deploymentCapacity = registryCapacity + stateCapacity;
  const changeMinimum = ccc.numFrom(
    ccc.CellOutput.from({ lock: ownerLock }, "0x").capacity,
  );
  const feeRate = ccc.numFrom(await client.getFeeRate());
  let selected: ccc.Cell[] = [];
  let selectedCapacity = 0n;
  let finalTransaction: ccc.Transaction | undefined;
  let estimatedFee = 0n;
  let change = 0n;

  for (const cell of candidates) {
    selected = [...selected, cell];
    selectedCapacity += ccc.numFrom(cell.cellOutput.capacity);

    const probe = makeTransaction(
      selected,
      transactionCellDeps,
      registryOutput,
      registry.data,
      stateOutput,
      state.data,
      ownerLock,
      changeMinimum,
    );
    probe.setWitness(
      0,
      ccc.WitnessArgs.from({
        lock: "0x" + "00".repeat(FEE_WITNESS_RESERVE_BYTES),
      }).toBytes(),
    );
    estimatedFee = ccc.numFrom(probe.estimateFee(feeRate));
    change = selectedCapacity - deploymentCapacity - estimatedFee;

    if (change >= changeMinimum) {
      finalTransaction = makeTransaction(
        selected,
        transactionCellDeps,
        registryOutput,
        registry.data,
        stateOutput,
        state.data,
        ownerLock,
        change,
      );
      break;
    }
  }

  if (!finalTransaction) {
    const requiredCapacity = deploymentCapacity + estimatedFee + changeMinimum;
    throw new Error(
      "The empty CKB cells found for this wallet do not cover this deployment. " +
      "Available: " + selectedCapacity.toString() + " shannons; required: at least " +
      requiredCapacity.toString() + " shannons, including change and the fee estimate.",
    );
  }

  const registryPreview = makeOutputPreview(
    "registry",
    0,
    registryTypeId,
    registryOutput,
    registry,
  );
  const statePreview = makeOutputPreview(
    "state",
    1,
    stateTypeId,
    stateOutput,
    state,
  );

  return {
    transaction: finalTransaction,
    ownerAddress,
    registry: registryPreview,
    state: statePreview,
    inputs: selected.map((cell) => ({
      outPoint: formatOutPoint(cell),
      capacity: ccc.numFrom(cell.cellOutput.capacity).toString(),
    })),
    typeIdCodeHash: typeIdInfo.codeHash,
    fee: estimatedFee.toString(),
    feeRate: feeRate.toString(),
    change: change.toString(),
  };
}