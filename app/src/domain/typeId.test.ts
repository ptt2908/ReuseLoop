import { describe, expect, it } from "vitest";
import {
  deriveRegistryId,
  serializeCellInputForTypeId,
  serializeRegistryTypeIdPreimage,
  type TypeIdCellInput,
} from "./typeId";

const firstCellInput: TypeIdCellInput = {
  since: 0x0807060504030201n,
  previousOutput: {
    txHash: "0x" + "aa".repeat(32),
    index: 0x04030201,
  },
};

describe("Registry Type ID derivation", () => {
  it("serializes CellInput fields in canonical Molecule order", () => {
    expect(serializeCellInputForTypeId(firstCellInput)).toBe(
      "0x" +
        "0102030405060708" +
        "aa".repeat(32) +
        "01020304",
    );
  });

  it("serializes the canonical 52-byte preimage with a little-endian output index", () => {
    const preimage = serializeRegistryTypeIdPreimage(
      firstCellInput,
      0x0102030405060708n,
    );

    expect(preimage).toBe(
      "0x" +
        "0102030405060708" +
        "aa".repeat(32) +
        "01020304" +
        "0807060504030201",
    );
    expect(preimage.slice(2)).toHaveLength(52 * 2);
  });

  it("matches a fixed CKB Blake2b Type ID vector", () => {
    expect(
      deriveRegistryId(firstCellInput, 0x0102030405060708n),
    ).toBe("0xe6bb108dc9cc8ec0c0da3ab2cf8409c0662b0b67185452b6966d6cffee9128f3");
  });

  it("accepts the CCC outPoint alias for previous_output", () => {
    const input: TypeIdCellInput = {
      since: 0,
      outPoint: {
        txHash: "0x" + "11".repeat(32),
        index: 2,
      },
    };

    expect(serializeCellInputForTypeId(input))
      .toBe("0x" + "00".repeat(8) + "11".repeat(32) + "02000000");
  });

  it("supports output indexes at both u64 boundaries", () => {
    expect(serializeRegistryTypeIdPreimage(firstCellInput, 0).slice(-16))
      .toBe("0000000000000000");
    expect(
      serializeRegistryTypeIdPreimage(
        firstCellInput,
        (1n << 64n) - 1n,
      ).slice(-16),
    ).toBe("ffffffffffffffff");
  });

  it("rejects invalid CellInput fields and output indexes", () => {
    expect(() =>
      serializeCellInputForTypeId({
        previousOutput: { txHash: "0x0102", index: 0 },
      }),
    ).toThrow("CellInput previous_output tx_hash must be exactly 32 bytes.");

    expect(() =>
      serializeCellInputForTypeId({
        previousOutput: {
          txHash: "0x" + "11".repeat(32),
          index: 1n << 32n,
        },
      }),
    ).toThrow(
      "CellInput previous_output index must fit in an unsigned 32-bit integer.",
    );

    expect(() =>
      serializeCellInputForTypeId({
        since: 1n << 64n,
        previousOutput: {
          txHash: "0x" + "11".repeat(32),
          index: 0,
        },
      }),
    ).toThrow("CellInput since must fit in an unsigned 64-bit integer.");

    expect(() =>
      serializeRegistryTypeIdPreimage(firstCellInput, -1),
    ).toThrow("Global output index must fit in an unsigned 64-bit integer.");

    expect(() =>
      serializeRegistryTypeIdPreimage(firstCellInput, 1n << 64n),
    ).toThrow("Global output index must fit in an unsigned 64-bit integer.");

    expect(() =>
      serializeRegistryTypeIdPreimage(
        firstCellInput,
        Number.MAX_SAFE_INTEGER + 1,
      ),
    ).toThrow("Global output index must be a safe integer or bigint.");
  });
});
