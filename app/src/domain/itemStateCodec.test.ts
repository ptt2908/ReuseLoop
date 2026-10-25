
import { describe, expect, it } from "vitest";
import { decodeItemState, encodeItemState } from "./itemStateCodec";
import type { ItemState } from "./itemState";

const validState: ItemState = {
  itemId: `0x${"ab".repeat(32)}`,
  status: "AVAILABLE",
};

describe("encodeItemState", () => {
  it("encodes a state into a 34-byte payload", () => {
    const data = encodeItemState(validState);

    expect(data).toBe(`0x01${"ab".repeat(32)}00`);
    expect(data.slice(2)).toHaveLength(68);
  });

  it("encodes each supported status", () => {
    expect(
      encodeItemState({ ...validState, status: "AVAILABLE" }).slice(-2),
    ).toBe("00");

    expect(
      encodeItemState({ ...validState, status: "BORROWED" }).slice(-2),
    ).toBe("01");

    expect(
      encodeItemState({ ...validState, status: "RETURNED" }).slice(-2),
    ).toBe("02");
  });

  it("accepts an item ID without the 0x prefix", () => {
    const data = encodeItemState({
      itemId: "ab".repeat(32),
      status: "BORROWED",
    });

    expect(data).toBe(`0x01${"ab".repeat(32)}01`);
  });

  it("rejects an invalid item ID", () => {
    expect(() =>
      encodeItemState({ itemId: "item-001", status: "AVAILABLE" }),
    ).toThrow("Item ID must contain exactly 32 bytes of hex data.");
  });
});

describe("decodeItemState", () => {
  it("decodes a valid payload", () => {
    const decoded = decodeItemState(`0x01${"ab".repeat(32)}01`);

    expect(decoded).toEqual({
      itemId: `0x${"ab".repeat(32)}`,
      status: "BORROWED",
    });
  });

  it("round-trips all supported statuses", () => {
    const statuses = ["AVAILABLE", "BORROWED", "RETURNED"] as const;

    for (const status of statuses) {
      const original = { ...validState, status };
      expect(decodeItemState(encodeItemState(original))).toEqual(original);
    }
  });

  it("rejects a payload with an invalid length", () => {
    expect(() => decodeItemState("0x0100")).toThrow(
      "Cell data must be exactly 34 bytes.",
    );
  });

  it("rejects an unsupported schema version", () => {
    expect(() =>
      decodeItemState(`0x02${"ab".repeat(32)}00`),
    ).toThrow("Unsupported item state schema version.");
  });

  it("rejects an invalid status byte", () => {
    expect(() =>
      decodeItemState(`0x01${"ab".repeat(32)}ff`),
    ).toThrow("Invalid item status byte.");
  });

  it("rejects non-hexadecimal data", () => {
    expect(() =>
      decodeItemState(`0x01${"ab".repeat(31)}zz00`),
    ).toThrow("Cell data must contain valid hexadecimal data.");
  });
});
