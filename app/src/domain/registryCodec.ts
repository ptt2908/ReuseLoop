const SCHEMA_VERSION = 0x01;
const MAX_ITEMS = 100;
const REGISTRY_SCRIPT_ARGS_BYTES = 98;
const REGISTRY_DATA_HEADER_BYTES = 37;
const BYTE32_BYTES = 32;

export type StateHashType = 0x00 | 0x01 | 0x02 | 0x04;

export interface RegistryScriptArgs {
  registryId: string;
  adminLockHash: string;
  stateCodeHash: string;
  stateHashType: StateHashType;
}

export interface RegistryDataInput {
  registryId: string;
  itemIds: string[];
}

export interface DecodedRegistryData extends RegistryDataInput {
  schemaVersion: 0x01;
  itemCount: number;
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

function concatBytes(chunks: Uint8Array[]): Uint8Array {
  const totalLength = chunks.reduce((sum, chunk) => sum + chunk.length, 0);
  const result = new Uint8Array(totalLength);
  let offset = 0;

  for (const chunk of chunks) {
    result.set(chunk, offset);
    offset += chunk.length;
  }

  return result;
}

function isValidStateHashType(value: number): value is StateHashType {
  return value === 0x00 ||
    value === 0x01 ||
    value === 0x02 ||
    value === 0x04;
}

function assertUniqueItemIds(itemIds: string[]): void {
  if (new Set(itemIds).size !== itemIds.length) {
    throw new Error("Registry item IDs must not contain duplicates.");
  }
}

export function encodeRegistryScriptArgs(args: RegistryScriptArgs): string {
  if (!isValidStateHashType(args.stateHashType)) {
    throw new Error("Registry state hash type must be one of 0, 1, 2, or 4.");
  }

  const bytes = concatBytes([
    Uint8Array.of(SCHEMA_VERSION),
    byte32(args.registryId, "Registry ID"),
    byte32(args.adminLockHash, "Admin lock hash"),
    byte32(args.stateCodeHash, "State code hash"),
    Uint8Array.of(args.stateHashType),
  ]);

  if (bytes.length !== REGISTRY_SCRIPT_ARGS_BYTES) {
    throw new Error("Registry script args must be exactly 98 bytes.");
  }

  return hexFromBytes(bytes);
}

export function decodeRegistryScriptArgs(data: string): RegistryScriptArgs {
  const bytes = bytesFromHex(data, "Registry script args");

  if (bytes.length !== REGISTRY_SCRIPT_ARGS_BYTES) {
    throw new Error("Registry script args must be exactly 98 bytes.");
  }

  if (bytes[0] !== SCHEMA_VERSION) {
    throw new Error("Unsupported Registry script args schema version.");
  }

  const stateHashType = bytes[97];
  if (!isValidStateHashType(stateHashType)) {
    throw new Error("Registry state hash type must be one of 0, 1, 2, or 4.");
  }

  return {
    registryId: hexFromBytes(bytes.slice(1, 33)),
    adminLockHash: hexFromBytes(bytes.slice(33, 65)),
    stateCodeHash: hexFromBytes(bytes.slice(65, 97)),
    stateHashType,
  };
}

export function encodeRegistryData(data: RegistryDataInput): string {
  if (data.itemIds.length > MAX_ITEMS) {
    throw new Error("Registry cannot contain more than 100 items.");
  }

  const registryId = byte32(data.registryId, "Registry ID");
  const itemIds = data.itemIds.map((itemId) => byte32(itemId, "Item ID"));
  const normalizedItemIds = itemIds.map((itemId) => hexFromBytes(itemId));
  assertUniqueItemIds(normalizedItemIds);

  const count = new Uint8Array(4);
  new DataView(count.buffer).setUint32(0, itemIds.length, true);

  const bytes = concatBytes([
    Uint8Array.of(SCHEMA_VERSION),
    registryId,
    count,
    ...itemIds,
  ]);

  return hexFromBytes(bytes);
}

export function decodeRegistryData(data: string): DecodedRegistryData {
  const bytes = bytesFromHex(data, "Registry cell data");

  if (bytes.length < REGISTRY_DATA_HEADER_BYTES) {
    throw new Error("Registry cell data must be at least 37 bytes.");
  }

  if (bytes[0] !== SCHEMA_VERSION) {
    throw new Error("Unsupported Registry cell data schema version.");
  }

  const itemCount = new DataView(
    bytes.buffer,
    bytes.byteOffset,
    bytes.byteLength,
  ).getUint32(33, true);

  if (itemCount > MAX_ITEMS) {
    throw new Error("Registry cannot contain more than 100 items.");
  }

  const expectedLength = REGISTRY_DATA_HEADER_BYTES + BYTE32_BYTES * itemCount;
  if (bytes.length !== expectedLength) {
    throw new Error("Registry cell data length must equal 37 + 32 * item_count.");
  }

  const itemIds: string[] = [];
  for (let index = 0; index < itemCount; index += 1) {
    const start = REGISTRY_DATA_HEADER_BYTES + BYTE32_BYTES * index;
    itemIds.push(hexFromBytes(bytes.slice(start, start + BYTE32_BYTES)));
  }
  assertUniqueItemIds(itemIds);

  return {
    schemaVersion: SCHEMA_VERSION,
    registryId: hexFromBytes(bytes.slice(1, 33)),
    itemCount,
    itemIds,
  };
}
