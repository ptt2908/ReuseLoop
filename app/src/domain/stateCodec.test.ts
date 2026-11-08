import { describe, expect, it } from "vitest";
import {
  decodeStateScriptArgs,
  encodeStateScriptArgs,
  type StateScriptArgs,
} from "./stateCodec";

const validArgs: StateScriptArgs = {
  itemId: "0x" + "07".repeat(32),
  trustedRegistryTypeHash: "0x" + "ee".repeat(32),
  adminLockHash: "0x" + "aa".repeat(32),
};

describe("State script args codec", () => {
  it("matches the 97-byte State args test vector", () => {
    const encoded = encodeStateScriptArgs(validArgs);

    expect(encoded).toBe(
      "0x01" +
        "07".repeat(32) +
        "ee".repeat(32) +
        "aa".repeat(32),
    );
    expect(encoded.slice(2)).toHaveLength(97 * 2);
  });

  it("round-trips State args", () => {
    expect(decodeStateScriptArgs(encodeStateScriptArgs(validArgs)))
      .toEqual(validArgs);
  });

  it("rejects invalid length and schema version", () => {
    expect(() => decodeStateScriptArgs("0x01")).toThrow(
      "State script args must be exactly 97 bytes.",
    );

    const encoded = encodeStateScriptArgs(validArgs);
    expect(() => decodeStateScriptArgs("0x02" + encoded.slice(4)))
      .toThrow("Unsupported State script args schema version.");
  });

  it("rejects malformed byte32 values and non-hex data", () => {
    expect(() =>
      encodeStateScriptArgs({ ...validArgs, itemId: "0x" + "07".repeat(31) }),
    ).toThrow("Item ID must be exactly 32 bytes.");

    expect(() =>
      decodeStateScriptArgs("0x" + "zz".repeat(97)),
    ).toThrow("State script args must contain valid hexadecimal data.");
  });
});
