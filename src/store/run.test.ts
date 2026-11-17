import { describe, expect, it } from "vitest";
import { applyEvent, canRetry, failedItems, newRun, overallProgress, type Run } from "./run";

function run(states: string[]): Run {
  const r = newRun(
    { title: "Umwandeln nach Opus", target: "Opus", threads: 2 },
    states.map((_, i) => [`C:\\Musik\\${i}.flac`, null]),
    0,
  );
  return states.reduce<Run>((acc, s, id) => {
    if (s === "done")
      return applyEvent(acc, {
        kind: "Done",
        id,
        outputs: [],
        audio_secs: 1,
        wall_secs: 1,
        note: "",
      });
    if (s === "failed") return applyEvent(acc, { kind: "Failed", id, reason: "kaputt" });
    return acc;
  }, r);
}

describe("run", () => {
  it("overall_progress_never_drops", () => {
    const r = applyEvent(run(["waiting", "waiting"]), {
      kind: "Progress",
      id: 0,
      progress: 0.6,
      stage: "Encoding",
    });
    expect(overallProgress(0.5, r.jobs)).toBeCloseTo(0.5);
    expect(overallProgress(0.2, r.jobs)).toBeCloseTo(0.3);
    // a file that starts its next stage at 0 does not pull the bar back
    const back = applyEvent(r, { kind: "Progress", id: 0, progress: 0.1, stage: "Encoding" });
    expect(back.maxOverall).toBeCloseTo(0.3);
  });

  it("retry_only_when_failed", () => {
    expect(canRetry(run(["failed", "done"]))).toBe(false); // still running
    const finished = (r: Run) => applyEvent(r, { kind: "Finished", elapsed: 3 });
    expect(canRetry(finished(run(["done", "done"])))).toBe(false);
    const failed = finished(run(["failed", "done"]));
    expect(canRetry(failed)).toBe(true);
    expect(failedItems(failed)).toEqual([["C:\\Musik\\0.flac", null]]);
  });

  it("events_update_jobs", () => {
    let r = run(["waiting"]);
    r = applyEvent(r, { kind: "Started", id: 0 });
    expect(r.jobs[0]?.state).toBe("running");
    r = applyEvent(r, {
      kind: "Done",
      id: 0,
      outputs: ["C:\\Ziel\\0.opus"],
      audio_secs: 60,
      wall_secs: 2,
      note: "",
    });
    expect(r.jobs[0]).toMatchObject({ state: "done", progress: 1, speed: 30 });
    r = applyEvent(r, { kind: "Report", text: "Album-Gain −3,1 dB" });
    expect(r.reports).toEqual(["Album-Gain −3,1 dB"]);
    r = applyEvent(r, { kind: "Finished", elapsed: 2.5 });
    expect(r.elapsed).toBe(2.5);
  });
});
