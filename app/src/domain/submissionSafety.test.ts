import { describe, expect, it, vi } from "vitest";
import {
  canStartSubmission,
  classifySubmissionFailure,
  findFirstUnavailableInput,
  observeTransactionConfirmation,
  transactionHashesMatch,
  type SubmissionState,
} from "./submissionSafety";

describe("submission safety", () => {
  it("allows a transaction only from the ready, idle state", () => {
    const states: SubmissionState[] = [
      "submitting",
      "submitted",
      "confirmed",
      "uncertain",
    ];

    expect(canStartSubmission("ready", false)).toBe(true);
    expect(canStartSubmission("ready", true)).toBe(false);
    for (const state of states) {
      expect(canStartSubmission(state, false)).toBe(false);
      expect(canStartSubmission(state, true)).toBe(false);
    }
  });

  it("compares RPC transaction hashes without case sensitivity", () => {
    expect(transactionHashesMatch("0xABCD", "0xabcd")).toBe(true);
    expect(transactionHashesMatch("0xABCD", "0xabce")).toBe(false);
  });

  it("stops the preflight at the first consumed input", async () => {
    const visited: string[] = [];
    const unavailable = await findFirstUnavailableInput(
      ["live-a", "spent-b", "live-c"],
      async (input) => {
        visited.push(input);
        return input !== "spent-b";
      },
    );

    expect(unavailable).toBe("spent-b");
    expect(visited).toEqual(["live-a", "spent-b"]);
  });

  it("returns no stale input when all inputs are live", async () => {
    await expect(
      findFirstUnavailableInput(["input-a", "input-b"], async () => true),
    ).resolves.toBeNull();
  });

  it("classifies failures before send as not submitted and failures after send as uncertain", () => {
    expect(classifySubmissionFailure(false)).toBe("not-submitted");
    expect(classifySubmissionFailure(true)).toBe("uncertain");
  });
});

describe("transaction confirmation polling", () => {
  it("reports a block-confirmed transaction", async () => {
    await expect(
      observeTransactionConfirmation(async () => ({ blockNumber: 42 })),
    ).resolves.toEqual({ kind: "confirmed", blockNumber: "42" });
  });

  it("keeps a delayed transaction pending when no block is available", async () => {
    await expect(
      observeTransactionConfirmation(async () => null),
    ).resolves.toEqual({ kind: "pending" });
    await expect(
      observeTransactionConfirmation(async () => ({ blockNumber: null })),
    ).resolves.toEqual({ kind: "pending" });
  });

  it("distinguishes an RPC polling failure from a pending transaction", async () => {
    const timeout = new Error("RPC timeout");
    const wait = vi.fn(async () => {
      throw timeout;
    });

    await expect(observeTransactionConfirmation(wait)).resolves.toEqual({
      kind: "unavailable",
      error: timeout,
    });
  });
});