import { describe, expect, it } from "vitest";
import { decodeStateCellData, encodeStateCellData } from "./stateDataCodec";

describe("state cell data codec", () => {
  it("round-trips the 34-byte AVAILABLE cell data", () => {
    const data = { itemId: "0x" + "a1".repeat(32), status: "AVAILABLE" as const };
    const encoded = encodeStateCellData(data);
    expect(encoded).toBe("0x01" + "a1".repeat(32) + "00");
    expect((encoded.length - 2) / 2).toBe(34);
    expect(decodeStateCellData(encoded)).toEqual(data);
  });

  it.each([
    ["BORROWED", 1],
    ["RETURNED", 2],
  ] as const)("encodes %s with its contract status byte", (status, byte) => {
    const encoded = encodeStateCellData({
      itemId: "0x" + "03".repeat(32),
      status,
    });
    expect(encoded.slice(-2)).toBe(byte.toString(16).padStart(2, "0"));
    expect(decodeStateCellData(encoded).status).toBe(status);
  });

  it("rejects malformed IDs and unsupported statuses", () => {
    expect(() => encodeStateCellData({
      itemId: "0x1234",
      status: "AVAILABLE",
    })).toThrow(/exactly 32 bytes/);
    expect(() => encodeStateCellData({
      itemId: "0x" + "00".repeat(32),
      status: "UNKNOWN" as never,
    })).toThrow(/must be AVAILABLE/);
  });

  it("rejects malformed on-chain data", () => {
    expect(() => decodeStateCellData("0x01")).toThrow(/exactly 34 bytes/);
    expect(() => decodeStateCellData("0x02" + "00".repeat(32) + "00"))
      .toThrow(/schema version/);
    expect(() => decodeStateCellData("0x01" + "00".repeat(32) + "03"))
      .toThrow(/unsupported status/);
  });
});
