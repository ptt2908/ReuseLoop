import { describe, expect, it } from "vitest";
import {
  decodeRegistryData,
  decodeRegistryScriptArgs,
  encodeRegistryData,
  encodeRegistryScriptArgs,
  type RegistryScriptArgs,
  type StateHashType,
} from "./registryCodec";

const registryId = "0x" + "55".repeat(32);
const adminLockHash = "0x" + "aa".repeat(32);
const stateCodeHash = "0x" + "cc".repeat(32);

describe("Registry script args codec", () => {
  const validArgs: RegistryScriptArgs = {
    registryId,
    adminLockHash,
    stateCodeHash,
    stateHashType: 0x01,
  };

  it("matches the 98-byte Registry args test vector", () => {
    const encoded = encodeRegistryScriptArgs(validArgs);

    expect(encoded).toBe(
      "0x01" +
        "55".repeat(32) +
        "aa".repeat(32) +
        "cc".repeat(32) +
        "01",
    );
    expect(encoded.slice(2)).toHaveLength(98 * 2);
  });

  it("round-trips each supported State hash type", () => {
    const hashTypes = [0x00, 0x01, 0x02, 0x04] as const;

    for (const stateHashType of hashTypes) {
      const args = { ...validArgs, stateHashType };
      expect(decodeRegistryScriptArgs(encodeRegistryScriptArgs(args)))
        .toEqual(args);
    }
  });

  it("normalizes uppercase input to lowercase output", () => {
    const encoded = encodeRegistryScriptArgs({
      ...validArgs,
      registryId: registryId.toUpperCase().replace("0X", "0x"),
    });

    expect(decodeRegistryScriptArgs(encoded).registryId).toBe(registryId);
  });

  it("rejects malformed byte32 values", () => {
    expect(() =>
      encodeRegistryScriptArgs({
        ...validArgs,
        registryId: "0x" + "55".repeat(31),
      }),
    ).toThrow("Registry ID must be exactly 32 bytes.");
  });

  it("rejects invalid State hash types when encoding or decoding", () => {
    expect(() =>
      encodeRegistryScriptArgs({
        ...validArgs,
        stateHashType: 0x03 as StateHashType,
      }),
    ).toThrow("Registry state hash type must be one of 0, 1, 2, or 4.");

    const encoded = encodeRegistryScriptArgs(validArgs).slice(0, -2) + "03";
    expect(() => decodeRegistryScriptArgs(encoded))
      .toThrow("Registry state hash type must be one of 0, 1, 2, or 4.");
  });

  it("rejects invalid length and schema version", () => {
    expect(() => decodeRegistryScriptArgs("0x01")).toThrow(
      "Registry script args must be exactly 98 bytes.",
    );

    const encoded = encodeRegistryScriptArgs(validArgs);
    expect(() => decodeRegistryScriptArgs("0x02" + encoded.slice(4)))
      .toThrow("Unsupported Registry script args schema version.");
  });
});

describe("Registry cell data codec", () => {
  it("matches the empty Registry data test vector", () => {
    const encoded = encodeRegistryData({ registryId, itemIds: [] });

    expect(encoded).toBe("0x01" + "55".repeat(32) + "00000000");
    expect(encoded.slice(2)).toHaveLength(37 * 2);
    expect(decodeRegistryData(encoded)).toEqual({
      schemaVersion: 0x01,
      registryId,
      itemCount: 0,
      itemIds: [],
    });
  });

  it("matches the one-item Registry data test vector", () => {
    const itemId = "0x" + "07".repeat(32);
    const encoded = encodeRegistryData({ registryId, itemIds: [itemId] });

    expect(encoded).toBe(
      "0x01" + "55".repeat(32) + "01000000" + "07".repeat(32),
    );
    expect(decodeRegistryData(encoded)).toEqual({
      schemaVersion: 0x01,
      registryId,
      itemCount: 1,
      itemIds: [itemId],
    });
  });

  it("round-trips IDs and normalizes their case", () => {
    const itemIds = ["0x" + "ab".repeat(32), "CD".repeat(32)];
    const encoded = encodeRegistryData({
      registryId: registryId.toUpperCase().replace("0X", "0x"),
      itemIds,
    });

    expect(decodeRegistryData(encoded)).toEqual({
      schemaVersion: 0x01,
      registryId,
      itemCount: 2,
      itemIds: ["0x" + "ab".repeat(32), "0x" + "cd".repeat(32)],
    });
  });

  it("encodes and decodes the 100-item capacity boundary", () => {
    const itemIds = Array.from({ length: 100 }, (_, index) => {
      const byte = index.toString(16).padStart(2, "0");
      return "0x" + byte.repeat(32);
    });
    const encoded = encodeRegistryData({ registryId, itemIds });

    expect(encoded.slice(2)).toHaveLength(3237 * 2);
    expect(encoded.slice(68, 76)).toBe("64000000");
    expect(decodeRegistryData(encoded).itemCount).toBe(100);
  });

  it("rejects more than 100 items and duplicate IDs", () => {
    const tooManyIds = Array.from({ length: 101 }, (_, index) => {
      const byte = index.toString(16).padStart(2, "0");
      return "0x" + byte.repeat(32);
    });

    expect(() =>
      encodeRegistryData({ registryId, itemIds: tooManyIds }),
    ).toThrow("Registry cannot contain more than 100 items.");

    expect(() =>
      encodeRegistryData({
        registryId,
        itemIds: ["0x" + "07".repeat(32), "07".repeat(32)],
      }),
    ).toThrow("Registry item IDs must not contain duplicates.");
  });

  it("rejects malformed Registry data", () => {
    expect(() => decodeRegistryData("0x01")).toThrow(
      "Registry cell data must be at least 37 bytes.",
    );

    expect(() =>
      decodeRegistryData("0x02" + "55".repeat(32) + "00000000"),
    ).toThrow("Unsupported Registry cell data schema version.");

    expect(() =>
      decodeRegistryData("0x01" + "55".repeat(32) + "01000000"),
    ).toThrow("Registry cell data length must equal 37 + 32 * item_count.");

    expect(() =>
      decodeRegistryData("0x01" + "55".repeat(32) + "65000000"),
    ).toThrow("Registry cannot contain more than 100 items.");

    const duplicateItemId = "07".repeat(32);
    const duplicateIds =
      "0x01" + "55".repeat(32) + "02000000" + duplicateItemId.repeat(2);
    expect(() => decodeRegistryData(duplicateIds))
      .toThrow("Registry item IDs must not contain duplicates.");

    expect(() =>
      decodeRegistryData("0x01" + "55".repeat(31) + "00000000"),
    ).toThrow("Registry cell data must be at least 37 bytes.");
  });
});
