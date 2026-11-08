import { ccc } from "@ckb-ccc/ccc";
import { decodeRegistryScriptArgs } from "./registryCodec";
import {
  buildRegistryGenesisTransaction,
} from "./transactionBuilders";
import type { VerifiedContractBinary } from "./deploymentPreview";

const MAX_WALLET_CELLS = 100;
const FEE_WITNESS_RESERVE_BYTES = 4096;
const DATA1_HASH_TYPE = "data1";
const DATA1_HASH_TYPE_CODE = 0x02;

export interface RegistryGenesisPreview {
  transaction: ccc.Transaction;
  ownerAddress: string;
  registryId: string;
  registryTypeScriptHash: string;
  registryCapacity: string;
  seedOutPoint: string;
  seedCapacity: string;
  registryCodeOutPoint: string;
  stateCodeOutPoint: string;
  fee: string;
  feeRate: string;
  change: string;
}

function outPointText(cell: ccc.Cell): string {
  return String(cell.outPoint.txHash) + ":" +
    ccc.numFrom(cell.outPoint.index).toString();
}

function findArtifactCell(
  cells: ccc.Cell[],
  artifact: VerifiedContractBinary,
): ccc.Cell {
  const matches = cells.filter((cell) =>
    cell.cellOutput.type != null &&
    ccc.hashCkb(cell.outputData) === artifact.codeHash &&
    String(cell.outputData).toLowerCase() === artifact.data.toLowerCase()
  );

  if (matches.length === 0) {
    throw new Error(
      "No live " + artifact.kind + " code cell matching the verified binary was found for this wallet. " +
      "Inspect cells and confirm the deployed code cell is still live.",
    );
  }

  return matches[0];
}

export async function buildRegistryGenesisPreview(
  client: ccc.Client,
  ownerAddress: string,
  registryArtifact: VerifiedContractBinary,
  stateArtifact: VerifiedContractBinary,
): Promise<RegistryGenesisPreview> {
  if (registryArtifact.kind !== "registry" || stateArtifact.kind !== "state") {
    throw new Error("Select verified Registry and State binaries in their matching fields.");
  }
  if (registryArtifact.codeHash === stateArtifact.codeHash) {
    throw new Error("Registry and State binaries must have different code hashes.");
  }

  const address = await ccc.Address.fromString(ownerAddress, client);
  const adminLock = address.script;
  const joyIdInfo = await client.getKnownScript(ccc.KnownScript.JoyId);
  if (
    adminLock.codeHash !== joyIdInfo.codeHash ||
    adminLock.hashType !== joyIdInfo.hashType
  ) {
    throw new Error("Registry Genesis preview currently supports a JoyID Testnet Lock Script only.");
  }

  const joyIdCellDeps = await client.getCellDeps(joyIdInfo.cellDeps);
  const liveCells: ccc.Cell[] = [];
  for await (const cell of client.findCellsByLock(
    adminLock,
    undefined,
    true,
    "desc",
    MAX_WALLET_CELLS,
  )) {
    liveCells.push(cell);
  }

  const registryCodeCell = findArtifactCell(liveCells, registryArtifact);
  const stateCodeCell = findArtifactCell(liveCells, stateArtifact);
  if (outPointText(registryCodeCell) === outPointText(stateCodeCell)) {
    throw new Error("Registry and State code binaries must be in separate live cells.");
  }

  const seedCandidates = liveCells
    .filter((cell) =>
      cell.cellOutput.type == null &&
      String(cell.outputData).toLowerCase() === "0x"
    )
    .sort((left, right) => {
      const leftCapacity = ccc.numFrom(left.cellOutput.capacity);
      const rightCapacity = ccc.numFrom(right.cellOutput.capacity);
      return leftCapacity > rightCapacity ? -1 : leftCapacity < rightCapacity ? 1 : 0;
    });
  const seedCell = seedCandidates[0];
  if (!seedCell) {
    throw new Error(
      "No live empty, untyped seed cell was found for the connected wallet. " +
      "Inspect the wallet cells and make sure a CKB capacity cell is available.",
    );
  }

  const registryCodeDep = ccc.CellDep.from({
    outPoint: registryCodeCell.outPoint,
    depType: "code",
  });
  const cellDeps = [...joyIdCellDeps, registryCodeDep];
  const sharedOptions = {
    seedCell,
    registryCodeHash: registryArtifact.codeHash,
    registryHashType: DATA1_HASH_TYPE,
    stateCodeHash: stateArtifact.codeHash,
    stateHashType: DATA1_HASH_TYPE_CODE,
    adminLock,
    cellDeps,
  } as const;

  const feeRate = ccc.numFrom(await client.getFeeRate());
  const feeProbe = buildRegistryGenesisTransaction({ ...sharedOptions, fee: 0n });
  feeProbe.setWitness(
    0,
    ccc.WitnessArgs.from({
      lock: "0x" + "00".repeat(FEE_WITNESS_RESERVE_BYTES),
    }).toBytes(),
  );
  const fee = ccc.numFrom(feeProbe.estimateFee(feeRate));
  const transaction = buildRegistryGenesisTransaction({ ...sharedOptions, fee });
  const registryOutput = transaction.outputs[0];
  if (!registryOutput?.type) {
    throw new Error("Registry Genesis transaction is missing its Registry Type Script output.");
  }

  const registryArgs = decodeRegistryScriptArgs(registryOutput.type.args);
  const change = transaction.outputs.length > 1
    ? ccc.numFrom(transaction.outputs[1].capacity)
    : 0n;

  return {
    transaction,
    ownerAddress,
    registryId: registryArgs.registryId,
    registryTypeScriptHash: registryOutput.type.hash(),
    registryCapacity: ccc.numFrom(registryOutput.capacity).toString(),
    seedOutPoint: outPointText(seedCell),
    seedCapacity: ccc.numFrom(seedCell.cellOutput.capacity).toString(),
    registryCodeOutPoint: outPointText(registryCodeCell),
    stateCodeOutPoint: outPointText(stateCodeCell),
    fee: fee.toString(),
    feeRate: feeRate.toString(),
    change: change.toString(),
  };
}
