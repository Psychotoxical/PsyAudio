import { useVirtualizer } from "@tanstack/react-virtual";
import {
  CircleCheck,
  CircleDashed,
  CircleX,
  FolderOpen,
  Play,
  Square,
  TriangleAlert,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { encodingSpeed, fmtLen, longDuration } from "../../format";
import { plural } from "../../i18n";
import { revealFolder } from "../../ipc/files";
import { cancelConversion } from "../../ipc/run";
import { canRetry, type JobView, type Run, useRun } from "../../store/run";
import { useSettings } from "../../store/settings";
import { closeRun, retryFailed } from "./runFlow";

const ROW = 52;

function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

function JobRow({ job }: { job: JobView }) {
  const { t, i18n } = useTranslation();
  const icon = {
    waiting: <CircleDashed size={16} className="st-wait" />,
    running: <Play size={16} className="st-run" />,
    done: <CircleCheck size={16} className="st-ok" />,
    skipped: <TriangleAlert size={16} className="st-warn" />,
    failed: <CircleX size={16} className="st-bad" />,
  }[job.state];
  let detail = "";
  if (job.state === "waiting") detail = t("waiting");
  else if (job.state === "running") detail = job.stage;
  else if (job.state === "skipped") detail = t("skipped: {r}", { r: job.reason });
  else if (job.state === "failed") detail = t("Error: {e}", { e: job.reason });
  else {
    const speed =
      job.speed > 0
        ? t("{0}× real time", {
            0: job.speed.toLocaleString(i18n.language === "de" ? "de-DE" : "en-US", {
              maximumFractionDigits: 1,
              minimumFractionDigits: 1,
            }),
          })
        : "";
    detail = [speed, job.outputs[0] ? `» ${job.outputs[0]}` : "", job.note]
      .filter(Boolean)
      .join("  ");
  }
  return (
    <div className={`job ${job.state}`}>
      {icon}
      <div className="job-text">
        <strong>{fileName(job.path)}</strong>
        <small title={detail}>{detail}</small>
      </div>
      {job.state === "running" && (
        <span className="job-bar">
          <i style={{ width: `${Math.round(job.progress * 100)}%` }} />
        </span>
      )}
    </div>
  );
}

function JobList({ jobs }: { jobs: JobView[] }) {
  const scroller = useRef<HTMLDivElement>(null);
  const list = useVirtualizer({
    count: jobs.length,
    getScrollElement: () => scroller.current,
    estimateSize: () => ROW,
    overscan: 8,
  });
  return (
    <div className="jobs" ref={scroller}>
      <div style={{ height: list.getTotalSize(), position: "relative" }}>
        {list.getVirtualItems().map((v) => {
          const job = jobs[v.index];
          return job ? (
            <div
              key={v.key}
              style={{
                position: "absolute",
                top: 0,
                left: 0,
                right: 0,
                height: ROW,
                transform: `translateY(${v.start}px)`,
              }}
            >
              <JobRow job={job} />
            </div>
          ) : null;
        })}
      </div>
    </div>
  );
}

/** Seconds since the start, ticking once a second while the run lasts. */
function useElapsed(run: Run): number {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    if (run.elapsed !== null) return;
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, [run.elapsed]);
  return run.elapsed ?? (now - run.startedAt) / 1000;
}

/** The running conversion and, once over, its summary. */
export function RunView({ run }: { run: Run }) {
  const { t, i18n } = useTranslation();
  const lang = useSettings((s) => s.settings?.lang ?? "en");
  const elapsed = useElapsed(run);
  const total = run.jobs.length;
  const count = (s: JobView["state"]) => run.jobs.filter((j) => j.state === s).length;
  const done = count("done");
  const failed = count("failed");
  const skipped = count("skipped");
  const finished = run.elapsed !== null;
  const overall = finished ? 1 : run.maxOverall;

  let heading = run.title;
  let sub: string;
  if (finished) {
    heading = t(plural(done, "Converted {0} file to {1}", "Converted {0} files to {1}"), {
      0: String(done),
      1: run.target,
    });
    sub = t(
      plural(
        failed,
        "Finished in {0} · {done} done · {skipped} skipped · 1 error",
        "Finished in {0} · {done} done · {skipped} skipped · {failed} errors",
      ),
      {
        0: longDuration(elapsed, true, t),
        done: String(done),
        skipped: String(skipped),
        failed: String(failed),
      },
    );
    if (run.audioSecs > 0 && done > 0)
      sub += ` · ${encodingSpeed(run.audioSecs, elapsed, t, lang)}`;
  } else {
    const current = Math.min(
      Math.max(1, run.jobs.filter((j) => j.state !== "waiting").length),
      Math.max(1, total),
    );
    sub = t("File {current} of {total}", { current: String(current), total: String(total) });
    if (total >= 2)
      sub += t(plural(run.threads, " · {0} core", " · {0} cores"), { 0: String(run.threads) });
    sub += t(" · {0} elapsed", { 0: fmtLen(elapsed) });
    if (elapsed > 4 && overall > 0)
      sub += t(" · about {0} left", { 0: fmtLen(Math.max(0, elapsed / overall - elapsed)) });
  }
  const firstOutput = run.jobs.flatMap((j) => j.outputs)[0];

  return (
    <div className="page-main run">
      <header className="run-head">
        <h1>{heading || t("Converting …")}</h1>
        <p>{sub}</p>
        <div
          className="run-bar"
          role="progressbar"
          aria-label={t("Progress")}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={Math.round(overall * 100)}
        >
          <i style={{ width: `${overall * 100}%` }} />
          <span>{`${Math.round(overall * 100).toLocaleString(i18n.language)} %`}</span>
        </div>
        <div className="run-actions">
          {finished ? (
            <>
              <button type="button" className="btn primary" onClick={closeRun}>
                {t("Done")}
              </button>
              {canRetry(run) && (
                <button type="button" className="btn" onClick={() => void retryFailed(t)}>
                  {t("Convert failed files again")}
                </button>
              )}
              {firstOutput && (
                <button
                  type="button"
                  className="btn"
                  onClick={() => void revealFolder(firstOutput)}
                >
                  <FolderOpen size={16} />
                  {t("Open output folder")}
                </button>
              )}
            </>
          ) : (
            <button type="button" className="btn" onClick={() => void cancelConversion()}>
              <Square size={14} />
              {t("Cancel")}
            </button>
          )}
        </div>
      </header>
      <JobList jobs={run.jobs} />
      {run.reports.length > 0 && (
        <section className="reports" aria-label={t("Results")}>
          <h2>{t("Results")}</h2>
          <pre>{run.reports.join("\n")}</pre>
        </section>
      )}
    </div>
  );
}

/** The run of the converter page, if there is one. */
export function useConversion(): Run | null {
  return useRun((s) => s.run);
}
