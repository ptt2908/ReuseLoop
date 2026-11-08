import type { ItemStatus } from "./itemState";

const SCHEMA_VERSION = 0x01;
const STATE_DATA_BYTES = 34;
const BYTE32_BYTES = 32;

const STATUS_TO_BYTE: Record<ItemStatus, number> = {
  AVAILABLE: 0x00,
  BORROWED: 0x01,
  RETURNED: 0x02,
};

const BYTE_TO_STATUS: Record<number, ItemStatus> = {
  0x00: "AVAILABLE",
  0x01: "BORROWED",
  0x02: "RETURNED",
};

export interface StateCellData {
  itemId: string;
  status: ItemStatus;
}

function normalizeHexPayload(hex: string, field: string): string {
  const raw = hex.startsWith("0x") || hex.startsWith("0X") ? hex.slice(2) : hex;
  if (raw.length % 2 !== 0 || !/^[0-9a-fA-F]*$/.test(raw)) {
    throw new Error(field + " must contain valid hexadecimal data.");
  }
  return raw.toLowerCase();
}

function bytesFromHex(hex: string, field: string): Uint8Array {
  const normalized = normalizeHexPayload(hex, field);
  const bytes = new Uint8Array(normalized.length / 2);
  for (let i = 0; i < bytes.length; i += 1) {
    bytes[i] = Number.parseInt(normalized.slice(i * 2, i * 2 + 2), 16);
  }
  return bytes;
}

function hexFromBytes(bytes: Uint8Array): string {
  return "0x" + Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

export function encodeStateCellData(data: StateCellData): string {
  const itemId = bytesFromHex(data.itemId, "Item ID");
  if (itemId.length !== BYTE32_BYTES) {
    throw new Error("Item ID must be exactly 32 bytes.");
  }
  const status = STATUS_TO_BYTE[data.status];
  if (status === undefined) {
    throw new Error("State status must be AVAILABLE, BORROWED, or RETURNED.");
  }
  const bytes = new Uint8Array(STATE_DATA_BYTES);
  bytes[0] = SCHEMA_VERSION;
  bytes.set(itemId, 1);
  bytes[33] = status;
  return hexFromBytes(bytes);
}

export function decodeStateCellData(data: string): StateCellData {
  const bytes = bytesFromHex(data, "State cell data");
  if (bytes.length !== STATE_DATA_BYTES) {
    throw new Error("State cell data must be exactly 34 bytes.");
  }
  if (bytes[0] !== SCHEMA_VERSION) {
    throw new Error("Unsupported State cell data schema version.");
  }
  const status = BYTE_TO_STATUS[bytes[33]];
  if (status === undefined) {
    throw new Error("State cell data contains an unsupported status byte.");
  }
  return {
    itemId: hexFromBytes(bytes.slice(1, 33)),
    status,
  };
}
