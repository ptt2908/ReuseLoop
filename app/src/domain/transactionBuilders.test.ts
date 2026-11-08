import { ccc } from "@ckb-ccc/ccc";
import { describe, expect, it } from "vitest";
import {
  decodeRegistryData,
  decodeRegistryScriptArgs,
  encodeRegistryData,
  encodeRegistryScriptArgs,
} from "./registryCodec";
import {
  buildAtomicRegistrationTransaction,
  buildBorrowTransaction,
  buildConfirmReturnTransaction,
  buildMakeAvailableTransaction,
  buildRegistryGenesisTransaction,
} from "./transactionBuilders";
import { decodeStateScriptArgs, encodeStateScriptArgs } from "./stateCodec";
import { decodeStateCellData, encodeStateCellData } from "./stateDataCodec";
import { deriveRegistryId } from "./typeId";

const ADMIN_LOCK = ccc.Script.from({
  codeHash: "0x" + "11".repeat(32),
  hashType: "type",
  args: "0x" + "22".repeat(20),
});
const REGISTRY_CODE_HASH = "0x" + "33".repeat(32);
const STATE_CODE_HASH = "0x" + "44".repeat(32);
const ITEM_ID = "0x" + "a1".repeat(32);
const OTHER_ITEM_ID = "0x" + "b2".repeat(32);
const REGISTRY_ID = "0x" + "55".repeat(32);
const CODE_DEP = {
  outPoint: { txHash: "0x" + "66".repeat(32), index: 0 },
  depType: "code",
} as const;
const CAPACITY = 100_000_000_000n;

function makeCell(
  txHashByte: string,
  index: number,
  capacity: bigint,
  lock: Parameters<typeof ccc.Script.from>[0],
  type: Parameters<typeof ccc.Script.from>[0] | undefined,
  outputData: string,
) {
  return ccc.Cell.from({
    outPoint: { txHash: "0x" + txHashByte.repeat(32), index },
    cellOutput: { capacity, lock, type },
    outputData,
  });
}

function canonicalRegistryCell(itemIds: string[] = []) {
  const type = ccc.Script.from({
    codeHash: REGISTRY_CODE_HASH,
    hashType: "type",
    args: encodeRegistryScriptArgs({
      registryId: REGISTRY_ID,
      adminLockHash: ADMIN_LOCK.hash(),
      stateCodeHash: STATE_CODE_HASH,
      stateHashType: 0x02,
    }),
  });
  return makeCell(
    "77",
    0,
    CAPACITY,
    ADMIN_LOCK,
    type,
    encodeRegistryData({ registryId: REGISTRY_ID, itemIds }),
  );
}

function canonicalStateCell(status: "AVAILABLE" | "BORROWED" | "RETURNED") {
  const registryType = canonicalRegistryCell().cellOutput.type!;
  const type = ccc.Script.from({
    codeHash: STATE_CODE_HASH,
    hashType: "data1",
    args: encodeStateScriptArgs({
      itemId: ITEM_ID,
      trustedRegistryTypeHash: registryType.hash(),
      adminLockHash: ADMIN_LOCK.hash(),
    }),
  });
  return makeCell(
    "88",
    1,
    CAPACITY,
    ADMIN_LOCK,
    type,
    encodeStateCellData({ itemId: ITEM_ID, status }),
  );
}

function genesisOptions(seedCell = makeCell(
  "99",
  0,
  CAPACITY,
  ADMIN_LOCK,
  undefined,
  "0x",
)) {
  return {
    seedCell,
    registryCodeHash: REGISTRY_CODE_HASH,
    registryHashType: "type",
    stateCodeHash: STATE_CODE_HASH,
    stateHashType: 0x02 as const,
    adminLock: ADMIN_LOCK,
    cellDeps: [CODE_DEP],
  };
}

describe("Registry Genesis transaction builder", () => {
  it("derives Type ID from Input[0] and keeps Registry at output index 0", () => {
    const seed = makeCell("99", 0, CAPACITY, ADMIN_LOCK, undefined, "0x");
    const tx = buildRegistryGenesisTransaction(genesisOptions(seed));
    const expectedId = deriveRegistryId({
      previousOutput: {
        txHash: seed.outPoint.txHash,
        index: seed.outPoint.index.toString(),
      },
      since: 0n,
    }, 0n);

    expect(tx.outputs).toHaveLength(2);
    expect(decodeRegistryScriptArgs(tx.outputs[0].type!.args).registryId).toBe(expectedId);
    expect(decodeRegistryData(tx.outputsData[0])).toEqual({
      schemaVersion: 0x01,
      registryId: expectedId,
      itemCount: 0,
      itemIds: [],
    });
    expect(tx.outputs[1].type).toBeUndefined();
    expect(tx.inputs[0].previousOutput.txHash).toBe(seed.outPoint.txHash);
    expect(tx.witnesses).toHaveLength(tx.inputs.length);
    expect(tx.witnesses.every((witness) => witness === "0x")).toBe(true);
  });

  it("rejects unauthorized seeds, canonical State inputs, and undersized outputs", () => {
    const otherLock = ccc.Script.from({
      codeHash: "0x" + "12".repeat(32),
      hashType: "type",
      args: "0x",
    });
    expect(() => buildRegistryGenesisTransaction(
      genesisOptions(makeCell("99", 0, CAPACITY, otherLock, undefined, "0x")),
    )).toThrow(/seed input Lock Script hash/);
    expect(() => buildRegistryGenesisTransaction({
      ...genesisOptions(),
      feeInputs: [canonicalStateCell("AVAILABLE")],
    })).toThrow(/another canonical State cell/);
    expect(() => buildRegistryGenesisTransaction({
      ...genesisOptions(),
      registryCapacity: 1n,
    })).toThrow(/occupied and preserved capacity/);
  });
});

describe("atomic item Registration transaction builder", () => {
  it("updates Registry and creates one AVAILABLE State output", () => {
    const registryCell = canonicalRegistryCell([OTHER_ITEM_ID]);
    const feeCell = makeCell("aa", 0, CAPACITY, ADMIN_LOCK, undefined, "0x");
    const tx = buildAtomicRegistrationTransaction({
      registryCell,
      itemId: ITEM_ID,
      feeInputs: [feeCell],
      cellDeps: [CODE_DEP],
    });
    const nextRegistry = decodeRegistryData(tx.outputsData[0]);
    const stateType = tx.outputs[1].type!;
    const stateData = decodeStateCellData(tx.outputsData[1]);

    expect(tx.inputs).toHaveLength(2);
    expect(tx.outputs[0].type!.hash()).toBe(registryCell.cellOutput.type!.hash());
    expect(nextRegistry.itemIds).toEqual([OTHER_ITEM_ID, ITEM_ID]);
    expect(decodeStateScriptArgs(stateType.args)).toEqual({
      itemId: ITEM_ID,
      trustedRegistryTypeHash: registryCell.cellOutput.type!.hash(),
      adminLockHash: ADMIN_LOCK.hash(),
    });
    expect(stateData).toEqual({ itemId: ITEM_ID, status: "AVAILABLE" });
    expect(tx.outputs[0].capacity.toString()).toBe(registryCell.cellOutput.capacity.toString());
    expect(tx.outputs).toHaveLength(3);
  });

  it("rejects duplicate IDs, Registry-as-CellDep, and reduced Registry capacity", () => {
    const registryCell = canonicalRegistryCell([ITEM_ID]);
    expect(() => buildAtomicRegistrationTransaction({
      registryCell,
      itemId: ITEM_ID,
    })).toThrow(/duplicates/);
    expect(() => buildAtomicRegistrationTransaction({
      registryCell,
      itemId: OTHER_ITEM_ID,
      cellDeps: [{ outPoint: registryCell.outPoint, depType: "code" }],
    })).toThrow(/input cannot also be a cell dep/);
    expect(() => buildAtomicRegistrationTransaction({
      registryCell,
      itemId: OTHER_ITEM_ID,
      registryCapacity: 1n,
    })).toThrow(/occupied and preserved capacity/);
  });
});

describe("State lifecycle transaction builders", () => {
  it("builds the three contract-permitted status transitions", () => {
    const registryCell = canonicalRegistryCell([ITEM_ID]);
    const borrow = buildBorrowTransaction({
      stateCell: canonicalStateCell("AVAILABLE"),
      registryCell,
      cellDeps: [CODE_DEP],
    });
    const returned = buildConfirmReturnTransaction({
      stateCell: canonicalStateCell("BORROWED"),
      registryCell,
    });
    const available = buildMakeAvailableTransaction({
      stateCell: canonicalStateCell("RETURNED"),
      registryCell,
    });

    expect(decodeStateCellData(borrow.outputsData[0]).status).toBe("BORROWED");
    expect(decodeStateCellData(returned.outputsData[0]).status).toBe("RETURNED");
    expect(decodeStateCellData(available.outputsData[0]).status).toBe("AVAILABLE");
  });

  it("adds one trusted Registry CellDep and preserves State identity/capacity", () => {
    const stateCell = canonicalStateCell("AVAILABLE");
    const registryCell = canonicalRegistryCell([ITEM_ID]);
    const tx = buildBorrowTransaction({ stateCell, registryCell });

    expect(tx.inputs).toHaveLength(1);
    expect(tx.cellDeps).toHaveLength(1);
    expect(tx.cellDeps[0].outPoint.txHash).toBe(registryCell.outPoint.txHash);
    expect(tx.cellDeps[0].depType).toBe("code");
    expect(tx.outputs[0].type!.hash()).toBe(stateCell.cellOutput.type!.hash());
    expect(tx.outputs[0].lock.hash()).toBe(stateCell.cellOutput.lock.hash());
    expect(tx.outputs[0].capacity.toString()).toBe(stateCell.cellOutput.capacity.toString());
    expect(tx.witnesses).toEqual(["0x"]);
  });

  it("rejects invalid transitions, unregistered IDs, and reduced State capacity", () => {
    const registryCell = canonicalRegistryCell([ITEM_ID]);
    expect(() => buildBorrowTransaction({
      stateCell: canonicalStateCell("BORROWED"),
      registryCell,
    })).toThrow(/requires a AVAILABLE State input/);
    expect(() => buildBorrowTransaction({
      stateCell: canonicalStateCell("AVAILABLE"),
      registryCell: canonicalRegistryCell([]),
    })).toThrow(/does not contain this State item ID/);
    expect(() => buildBorrowTransaction({
      stateCell: canonicalStateCell("AVAILABLE"),
      registryCell,
      stateCapacity: 1n,
    })).toThrow(/occupied and preserved capacity/);
  });
});
