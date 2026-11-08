const SCHEMA_VERSION = 0x01;
const STATE_SCRIPT_ARGS_BYTES = 97;
const BYTE32_BYTES = 32;

export interface StateScriptArgs {
  itemId: string;
  trustedRegistryTypeHash: string;
  adminLockHash: string;
}

function normalizeHexPayload(hex: string, field: string): string {
  const raw = hex.startsWith("0x") || hex.startsWith("0X")
    ? hex.slice(2)
    : hex;

  if (raw.length % 2 !== 0 || !/^[0-9a-fA-F]*$/.test(raw)) {
    throw new Error(field + " must contain valid hexadecimal data.");
  }

  return raw.toLowerCase();
}

function bytesFromHex(hex: string, field: string): Uint8Array {
  const normalized = normalizeHexPayload(hex, field);
  const bytes = new Uint8Array(normalized.length / 2);

  for (let index = 0; index < bytes.length; index += 1) {
    const offset = index * 2;
    bytes[index] = Number.parseInt(normalized.slice(offset, offset + 2), 16);
  }

  return bytes;
}

function hexFromBytes(bytes: Uint8Array): string {
  const hex = Array.from(bytes, (byte) =>
    byte.toString(16).padStart(2, "0"),
  ).join("");

  return "0x" + hex;
}

function byte32(value: string, field: string): Uint8Array {
  const bytes = bytesFromHex(value, field);

  if (bytes.length !== BYTE32_BYTES) {
    throw new Error(field + " must be exactly 32 bytes.");
  }

  return bytes;
}

export function encodeStateScriptArgs(args: StateScriptArgs): string {
  const bytes = new Uint8Array(STATE_SCRIPT_ARGS_BYTES);
  bytes[0] = SCHEMA_VERSION;
  bytes.set(byte32(args.itemId, "Item ID"), 1);
  bytes.set(byte32(args.trustedRegistryTypeHash, "Trusted Registry type hash"), 33);
  bytes.set(byte32(args.adminLockHash, "Admin lock hash"), 65);

  return hexFromBytes(bytes);
}

export function decodeStateScriptArgs(data: string): StateScriptArgs {
  const bytes = bytesFromHex(data, "State script args");

  if (bytes.length !== STATE_SCRIPT_ARGS_BYTES) {
    throw new Error("State script args must be exactly 97 bytes.");
  }

  if (bytes[0] !== SCHEMA_VERSION) {
    throw new Error("Unsupported State script args schema version.");
  }

  return {
    itemId: hexFromBytes(bytes.slice(1, 33)),
    trustedRegistryTypeHash: hexFromBytes(bytes.slice(33, 65)),
    adminLockHash: hexFromBytes(bytes.slice(65, 97)),
  };
}
