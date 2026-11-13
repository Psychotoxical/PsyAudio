import { act, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

const confirmClose = vi.fn((_cancel: boolean) => Promise.resolve());
let fire: () => void = () => {};
vi.mock("../ipc/close", () => ({
  confirmClose: (cancel: boolean) => confirmClose(cancel),
  onCloseRequested: (handler: () => void) => {
    fire = handler;
    return Promise.resolve(() => {});
  },
}));

import { CloseDialog } from "./CloseDialog";

describe("close question", () => {
  it("cancel_and_quit_calls_confirm_true", async () => {
    render(<CloseDialog />);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await act(async () => fire());
    expect(screen.getByRole("dialog", { name: "PsyAudio beenden?" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Abbrechen und beenden" }));
    expect(confirmClose).toHaveBeenCalledWith(true);
  });

  it("keep_working_closes_the_question", async () => {
    render(<CloseDialog />);
    await act(async () => fire());
    fireEvent.click(screen.getByRole("button", { name: "Weiterarbeiten" }));
    expect(confirmClose).toHaveBeenLastCalledWith(false);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
});
