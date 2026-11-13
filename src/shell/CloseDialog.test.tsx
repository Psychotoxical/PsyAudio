import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const order: string[] = [];
const confirmClose = vi.fn((cancel: boolean) => {
  order.push(`confirm ${cancel}`);
  return Promise.resolve();
});
let fireAsk: () => void = () => {};
let fireNow: () => void = () => {};
vi.mock("../ipc/close", () => ({
  confirmClose: (cancel: boolean) => confirmClose(cancel),
  onCloseRequested: (handler: () => void) => {
    fireAsk = handler;
    return Promise.resolve(() => {});
  },
  onCloseNow: (handler: () => void) => {
    fireNow = handler;
    return Promise.resolve(() => {});
  },
}));
vi.mock("../store/settings", () => ({
  flushSettings: () => {
    order.push("flush");
    return Promise.resolve();
  },
}));

import { CloseDialog } from "./CloseDialog";

describe("close question", () => {
  beforeEach(() => {
    order.length = 0;
    confirmClose.mockClear();
  });

  it("cancel_and_quit_calls_confirm_true", async () => {
    render(<CloseDialog />);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await act(async () => fireAsk());
    expect(screen.getByRole("dialog", { name: "PsyAudio beenden?" })).toBeInTheDocument();
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Abbrechen und beenden" }));
    });
    expect(order).toEqual(["flush", "confirm true"]);
  });

  it("keep_working_closes_the_question", async () => {
    render(<CloseDialog />);
    await act(async () => fireAsk());
    fireEvent.click(screen.getByRole("button", { name: "Weiterarbeiten" }));
    expect(confirmClose).toHaveBeenLastCalledWith(false);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("closing_when_idle_saves_settings_first", async () => {
    render(<CloseDialog />);
    await act(async () => fireNow());
    expect(order).toEqual(["flush", "confirm true"]);
  });
});
