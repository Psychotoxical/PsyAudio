import { beforeEach, describe, expect, it, vi } from "vitest";

const calls: { cmd: string; args: Record<string, unknown> }[] = [];

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string, args: Record<string, unknown>) => {
    calls.push({ cmd, args });
  }),
  Channel: class {
    onmessage: (e: unknown) => void = () => {};
  },
}));

import { type MeasureEvent, measureDrive } from "./cd";

describe("cd commands", () => {
  beforeEach(() => {
    calls.length = 0;
  });

  it("measureDrive_forwards_events", async () => {
    const seen: MeasureEvent[] = [];
    await measureDrive("G", (e) => seen.push(e));
    expect(calls[0]?.cmd).toBe("measure_drive");
    expect(calls[0]?.args.drive).toBe("G");
    const on = calls[0]?.args.on as { onmessage: (e: MeasureEvent) => void };
    on.onmessage({ kind: "Progress", fraction: 0.5 });
    on.onmessage({ kind: "Failed", reason: "keine CD" });
    expect(seen).toEqual([
      { kind: "Progress", fraction: 0.5 },
      { kind: "Failed", reason: "keine CD" },
    ]);
  });
});
