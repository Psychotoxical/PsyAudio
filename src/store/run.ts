import { create } from "zustand";
import type { Codec } from "../ipc/bindings/Codec";

/** What Rust reports about a run (run_cmd::RunEvent). */
export type RunEvent =
  | { kind: "Started"; id: number }
  | { kind: "Progress"; id: number; progress: number; stage: string }
  | {
      kind: "Done";
      id: number;
      outputs: string[];
      audio_secs: number;
      wall_secs: number;
      note: string;
    }
  | { kind: "Skipped"; id: number; reason: string }
  | { kind: "Failed"; id: number; reason: string }
  | { kind: "Report"; text: string }
  | { kind: "Finished"; elapsed: number };

export interface RunInfo {
  title: string;
  target: string;
  threads: number;
}

export type JobState = "waiting" | "running" | "done" | "skipped" | "failed";

export interface JobView {
  path: string;
  root: string | null;
  state: JobState;
  progress: number;
  stage: string;
  outputs: string[];
  /** Times real time (audio length / time taken). */
  speed: number;
  note: string;
  reason: string;
}

export interface Run extends RunInfo {
  jobs: JobView[];
  reports: string[];
  /** Date.now() at the start. */
  startedAt: number;
  /** Seconds the run took, once it is over. */
  elapsed: number | null;
  /** The overall progress shown so far (it never drops). */
  maxOverall: number;
  /** Length of all files, for the encoding speed in the summary. */
  audioSecs: number;
}

export function newRun(info: RunInfo, items: [string, string | null][], audioSecs: number): Run {
  return {
    ...info,
    jobs: items.map(([path, root]) => ({
      path,
      root,
      state: "waiting",
      progress: 0,
      stage: "",
      outputs: [],
      speed: 0,
      note: "",
      reason: "",
    })),
    reports: [],
    startedAt: Date.now(),
    elapsed: null,
    maxOverall: 0,
    audioSecs,
  };
}

/** The share of the run that is done, never less than `prev`. */
export function overallProgress(prev: number, jobs: JobView[]): number {
  const now = jobs.reduce((sum, j) => sum + j.progress, 0) / Math.max(1, jobs.length);
  return Math.max(prev, now);
}

function withJob(run: Run, id: number, change: Partial<JobView>): Run {
  const job = run.jobs[id];
  if (!job) return run;
  const jobs = run.jobs.slice();
  jobs[id] = { ...job, ...change };
  return { ...run, jobs, maxOverall: overallProgress(run.maxOverall, jobs) };
}

export function applyEvent(run: Run, e: RunEvent): Run {
  switch (e.kind) {
    case "Started":
      return withJob(run, e.id, { state: "running" });
    case "Progress":
      return withJob(run, e.id, { progress: e.progress, stage: e.stage });
    case "Done":
      return withJob(run, e.id, {
        state: "done",
        progress: 1,
        outputs: e.outputs,
        speed: e.wall_secs > 0 ? e.audio_secs / e.wall_secs : 0,
        note: e.note,
      });
    case "Skipped":
      return withJob(run, e.id, { state: "skipped", progress: 1, reason: e.reason });
    case "Failed":
      return withJob(run, e.id, { state: "failed", progress: 1, reason: e.reason });
    case "Report":
      return { ...run, reports: [...run.reports, e.text] };
    case "Finished":
      return { ...run, elapsed: e.elapsed };
  }
}

/** "Fehlgeschlagene erneut umwandeln" is offered once the run is over and something failed. */
export function canRetry(run: Run): boolean {
  return run.elapsed !== null && run.jobs.some((j) => j.state === "failed");
}

export function failedItems(run: Run): [string, string | null][] {
  return run.jobs.filter((j) => j.state === "failed").map((j) => [j.path, j.root]);
}

/** An open "Dateien sind schon vorhanden" question (converter or batch page). */
export interface OverwriteAsk {
  items: [string, string | null][];
  codec: Codec;
  keepStructure: boolean;
  paths: string[];
}

export const useRun = create<{ run: Run | null; ask: OverwriteAsk | null }>(() => ({
  run: null,
  ask: null,
}));
