
import { describe, expect, it } from "vitest";
import { transitionItemState } from "./itemState";

describe("transitionItemState", () => {
  it("transitions AVAILABLE to BORROWED", () => {
    const result = transitionItemState(
      { itemId: "item-001", status: "AVAILABLE" },
      "BORROW",
    );

    expect(result).toEqual({
      itemId: "item-001",
      status: "BORROWED",
    });
  });

  it("transitions BORROWED to RETURNED", () => {
    const result = transitionItemState(
      { itemId: "item-001", status: "BORROWED" },
      "CONFIRM_RETURN",
    );

    expect(result.status).toBe("RETURNED");
  });

  it("transitions RETURNED to AVAILABLE", () => {
    const result = transitionItemState(
      { itemId: "item-001", status: "RETURNED" },
      "MAKE_AVAILABLE",
    );

    expect(result.status).toBe("AVAILABLE");
  });

  it("rejects borrowing an item that is not AVAILABLE", () => {
    expect(() =>
      transitionItemState(
        { itemId: "item-001", status: "BORROWED" },
        "BORROW",
      ),
    ).toThrow("Only AVAILABLE items can be borrowed.");
  });

  it("rejects confirming return for an item that is not BORROWED", () => {
    expect(() =>
      transitionItemState(
        { itemId: "item-001", status: "AVAILABLE" },
        "CONFIRM_RETURN",
      ),
    ).toThrow("Only BORROWED items can be returned.");
  });

  it("rejects making an item available before it is RETURNED", () => {
    expect(() =>
      transitionItemState(
        { itemId: "item-001", status: "BORROWED" },
        "MAKE_AVAILABLE",
      ),
    ).toThrow("Only RETURNED items can become AVAILABLE.");
  });

  it("preserves the item ID", () => {
    const current = {
      itemId: "item-123",
      status: "AVAILABLE" as const,
    };

    expect(transitionItemState(current, "BORROW").itemId)
      .toBe("item-123");
  });

  it("does not mutate the input state", () => {
    const current = {
      itemId: "item-001",
      status: "AVAILABLE" as const,
    };

    transitionItemState(current, "BORROW");

    expect(current.status).toBe("AVAILABLE");
  });
});
