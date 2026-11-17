import { open } from "@tauri-apps/plugin-dialog";
import { useId, useRef } from "react";
import { useTranslation } from "react-i18next";
import type { Effect } from "../../ipc/bindings/Effect";
import { effectKind, useEffects } from "../../store/effects";
import { Check, Hint, Segmented, Select } from "../../ui/controls";
import { SliderField } from "../../ui/SliderField";
import { useSystemInfo } from "../settings/SettingsPage";

const RATES = [8000, 11025, 16000, 22050, 32000, 44100, 48000, 88200, 96000, 176400, 192000];
const EQ_LABELS = ["60", "130", "260", "500", "1k", "2k", "4k", "8k", "16k"];
const EQ_PRESETS: [string, number[]][] = [
  ["To 0 dB", [0, 0, 0, 0, 0, 0, 0, 0, 0]],
  ["Concert hall", [4, 3, 1, 0, -1, 0, 2, 3, 4]],
  ["More bass", [8, 6, 4, 1, 0, 0, 0, 0, 0]],
  ["More treble", [0, 0, 0, 0, 0, 1, 4, 6, 8]],
];

/** What each kind of "Normalize volume" does, in the order of NORMALIZE_MODES. */
const NORMALIZE_HINTS = [
  "Raises or lowers each file until its loudest point reaches the target level. How loud the files sound can still differ.",
  "Evens out the perceived loudness (measured to EBU R128, as broadcasters and streaming services do) and changes the audio for good.",
  "Evens out the perceived loudness (measured to EBU R128, as broadcasters and streaming services do) and changes the audio for good.",
  "Keeps adjusting, also within a track: quiet passages get louder. For audiobooks and live recordings, not for music whose dynamics are intended.",
  "Keeps adjusting to the peaks, also within a track. For audiobooks and live recordings, not for music whose dynamics are intended.",
  "Changes every file by the same amount; raising too much distorts.",
  "Lowers only files whose loudest point is above the target level; quieter ones stay unchanged.",
];

/** Tags a value can take over, as `<name>` (the names of file naming). */
const PLACEHOLDERS: [string, string][] = [
  ["artist", "Artist"],
  ["albumartist", "Album artist"],
  ["album", "Album"],
  ["title", "Title"],
  ["year", "Year"],
  ["genre", "Genre"],
];

/** Fields Tag-Bearbeitung fills in by itself (see AUTO_FIELDS in the core). */
const AUTO_FIELDS = ["track", "track_total"];

/** An example value per field, as the placeholder of the value field. */
const FIELD_EXAMPLES: Record<string, string> = {
  title: "e.g. <title> (Remastered)",
  artist: "e.g. Band",
  album: "e.g. <album> (Deluxe)",
  "album artist": "e.g. <artist>",
  disc: "e.g. 1",
  disc_total: "e.g. 2",
  year: "e.g. 1994",
  genre: "e.g. Rock",
  composer: "e.g. Composer",
  conductor: "e.g. Conductor",
  comment: "e.g. Ripped with PsyAudio",
  label: "e.g. Label",
  isrc: "e.g. DEA123456789",
  compilation: "1 = compilation, 0 = no compilation",
  lyrics: "Lyrics",
};

// biome-ignore lint/suspicious/noExplicitAny: the fields differ per effect; each case knows its own
type Fields = Record<string, any>;

function TextField({
  label,
  value,
  placeholder,
  onChange,
  inputRef,
}: {
  label: string;
  value: string;
  placeholder?: string;
  onChange: (v: string) => void;
  inputRef?: React.Ref<HTMLInputElement>;
}) {
  const id = useId();
  return (
    <div className="field">
      <label className="lbl" htmlFor={id}>
        {label}
      </label>
      <input
        id={id}
        ref={inputRef}
        className="text-field"
        type="text"
        value={value}
        placeholder={placeholder}
        onChange={(e) => onChange(e.target.value)}
      />
    </div>
  );
}

/** The value of a Tag-Bearbeitung: text with buttons that insert other tags. */
function TagValue({
  field,
  value,
  onChange,
}: {
  field: string;
  value: string;
  onChange: (v: string) => void;
}) {
  const { t } = useTranslation();
  const ref = useRef<HTMLInputElement>(null);
  const insert = (name: string) => {
    const el = ref.current;
    const token = `<${name}>`;
    const at = el?.selectionStart ?? value.length;
    const end = el?.selectionEnd ?? at;
    onChange(value.slice(0, at) + token + value.slice(end));
    requestAnimationFrame(() => {
      el?.focus();
      el?.setSelectionRange(at + token.length, at + token.length);
    });
  };
  return (
    <>
      <TextField
        label={t("Value")}
        value={value}
        placeholder={t(FIELD_EXAMPLES[field] ?? "Text")}
        onChange={onChange}
        inputRef={ref}
      />
      <div className="chips">
        {PLACEHOLDERS.map(([key, label]) => (
          <button key={key} type="button" className="chip-btn" onClick={() => insert(key)}>
            {t(label)}
          </button>
        ))}
      </div>
      <Hint>
        {t(
          "Applies to every file. A button inserts the value of another tag; <artist>, for example, becomes the file's artist.",
        )}
      </Hint>
    </>
  );
}

/** The settings of one effect; every effect says in a hint what it does and when to use it. */
export function EffectEditor({
  effect,
  onChange,
}: {
  effect: Effect;
  onChange: (effect: Effect) => void;
}) {
  const { t } = useTranslation();
  const catalogue = useEffects((s) => s.catalogue);
  const os = useSystemInfo()?.os;
  const kind = effectKind(effect);
  const v = typeof effect === "string" ? {} : ((effect as Record<string, Fields>)[kind] ?? {});
  const set = (patch: Fields) => onChange({ [kind]: { ...v, ...patch } } as Effect);
  const ms = (label: string, key: string, max: number, step = 100) => (
    <SliderField
      label={t(label)}
      min={0}
      max={max}
      step={step}
      value={v[key]}
      unit="ms"
      onChange={(x) => set({ [key]: x })}
    />
  );

  switch (kind) {
    case "DeleteSource":
      return (
        <Hint warn>
          {t(
            "Moves the source file to the recycle bin once all its output files are written. If the conversion fails, it stays where it is.",
          )}
        </Hint>
      );
    case "PreserveAttributes":
      return (
        <Hint>{t("The new file gets the source's modification date instead of today's.")}</Hint>
      );
    case "FolderJpgPreserve":
      return (
        <Hint>
          {t(
            "A folder.jpg or cover.jpg in the source folder is copied to the output folder; an image already there is kept.",
          )}
        </Hint>
      );
    case "PlaySound":
      return <Hint>{t("A short sound when all files are done.")}</Hint>;
    case "DeEmphasis":
      return (
        <Hint warn>
          {t(
            "Only for CDs with pre-emphasis, mostly from the early 1980s: their treble was raised in mastering, and this effect lowers it again. Any other recording sounds dull afterwards.",
          )}
        </Hint>
      );
    case "Resample":
      return (
        <>
          <Select<number>
            label={t("Sample rate")}
            value={v.rate}
            options={RATES.map((r) => [r, `${r} Hz`] as [number, string])}
            onChange={(x) => set({ rate: x })}
          />
          <Hint>{t("44100 Hz is CD quality, 48000 Hz the standard for video.")}</Hint>
        </>
      );
    case "BitDepth":
      return (
        <>
          <Segmented<number>
            label={t("Bit depth")}
            value={v.bits}
            options={[8, 16, 24, 32].map((b) => [b, `${b} Bit`] as [number, string])}
            onChange={(x) => set({ bits: x })}
          />
          <Check
            label={t("Dither (recommended when reducing to 16 bit)")}
            checked={v.dither}
            onChange={(x) => set({ dither: x })}
          />
          <Hint>
            {t(
              "16 bit is CD quality. When reducing, dither adds inaudible noise so quiet passages don't distort.",
            )}
          </Hint>
        </>
      );
    case "ChannelCount":
      return (
        <>
          <Segmented<number>
            label={t("Channels")}
            value={v.channels}
            options={[
              [1, "Mono"] as [number, string],
              [2, "Stereo"] as [number, string],
              // 5.1 only kept for old profiles: it put stereo on the front speakers, nothing more
              ...(v.channels === 6 ? [[6, "5.1"] as [number, string]] : []),
            ]}
            onChange={(x) => set({ channels: x })}
          />
          <Hint>
            {t("Mono mixes both sides into one; stereo turns mono into two identical sides.")}
          </Hint>
        </>
      );
    case "ChannelMapper":
      return (
        <Select<number>
          label={t("Mapping")}
          value={v.mode}
          options={[
            [0, t("Swap left/right")],
            [1, t("Left on both sides")],
            [2, t("Right on both sides")],
          ]}
          onChange={(x) => set({ mode: x })}
        />
      );
    case "VolumeNormalize": {
      const mode = v.mode as number;
      const offset = catalogue?.ebu_offset ?? 18;
      return (
        <>
          <Select<number>
            label={t("Kind")}
            value={mode}
            options={(catalogue?.normalize_modes ?? [])
              .map((n, i) => [i, t(n)] as [number, string])
              // "ReplayGain (track)" measures like "EBU R128 (track)": shown only for old profiles
              .filter(([i]) => i !== 1 || mode === 1)}
            onChange={(x) => set({ mode: x })}
          />
          <Hint>{t(NORMALIZE_HINTS[mode] ?? "")}</Hint>
          {mode === 5 ? (
            <SliderField
              label={t("Gain")}
              min={-12}
              max={12}
              step={0.1}
              value={v.fixed_db}
              unit="dB"
              onChange={(x) => set({ fixed_db: x })}
            />
          ) : mode >= 1 && mode <= 3 ? (
            <SliderField
              label={t("Target loudness")}
              min={-38}
              max={-8}
              step={0.5}
              value={v.target_db - offset}
              unit="LUFS"
              onChange={(x) => set({ target_db: x + offset })}
            />
          ) : (
            <SliderField
              label={t("Target level of the loudest point")}
              min={-20}
              max={0}
              step={0.1}
              value={v.target_db}
              unit="dB"
              onChange={(x) => set({ target_db: x })}
            />
          )}
          {mode !== 5 && mode !== 6 && (
            <SliderField
              label={t("Raise by at most")}
              min={1}
              max={20}
              step={0.5}
              value={v.max_amp}
              format={(x) => `× ${x} (+${(20 * Math.log10(x)).toFixed(0)} dB)`}
              onChange={(x) => set({ max_amp: x })}
            />
          )}
          {(mode === 3 || mode === 4) && (
            <>
              {ms("Ausgleichsfenster", "window_ms", 30000)}
              <Hint>
                {t("PsyAudio averages the volume over this span; shorter reacts faster.")}
              </Hint>
            </>
          )}
        </>
      );
    }
    case "ReplayGain":
      return (
        <>
          <SliderField
            label={t("Target loudness")}
            min={-30}
            max={-5}
            step={0.5}
            value={v.target_lufs}
            unit="LUFS"
            onChange={(x) => set({ target_lufs: x })}
          />
          <SliderField
            label={t("Raise by at most")}
            min={0}
            max={40}
            step={0.5}
            value={v.max_gain}
            unit="dB"
            onChange={(x) => set({ max_gain: x })}
          />
          <Check
            label={t("Also calculate album gain")}
            checked={v.album}
            onChange={(x) => set({ album: x })}
          />
          <Check
            label={t("Never raise so far that it clips")}
            checked={v.clip_prevention}
            onChange={(x) => set({ clip_prevention: x })}
          />
          <Check
            label={t("Measure peaks more precisely (true peak, slower)")}
            checked={v.true_peak_ratio > 1}
            onChange={(x) => set({ true_peak_ratio: x ? 4 : 1 })}
          />
          <Hint>
            {t(
              "The audio stays unchanged; players with ReplayGain then play every track equally loud. −18 LUFS is the usual value. Album gain keeps the differences in volume within an album.",
            )}
          </Hint>
        </>
      );
    case "ReplayGainApply":
      return (
        <>
          <Segmented<boolean>
            label={t("Value")}
            value={v.album}
            options={[
              [false, t("Track gain")],
              [true, t("Album gain")],
            ]}
            onChange={(x) => set({ album: x })}
          />
          <Check
            label={t("Only raise as far as nothing clips")}
            checked={v.limit_peaks}
            onChange={(x) => set({ limit_peaks: x })}
          />
          <SliderField
            label={t("Change files without ReplayGain by")}
            min={-40}
            max={0}
            step={0.5}
            value={v.missing_db}
            unit="dB"
            onChange={(x) => set({ missing_db: x })}
          />
          <Hint>
            {t(
              "Applies the ReplayGain values from the tags to the audio permanently, for players without ReplayGain. Files without values change by the amount set; 0 dB leaves them as they are.",
            )}
          </Hint>
        </>
      );
    case "Fade":
      return (
        <>
          {ms("Fade in", "in_ms", 20000)}
          {ms("Fade out (from the end)", "out_ms", 20000)}
          <Hint>{t("0 ms turns that fade off.")}</Hint>
        </>
      );
    case "Trim":
      return (
        <>
          {ms("From the start", "start_ms", 120000)}
          {ms("From the end", "end_ms", 120000)}
          <Hint>{t("Cuts the same time off every file; 1000 ms = 1 second.")}</Hint>
        </>
      );
    case "TrimSilence":
      return (
        <>
          <Check
            label={t("Remove silence at the start")}
            checked={v.begin}
            onChange={(x) => set({ begin: x })}
          />
          <Check
            label={t("Remove silence at the end")}
            checked={v.end}
            onChange={(x) => set({ end: x })}
          />
          <Check
            label={t("Only complete silence (digital zero)")}
            checked={v.digital_only}
            onChange={(x) => set({ digital_only: x })}
          />
          <SliderField
            label={t("Counts as silence below")}
            min={-96}
            max={-20}
            value={v.threshold_db}
            unit="dB"
            disabled={v.digital_only}
            onChange={(x) => set({ threshold_db: x })}
          />
          <Hint>
            {t("−60 dB suits most recordings; for noisy records a little higher, about −45 dB.")}
          </Hint>
        </>
      );
    case "Lowpass":
    case "Highpass":
      return (
        <>
          <SliderField
            label={t("Cutoff frequency")}
            log
            min={10}
            max={22000}
            value={v.hz}
            unit="Hz"
            onChange={(x) => set({ hz: x })}
          />
          <Hint>
            {kind === "Highpass"
              ? t("Removes everything below the limit, such as turntable rumble: 20 to 40 Hz.")
              : t("Removes everything above the limit, such as hiss: 12000 to 16000 Hz.")}
          </Hint>
        </>
      );
    case "GraphicEq": {
      const gains = v.gains as number[];
      return (
        <>
          <div className="eq">
            {gains.map((g, i) => (
              <label key={EQ_LABELS[i]} className="eq-band">
                <span className="eq-val">{g > 0 ? `+${g}` : g}</span>
                <input
                  type="range"
                  aria-label={`${EQ_LABELS[i]} Hz`}
                  min={-12}
                  max={12}
                  step={0.5}
                  value={g}
                  onChange={(e) =>
                    set({ gains: gains.map((x, j) => (j === i ? Number(e.target.value) : x)) })
                  }
                />
                <span className="eq-hz">{EQ_LABELS[i]}</span>
              </label>
            ))}
          </div>
          <div className="chips">
            {EQ_PRESETS.map(([name, preset]) => (
              <button
                key={name}
                type="button"
                className="chip-btn"
                onClick={() => set({ gains: [...preset] })}
              >
                {t(name)}
              </button>
            ))}
          </div>
          <Hint>{t("Values in dB; a click on a preset sets all bands.")}</Hint>
        </>
      );
    }
    case "Compressor":
      return (
        <>
          <SliderField
            label={t("Threshold")}
            min={-40}
            max={0}
            step={0.5}
            value={v.threshold_db}
            unit="dB"
            onChange={(x) => set({ threshold_db: x })}
          />
          <SliderField
            label={t("Strength")}
            min={1}
            max={20}
            step={0.5}
            value={v.ratio}
            format={(x) => `${x}:1`}
            onChange={(x) => set({ ratio: x })}
          />
          <SliderField
            label={t("Attack time")}
            min={0.1}
            max={200}
            step={0.1}
            value={v.attack_ms}
            unit="ms"
            onChange={(x) => set({ attack_ms: x })}
          />
          <SliderField
            label={t("Release time")}
            min={10}
            max={5000}
            step={10}
            value={v.release_ms}
            unit="ms"
            onChange={(x) => set({ release_ms: x })}
          />
          <Hint>
            {t(
              'Everything louder than the threshold is turned down: at 3:1, 3 dB above it comes out as 1 dB. The attack time sets how fast this sets in, the release time how fast it lets go. Afterwards everything is quieter; with "Normalize volume" after it, the quiet passages get louder. Good for audiobooks and live recordings with big jumps in volume.',
            )}
          </Hint>
        </>
      );
    case "Hdcd":
      return (
        <>
          <Check
            label={t("Raise by 6 dB")}
            checked={v.amp_6db}
            onChange={(x) => set({ amp_6db: x })}
          />
          <Hint>
            {t(
              "Detects HDCDs (20-bit audio CDs) by itself and turns them into 24 bit. Normal CDs stay unchanged at 16 bit.",
            )}
          </Hint>
          <Hint>
            {t(
              "Decoded HDCDs are 6 dB quieter than normal CDs to make room for their extended peaks. Raising makes up for that; those peaks can then clip.",
            )}
          </Hint>
        </>
      );
    case "PlaylistWriter":
      return (
        <>
          <Segmented<number>
            label={t("Format")}
            value={v.kind}
            options={[
              [0, "M3U8"],
              [1, "M3U"],
              [2, "PLS"],
            ]}
            onChange={(x) => set({ kind: x })}
          />
          <Segmented<boolean>
            label={t("Location")}
            value={v.same_folder}
            options={[
              [true, t("One per folder")],
              [false, t("One in the output folder")],
            ]}
            onChange={(x) => set({ same_folder: x })}
          />
          <Hint>
            {t(
              'The playlist is named after its folder, such as "Album.m3u8", and lists the new files in list order. Almost every player understands M3U8.',
            )}
          </Hint>
        </>
      );
    case "RunExternal":
      return (
        <>
          <TextField
            label={t("Command")}
            value={v.command}
            placeholder={
              os === "windows"
                ? t("e.g. my-script.bat [outfile]")
                : t("e.g. ~/bin/my-script.sh [outfile]")
            }
            onChange={(x) => set({ command: x })}
          />
          <Hint>
            {t(
              "Runs after each finished file, through cmd on Windows and sh on Linux and macOS; PsyAudio waits until the command has finished. [outfile] is the new file, [infile] the source; PsyAudio adds the quotes itself.",
            )}
          </Hint>
        </>
      );
    case "ConditionalEncoding":
      return (
        <>
          <TextField
            label={t("Sources with the extension")}
            value={v.skip_ext}
            placeholder="mp3"
            onChange={(x) => set({ skip_ext: x })}
          />
          <Segmented<boolean>
            value={v.copy_instead}
            options={[
              [false, t("skip")],
              [true, t("copy unchanged")],
            ]}
            onChange={(x) => set({ copy_instead: x })}
          />
          <Hint>
            {t(
              "One extension, without the dot. Example: convert a mixed folder to MP3 but only copy the MP3s already there instead of encoding them lossily a second time.",
            )}
          </Hint>
        </>
      );
    case "MoveDestOnError":
      return (
        <>
          <div className="field">
            <p className="lbl">{t("Folder")}</p>
            <div className="row">
              <input
                className="text-field path-field"
                type="text"
                readOnly
                aria-label={t("Folder")}
                value={v.folder}
                placeholder={t("Choose folder …")}
              />
              <button
                type="button"
                className="btn"
                onClick={async () => {
                  const picked = await open({
                    directory: true,
                    defaultPath: v.folder || undefined,
                  });
                  if (typeof picked === "string") set({ folder: picked });
                }}
              >
                {t("Change")}
              </button>
            </div>
          </div>
          <Hint>
            {t(
              "If a conversion fails, the half-written file is moved here so you can inspect it. Without this effect PsyAudio deletes it.",
            )}
          </Hint>
        </>
      );
    case "TagProcessing": {
      const remove = v.mode === 1;
      const auto = AUTO_FIELDS.includes(v.field);
      const fields = (catalogue?.tag_fields ?? []).map(([k, n]) => [k, t(n)] as [string, string]);
      const options = remove
        ? [
            ...fields,
            ["cover", t("Cover picture")] as [string, string],
            ["*", t("All tags")] as [string, string],
          ]
        : fields;
      return (
        <>
          <Segmented<number>
            value={auto && v.mode === 2 ? 0 : v.mode}
            options={[
              [0, t("Set")] as [number, string],
              // numbering only some files would give gaps and doubles
              ...(auto ? [] : [[2, t("Set if empty")] as [number, string]]),
              [1, t("Remove")] as [number, string],
            ]}
            onChange={(x) =>
              set(
                x !== 1 && (v.field === "*" || v.field === "cover")
                  ? { mode: x, field: "comment" }
                  : { mode: x },
              )
            }
          />
          <Select<string>
            label={t("Field")}
            value={v.field}
            options={options}
            onChange={(x) =>
              set(AUTO_FIELDS.includes(x) && v.mode === 2 ? { field: x, mode: 0 } : { field: x })
            }
          />
          {!remove &&
            (auto ? (
              <Hint>
                {v.field === "track"
                  ? t(
                      "Filled in automatically: 1, 2, 3 … per folder, in the order of the file list.",
                    )
                  : t("Filled in automatically: the number of files from this folder in the list.")}
              </Hint>
            ) : (
              <TagValue field={v.field} value={v.value} onChange={(x) => set({ value: x })} />
            ))}
        </>
      );
    }
    default:
      return <Hint>{t("No settings.")}</Hint>;
  }
}
