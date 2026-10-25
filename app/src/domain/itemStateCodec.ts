
import type { ItemState, ItemStatus } from "./itemState";

const SCHEMA_VERSION = 0x01;
// const ITEM_ID_BYTES = 32;
const PAYLOAD_BYTES = 34;

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

function normalizeItemId(itemId: string): string {
  const normalized = itemId.startsWith("0x")
    ? itemId.slice(2)
    : itemId;

  if (!/^[0-9a-fA-F]{64}$/.test(normalized)) {
    throw new Error("Item ID must contain exactly 32 bytes of hex data.");
  }

  return normalized.toLowerCase();
}

function normalizeHex(data: string): string {
  const normalized = data.startsWith("0x")
    ? data.slice(2)
    : data;

  if (!/^[0-9a-fA-F]*$/.test(normalized)) {
    throw new Error("Cell data must contain valid hexadecimal data.");
  }

  return normalized.toLowerCase();
}

export function encodeItemState(state: ItemState): string {
  const itemId = normalizeItemId(state.itemId);
  const statusByte = STATUS_TO_BYTE[state.status];

  if (statusByte === undefined) {
    throw new Error("Unsupported item status.");
  }

  const data =
    SCHEMA_VERSION.toString(16).padStart(2, "0") +
    itemId +
    statusByte.toString(16).padStart(2, "0");

  return `0x${data}`;
}

export function decodeItemState(data: string): ItemState {
  const normalized = normalizeHex(data);

  if (normalized.length !== PAYLOAD_BYTES * 2) {
    throw new Error("Cell data must be exactly 34 bytes.");
  }

  const version = Number.parseInt(normalized.slice(0, 2), 16);

  if (version !== SCHEMA_VERSION) {
    throw new Error("Unsupported item state schema version.");
  }

  const itemId = `0x${normalized.slice(2, 66)}`;
  const statusByte = Number.parseInt(normalized.slice(66, 68), 16);
  const status = BYTE_TO_STATUS[statusByte];

  if (status === undefined) {
    throw new Error("Invalid item status byte.");
  }

  return {
    itemId,
    status,
  };
}
