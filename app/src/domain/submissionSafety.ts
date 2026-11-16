export type SubmissionState = "ready" | "submitting" | "submitted" | "confirmed" | "uncertain";

export type ConfirmationOutcome =
  | { kind: "confirmed"; blockNumber: string }
  | { kind: "pending" }
  | { kind: "unavailable"; error: unknown };

export function canStartSubmission(state: SubmissionState, busy: boolean): boolean {
  return state === "ready" && !busy;
}

export function transactionHashesMatch(expected: string, actual: string): boolean {
  return expected.toLowerCase() === actual.toLowerCase();
}

export function classifySubmissionFailure(sendStarted: boolean): "not-submitted" | "uncertain" {
  return sendStarted ? "uncertain" : "not-submitted";
}

export async function findFirstUnavailableInput<T>(
  inputs: readonly T[],
  isLive: (input: T) => Promise<boolean>,
): Promise<T | null> {
  for (const input of inputs) {
    if (!(await isLive(input))) {
      return input;
    }
  }
  return null;
}

export async function observeTransactionConfirmation(
  wait: () => Promise<{ blockNumber?: unknown } | null | undefined>,
): Promise<ConfirmationOutcome> {
  try {
    const transaction = await wait();
    if (transaction?.blockNumber != null) {
      return { kind: "confirmed", blockNumber: String(transaction.blockNumber) };
    }
    return { kind: "pending" };
  } catch (error) {
    return { kind: "unavailable", error };
  }
}