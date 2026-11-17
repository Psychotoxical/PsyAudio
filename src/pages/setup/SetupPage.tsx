import { Check, Disc3, HardDrive, MousePointerClick, Package, Palette } from "lucide-react";
import { type ReactNode, useState } from "react";
import { useTranslation } from "react-i18next";
import { FooterBar } from "../../shell/FooterBar";
import { PageHeader } from "../../shell/PageHeader";
import { useCd } from "../../store/cd";
import { codecInfo, useEncode } from "../../store/encode";
import { useSettings } from "../../store/settings";
import { type Page, useUi } from "../../store/ui";
import { Hint } from "../../ui/controls";
import { driveKey } from "../cd/driveProfile";
import { signed } from "../cd/measure";
import { RipFolderField } from "../cd/RipPanels";
import { CodecChips } from "../convert/CodecChips";
import { DriveRow, PendingQuestion, useAttachedDrives } from "../settings/DrivesBox";
import {
  IntegrationFields,
  LookFields,
  lookSummary,
  ProgramsFields,
  usePrograms,
  useSystemInfo,
} from "../settings/SettingsPage";
import "../settings/settings.css";
import "./setup.css";

const STEPS = 5;

/** The page to open at start: the setup until it was finished or left once. */
export function firstPage(setupDone: boolean): Page | null {
  return setupDone ? null : "setup";
}

function markSetupDone(): void {
  const s = useSettings.getState();
  if (s.settings && !s.settings.setup_done)
    s.update((x) => {
      x.setup_done = true;
    });
}

// leaving the setup by the sidebar counts as finished too: it stays one click away under "System"
useUi.subscribe((now, before) => {
  if (before.page === "setup" && now.page !== "setup") markSetupDone();
});

/** The last part of a path: "Musik" for "C:\Users\x\Musik". */
function folderName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? "";
}

/** A step's box: it does not fold, its head is the step's title. */
function StepBox({
  icon,
  title,
  head,
  children,
}: {
  icon: ReactNode;
  title: string;
  /** In place of the title heading, e.g. a drive's name and letter. */
  head?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className="box open" aria-label={title}>
      <div className="box-head static">
        {icon}
        {head ?? <h2>{title}</h2>}
      </div>
      <div className="box-body">{children}</div>
    </section>
  );
}

/** First start: language and look, programs, the file manager menu, rips, the CD drive. Every
 * step saves at once and can be skipped; "Done" on the last one. */
export function SetupPage() {
  const { t } = useTranslation();
  const s = useSettings((x) => x.settings);
  const options = useEncode((x) => x.options);
  const ffmpegSource = useEncode((x) => x.ffmpegSource);
  const measuring = useCd((x) => x.offset.busy);
  const info = useSystemInfo();
  const progs = usePrograms();
  const attached = useAttachedDrives();
  const [step, setStep] = useState(1);
  const [menu, setMenu] = useState<boolean | null>(null);
  if (!s) return null;

  const ffmpeg = progs.list.find((p) => p.key === "ffmpeg");
  const codec =
    s.rip_codec === "Multi"
      ? t("Several formats")
      : (codecInfo(options, s.rip_codec)?.name ?? s.rip_codec);
  const firstDrive = attached[0];
  const firstProfile = firstDrive
    ? s.rip.drives.find((p) => p.model === driveKey(firstDrive))
    : undefined;
  const tiles = [
    { title: t("Welcome"), summary: lookSummary(s.lang, s.dark) },
    {
      title: t("Programs"),
      summary: !ffmpeg
        ? "…"
        : !ffmpeg.found
          ? t("FFmpeg missing")
          : ffmpegSource === "bundled"
            ? t("FFmpeg bundled")
            : t("FFmpeg found"),
    },
    {
      title: t("Right-click"),
      summary:
        menu === null ? t("optional") : menu ? t("Context menu set up") : t("Context menu removed"),
    },
    {
      title: t("Rips"),
      summary: `${codec} · ${folderName(s.rip_folder) || t("no folder")}`,
    },
    {
      title: t("CD drive"),
      summary: measuring
        ? t("measuring …")
        : !firstDrive
          ? t("no drive")
          : firstProfile
            ? `${signed(firstProfile.offset)} Samples`
            : t("not measured yet"),
    },
  ];

  const finish = () => {
    markSetupDone();
    useUi.getState().setPage("convert");
  };
  const next = () => setStep((n) => Math.min(n + 1, STEPS));

  return (
    <div className="page setup">
      <div className="page-main">
        <PageHeader title={t("Setup")}>
          <span className="muted">
            {t("Step {0} of {1}", { 0: String(step), 1: String(STEPS) })}
          </span>
        </PageHeader>
        <nav className="steps" aria-label={t("Setup")}>
          {tiles.map((tile, i) => {
            const n = i + 1;
            return (
              <button
                key={tile.title}
                type="button"
                className={n === step ? "step on" : "step"}
                aria-pressed={n === step}
                onClick={() => setStep(n)}
              >
                <span className={n < step ? "step-n done" : "step-n"}>
                  {n < step ? <Check size={14} /> : n}
                </span>
                <span className="step-text">
                  <strong>{tile.title}</strong>
                  <small title={tile.summary}>{tile.summary}</small>
                </span>
              </button>
            );
          })}
        </nav>
        <div className="content settings">
          <div className="settings-col">
            {step === 1 && (
              <>
                <p className="setup-lead">
                  {t(
                    "Welcome to PsyAudio. PsyAudio converts audio files and rips CDs bit-exactly. Five short steps set everything up; you can skip any step and change it later under “Settings”.",
                  )}
                </p>
                <StepBox icon={<Palette size={16} />} title={t("Language and look")}>
                  <LookFields />
                </StepBox>
              </>
            )}
            {step === 2 && (
              <StepBox icon={<Package size={16} />} title={t("Encoder programs")}>
                <ProgramsFields {...progs} />
              </StepBox>
            )}
            {step === 3 && (
              <StepBox icon={<MousePointerClick size={16} />} title={t("Context menu")}>
                {info ? <IntegrationFields info={info} onDone={setMenu} /> : null}
              </StepBox>
            )}
            {step === 4 && (
              <StepBox icon={<Disc3 size={16} />} title={t("Format and folder of rips")}>
                <CodecChips field="rip_codec" />
                <RipFolderField />
              </StepBox>
            )}
            {step === 5 && (
              <>
                <p className="setup-lead">
                  {t(
                    "Every CD drive reads a tiny bit shifted. PsyAudio measures this once with a bought audio CD and corrects it in every rip. Whether a rip is bit-exact is checked by the open CUETools database (CTDB).",
                  )}
                </p>
                {attached.length === 0 && (
                  <StepBox icon={<HardDrive size={16} />} title={t("CD drive")}>
                    <Hint>{t("No CD drive found.")}</Hint>
                  </StepBox>
                )}
                {attached.map((d, i) => (
                  <StepBox
                    key={d.id}
                    icon={<HardDrive size={16} />}
                    title={driveKey(d)}
                    head={
                      <>
                        <span className="drive-name">{driveKey(d)}</span>
                        <small className="where">{d.name}</small>
                      </>
                    }
                  >
                    <DriveRow
                      drive={d}
                      profile={s.rip.drives.find((p) => p.model === driveKey(d))}
                      titled={false}
                    />
                    {i === attached.length - 1 && (
                      <>
                        <PendingQuestion />
                        <Hint>
                          {measuring
                            ? t(
                                "Takes about fifteen seconds. The CD must be a bought audio CD that is in the CUETools database – almost every one is.",
                              )
                            : t(
                                "No CD at hand? Just go on: PsyAudio then measures automatically the first time it reads a CD.",
                              )}
                        </Hint>
                      </>
                    )}
                  </StepBox>
                ))}
              </>
            )}
          </div>
        </div>
        <FooterBar summary={t("Everything can be changed later under “Settings”.")}>
          {step > 1 && (
            <button type="button" className="btn" onClick={() => setStep((n) => n - 1)}>
              {t("Back")}
            </button>
          )}
          {step < STEPS ? (
            <>
              <button type="button" className="btn" onClick={next}>
                {t("Skip")}
              </button>
              <button type="button" className="btn primary" onClick={next}>
                {t("Next")}
              </button>
            </>
          ) : (
            <button type="button" className="btn primary" disabled={measuring} onClick={finish}>
              {t("Done")}
            </button>
          )}
        </FooterBar>
      </div>
    </div>
  );
}
