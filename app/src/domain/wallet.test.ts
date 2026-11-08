import { ccc } from "@ckb-ccc/ccc";
import { describe, expect, it, vi } from "vitest";
import { createWalletSession } from "./wallet";

describe("wallet session", () => {
  it("signs to a transaction result without exposing a broadcast operation", async () => {
    const unsigned = ccc.Transaction.default();
    const signed = ccc.Transaction.default();
    const signer = {
      signTransaction: vi.fn(async () => signed),
      disconnect: vi.fn(async () => undefined),
    } as unknown as Pick<ccc.Signer, "signTransaction" | "disconnect">;
    const disposeClient = vi.fn(async () => undefined);
    const session = createWalletSession("ckt1-test-address", signer, disposeClient);

    await expect(session.signTransaction(unsigned)).resolves.toBe(signed);
    expect(signer.signTransaction).toHaveBeenCalledWith(unsigned);
    expect("sendTransaction" in session).toBe(false);
  });

  it("disconnects the signer and releases the client once", async () => {
    const signer = {
      signTransaction: vi.fn(async () => ccc.Transaction.default()),
      disconnect: vi.fn(async () => undefined),
    } as unknown as Pick<ccc.Signer, "signTransaction" | "disconnect">;
    const disposeClient = vi.fn(async () => undefined);
    const session = createWalletSession("ckt1-test-address", signer, disposeClient);

    await session.disconnect();
    await session.disconnect();

    expect(signer.disconnect).toHaveBeenCalledTimes(1);
    expect(disposeClient).toHaveBeenCalledTimes(1);
  });
});
