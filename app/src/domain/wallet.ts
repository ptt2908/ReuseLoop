import { ccc } from "@ckb-ccc/ccc";

export interface WalletSession {
  address: string;
  signTransaction(transaction: ccc.TransactionLike): Promise<ccc.Transaction>;
  disconnect(): Promise<void>;
}

type WalletSigner = Pick<ccc.Signer, "signTransaction" | "disconnect">;

export function createWalletSession(
  address: string,
  signer: WalletSigner,
  disposeClient: () => Promise<void>,
): WalletSession {
  let disconnected = false;

  return {
    address,
    signTransaction: (transaction) => signer.signTransaction(transaction),
    disconnect: async () => {
      if (disconnected) {
        return;
      }
      disconnected = true;
      try {
        await signer.disconnect();
      } finally {
        await disposeClient();
      }
    },
  };
}

export async function connectJoyIdTestnet(): Promise<WalletSession> {
  const clientOwner = ccc.ClientPublicTestnet.open();

  try {
    const signerInfo = ccc.JoyId.getJoyIdSigners(
      clientOwner.value,
      "ReuseLoop",
      new URL("/favicon.svg", window.location.href).toString(),
    ).find((info) => info.signer.type === ccc.SignerType.CKB);

    if (!signerInfo) {
      throw new Error("JoyID CKB Testnet signer is unavailable.");
    }

    await signerInfo.signer.connect();
    const address = await signerInfo.signer.getInternalAddress();

    return createWalletSession(
      address,
      signerInfo.signer,
      async () => {
        await clientOwner.dispose();
      },
    );
  } catch (error) {
    await clientOwner.dispose();
    throw error;
  }
}
