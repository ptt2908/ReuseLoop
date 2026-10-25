
export type ItemStatus = "AVAILABLE" | "BORROWED" | "RETURNED";

export type ItemState = {
  itemId: string;
  status: ItemStatus;
};

export type LifecycleAction =
  | "BORROW"
  | "CONFIRM_RETURN"
  | "MAKE_AVAILABLE";

export function transitionItemState(
  current: ItemState,
  action: LifecycleAction,
): ItemState {
  switch (action) {
    case "BORROW":
      if (current.status !== "AVAILABLE") {
        throw new Error("Only AVAILABLE items can be borrowed.");
      }

      return { ...current, status: "BORROWED" };

    case "CONFIRM_RETURN":
      if (current.status !== "BORROWED") {
        throw new Error("Only BORROWED items can be returned.");
      }

      return { ...current, status: "RETURNED" };

    case "MAKE_AVAILABLE":
      if (current.status !== "RETURNED") {
        throw new Error(
          "Only RETURNED items can become AVAILABLE.",
        );
      }

      return { ...current, status: "AVAILABLE" };
  }
}
