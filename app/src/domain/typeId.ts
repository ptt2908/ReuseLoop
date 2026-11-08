import { ccc } from "@ckb-ccc/ccc";

const SERIALIZED_CELL_INPUT_BYTES = 44;
const TYPE_ID_PREIMAGE_BYTES = 52;
const MAX_U64 = (1n << 64n) - 1n;

type IntegerLike = string | number | bigint;

interface TypeIdOutPoint {
  txHash: string;
  index: IntegerLike;
}

export type TypeIdCellInput = (
  | { previousOutput: TypeIdOutPoint; outPoint?: never }
  | { outPoint: TypeIdOutPoint; previousOutput?: never }
) & {
  since?: IntegerLike | null;
};

function normalizeHexPayload(hex: string, field: string): string {
  const raw = hex.startsWith("0x") || hex.startsWith("0X")
    ? hex.slice(2)
    : hex;

  if (raw.length % 2 !== 0 || !/^[0-9a-fA-F]*$/.test(raw)) {
    throw new Error(field + " must contain valid hexadecimal data.");
  }

  return raw.toLowerCase();
}

function toUnsignedInteger(value: IntegerLike, field: string): bigint {
  if (typeof value === "number" && !Number.isSafeInteger(value)) {
    throw new Error(field + " must be a safe integer or bigint/string.");
  }

  try {
    return typeof value === "bigint" ? value : BigInt(value);
  } catch {
    throw new Error(field + " must be an integer.");
  }
}

function toLittleEndianHex(
  value: IntegerLike,
  byteLength: number,
  field: string,
): string {
  let remaining = toUnsignedInteger(value, field);
  const maxValue = (1n << BigInt(byteLength * 8)) - 1n;

  if (remaining < 0n || remaining > maxValue) {
    throw new Error(
      field + " must fit in an unsigned " + (byteLength * 8) + "-bit integer.",
    );
  }

  let littleEndian = "";
  for (let index = 0; index < byteLength; index += 1) {
    littleEndian += Number(remaining & 0xffn).toString(16).padStart(2, "0");
    remaining >>= 8n;
  }

  return littleEndian;
}

function outputIndexToLittleEndianHex(outputIndex: number | bigint): string {
  if (
    typeof outputIndex === "number" &&
    !Number.isSafeInteger(outputIndex)
  ) {
    throw new Error("Global output index must be a safe integer or bigint.");
  }

  const remaining = typeof outputIndex === "bigint"
    ? outputIndex
    : BigInt(outputIndex);

  if (remaining < 0n || remaining > MAX_U64) {
    throw new Error("Global output index must fit in an unsigned 64-bit integer.");
  }

  return toLittleEndianHex(remaining, 8, "Global output index");
}

/**
 * Serializes CellInput as Molecule bytes: since (8-byte LE), then
 * previous_output (32-byte tx hash followed by its 4-byte LE index).
 */
export function serializeCellInputForTypeId(
  firstCellInput: TypeIdCellInput,
): string {
  const previousOutput = firstCellInput.previousOutput ??
    firstCellInput.outPoint;

  if (!previousOutput) {
    throw new Error("CellInput must include previousOutput or outPoint.");
  }

  const txHash = normalizeHexPayload(
    previousOutput.txHash,
    "CellInput previous_output tx_hash",
  );
  if (txHash.length !== 32 * 2) {
    throw new Error(
      "CellInput previous_output tx_hash must be exactly 32 bytes.",
    );
  }

  const since = toLittleEndianHex(
    firstCellInput.since ?? 0,
    8,
    "CellInput since",
  );
  const outPointIndex = toLittleEndianHex(
    previousOutput.index,
    4,
    "CellInput previous_output index",
  );
  const serialized = since + txHash + outPointIndex;

  if (serialized.length !== SERIALIZED_CELL_INPUT_BYTES * 2) {
    throw new Error("Serialized CellInput must be exactly 44 bytes.");
  }

  return "0x" + serialized;
}

/**
 * Builds the exact 52-byte Type ID preimage used by the Registry contract.
 */
export function serializeRegistryTypeIdPreimage(
  firstCellInput: TypeIdCellInput,
  globalOutputIndex: number | bigint,
): string {
  const serializedInput = serializeCellInputForTypeId(firstCellInput);
  const input = normalizeHexPayload(serializedInput, "Serialized CellInput");
  const preimage = input + outputIndexToLittleEndianHex(globalOutputIndex);

  if (preimage.length !== TYPE_ID_PREIMAGE_BYTES * 2) {
    throw new Error("Registry Type ID preimage must be exactly 52 bytes.");
  }

  return "0x" + preimage;
}

export function deriveRegistryId(
  firstCellInput: TypeIdCellInput,
  globalOutputIndex: number | bigint,
): string {
  return ccc.hashCkb(
    serializeRegistryTypeIdPreimage(firstCellInput, globalOutputIndex),
  );
}
