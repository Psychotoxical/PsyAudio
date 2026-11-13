import { open } from "@tauri-apps/plugin-dialog";
import { useId } from "react";
import { useTranslation } from "react-i18next";
import type { Effect } from "../../ipc/bindings/Effect";
import { effectKind, useEffects } from "../../store/effects";
import { Check, Hint, Segmented, Select } from "../../ui/controls";
import { SliderField } from "../../ui/SliderField";

const RATES = [8000, 11025, 16000, 22050, 32000, 44100, 48000, 88200, 96000, 176400, 192000];
const EQ_LABELS = ["60", "130", "260", "500", "1k", "2k", "4k", "8k", "16k"];
const EQ_PRESETS: [string, number[]][] = [
  ["Auf 0 dB", [0, 0, 0, 0, 0, 0, 0, 0, 0]],
  ["Konzertsaal", [4, 3, 1, 0, -1, 0, 2, 3, 4]],
  ["Mehr Bass", [8, 6, 4, 1, 0, 0, 0, 0, 0]],
  ["Mehr Höhen", [0, 0, 0, 0, 0, 1, 4, 6, 8]],
];

// biome-ignore lint/suspicious/noExplicitAny: the fields differ per effect; each case knows its own
type Fields = Record<string, any>;

function TextField({
  label,
  value,
  placeholder,
  onChange,
}: {
  label: string;
  value: string;
  placeholder?: string;
  onChange: (v: string) => void;
}) {
  const id = useId();
  return (
    <div className="field">
      <label className="lbl" htmlFor={id}>
        {label}
      </label>
      <input
        id={id}
        className="text-field"
        type="text"
        value={value}
        placeholder={placeholder}
        onChange={(e) => onChange(e.target.value)}
      />
    </div>
  );
}

/** The settings of one effect, as in the egui version; units sit in the value fields. */
export function EffectEditor({
  effect,
  onChange,
}: {
  effect: Effect;
  onChange: (effect: Effect) => void;
}) {
  const { t } = useTranslation();
  const catalogue = useEffects((s) => s.catalogue);
  const kind = effectKind(effect);
  if (typeof effect === "string") return <Hint>{t("Keine Einstellungen.")}</Hint>;
  const v = (effect as Record<string, Fields>)[kind] ?? {};
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
    case "Resample":
      return (
        <Select<number>
          label={t("Abtastrate")}
          value={v.rate}
          options={RATES.map((r) => [r, `${r} Hz`] as [number, string])}
          onChange={(x) => set({ rate: x })}
        />
      );
    case "BitDepth":
      return (
        <>
          <Segmented<number>
            label={t("Bittiefe")}
            value={v.bits}
            options={[8, 16, 24, 32].map((b) => [b, `${b} Bit`] as [number, string])}
            onChange={(x) => set({ bits: x })}
          />
          <Check
            label={t("Dither (empfohlen beim Reduzieren auf 16 Bit)")}
            checked={v.dither}
            onChange={(x) => set({ dither: x })}
          />
        </>
      );
    case "ChannelCount":
      return (
        <Segmented<number>
          label={t("Kanäle")}
          value={v.channels}
          options={[
            [1, "Mono"],
            [2, "Stereo"],
            [6, "5.1"],
          ]}
          onChange={(x) => set({ channels: x })}
        />
      );
    case "ChannelMapper":
      return (
        <Select<number>
          label={t("Zuordnung")}
          value={v.mode}
          options={[
            [0, t("Links/Rechts tauschen")],
            [1, t("Nur links")],
            [2, t("Nur rechts")],
          ]}
          onChange={(x) => set({ mode: x })}
        />
      );
    case "VolumeNormalize": {
      const mode = v.mode as number;
      return (
        <>
          <Select<number>
            label={t("Art")}
            value={mode}
            options={(catalogue?.normalize_modes ?? []).map(
              (n, i) => [i, t(n)] as [number, string],
            )}
            onChange={(x) => set({ mode: x })}
          />
          {mode === 5 ? (
            <SliderField
              label={t("Verstärkung")}
              min={-12}
              max={12}
              step={0.1}
              value={v.fixed_db}
              unit="dB"
              onChange={(x) => set({ fixed_db: x })}
            />
          ) : mode >= 1 && mode <= 3 ? (
            <div className="field">
              <SliderField
                label={t("Zielpegel")}
                min={-20}
                max={10}
                step={0.5}
                value={v.target_db}
                unit="dB"
                onChange={(x) => set({ target_db: x })}
              />
              <Hint>{`= ${(v.target_db - (catalogue?.ebu_offset ?? 18)).toFixed(1)} LUFS`}</Hint>
            </div>
          ) : (
            <SliderField
              label={t("Zielpegel (Spitze)")}
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
              label={t("Max. Verstärkung")}
              min={1}
              max={20}
              step={0.5}
              value={v.max_amp}
              format={(x) => `× ${x}`}
              onChange={(x) => set({ max_amp: x })}
            />
          )}
          {(mode === 3 || mode === 4) && ms("Fensterlänge", "window_ms", 30000)}
        </>
      );
    }
    case "ReplayGain":
      return (
        <>
          <SliderField
            label={t("Ziel")}
            min={-30}
            max={-5}
            step={0.5}
            value={v.target_lufs}
            unit="LUFS"
            onChange={(x) => set({ target_lufs: x })}
          />
          <SliderField
            label={t("Max. Verstärkung")}
            min={0}
            max={40}
            step={0.5}
            value={v.max_gain}
            unit="dB"
            onChange={(x) => set({ max_gain: x })}
          />
          <Check
            label={t("Auch Album-Gain berechnen")}
            checked={v.album}
            onChange={(x) => set({ album: x })}
          />
          <Check
            label={t("Übersteuerung verhindern")}
            checked={v.clip_prevention}
            onChange={(x) => set({ clip_prevention: x })}
          />
          <SliderField
            label={t("True-Peak-Überabtastung")}
            min={1}
            max={16}
            value={v.true_peak_ratio}
            format={(x) => `× ${x}`}
            onChange={(x) => set({ true_peak_ratio: x })}
          />
        </>
      );
    case "ReplayGainApply":
      return (
        <>
          <Segmented<boolean>
            label={t("Wert")}
            value={v.album}
            options={[
              [false, t("Titel-Gain")],
              [true, t("Album-Gain")],
            ]}
            onChange={(x) => set({ album: x })}
          />
          <Check
            label={t("Spitzen begrenzen")}
            checked={v.limit_peaks}
            onChange={(x) => set({ limit_peaks: x })}
          />
          <SliderField
            label={t("Wenn ReplayGain fehlt")}
            min={-40}
            max={0}
            step={0.5}
            value={v.missing_db}
            unit="dB"
            onChange={(x) => set({ missing_db: x })}
          />
        </>
      );
    case "Fade":
      return (
        <>
          {ms("Einblenden", "in_ms", 20000)}
          {ms("Ausblenden (vom Ende)", "out_ms", 20000)}
        </>
      );
    case "Trim":
      return (
        <>
          {ms("Vom Anfang", "start_ms", 120000)}
          {ms("Vom Ende", "end_ms", 120000)}
        </>
      );
    case "TrimSilence":
      return (
        <>
          <Check
            label={t("Stille am Anfang entfernen")}
            checked={v.begin}
            onChange={(x) => set({ begin: x })}
          />
          <Check
            label={t("Stille am Ende entfernen")}
            checked={v.end}
            onChange={(x) => set({ end: x })}
          />
          <Check
            label={t("Nur digitale Stille (Nullen)")}
            checked={v.digital_only}
            onChange={(x) => set({ digital_only: x })}
          />
          <SliderField
            label={t("Schwelle")}
            min={-96}
            max={-20}
            value={v.threshold_db}
            unit="dB"
            disabled={v.digital_only}
            onChange={(x) => set({ threshold_db: x })}
          />
        </>
      );
    case "WriteSilence":
      return (
        <>
          {ms("Vorne", "start_ms", 60000)}
          {ms("Hinten", "end_ms", 60000)}
        </>
      );
    case "Loop":
      return (
        <SliderField
          label={t("Wiederholungen")}
          min={1}
          max={50}
          value={v.times}
          format={(x) => `× ${x}`}
          onChange={(x) => set({ times: x })}
        />
      );
    case "Speed":
      return (
        <SliderField
          label={t("Geschwindigkeit")}
          min={25}
          max={400}
          value={v.percent}
          unit="%"
          onChange={(x) => set({ percent: x })}
        />
      );
    case "MinimumLength":
      return (
        <SliderField
          label={t("Mindestlänge")}
          min={1}
          max={600}
          value={v.seconds}
          unit="s"
          onChange={(x) => set({ seconds: x })}
        />
      );
    case "MaximumLength":
      return (
        <>
          <SliderField
            label={t("Höchstlänge")}
            min={0}
            max={3600}
            value={v.seconds}
            unit="s"
            onChange={(x) => set({ seconds: x })}
          />
          <SliderField
            label={t("Anteil des Originals")}
            min={1}
            max={100}
            value={v.percent}
            unit="%"
            disabled={v.seconds !== 0}
            onChange={(x) => set({ percent: x })}
          />
          <Hint>{t("Bei 0 s gilt der Anteil in Prozent.")}</Hint>
        </>
      );
    case "Lowpass":
    case "Highpass":
      return (
        <SliderField
          label={t("Grenzfrequenz")}
          log
          min={10}
          max={22000}
          value={v.hz}
          unit="Hz"
          onChange={(x) => set({ hz: x })}
        />
      );
    case "Bandpass":
      return (
        <>
          <SliderField
            label={t("Untere Grenze")}
            log
            min={10}
            max={22000}
            value={v.low}
            unit="Hz"
            onChange={(x) => set({ low: x })}
          />
          <SliderField
            label={t("Obere Grenze")}
            log
            min={10}
            max={22000}
            value={v.high}
            unit="Hz"
            onChange={(x) => set({ high: x })}
          />
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
          <Hint>{t("Werte in dB; ein Klick auf eine Vorlage setzt alle Bänder.")}</Hint>
        </>
      );
    }
    case "Compressor":
      return (
        <>
          <SliderField
            label={t("Schwelle")}
            min={-40}
            max={0}
            step={0.5}
            value={v.threshold_db}
            unit="dB"
            onChange={(x) => set({ threshold_db: x })}
          />
          <SliderField
            label={t("Verhältnis")}
            min={1}
            max={20}
            step={0.5}
            value={v.ratio}
            format={(x) => `1:${x}`}
            onChange={(x) => set({ ratio: x })}
          />
          <SliderField
            label={t("Ansprechzeit")}
            min={0.1}
            max={200}
            step={0.1}
            value={v.attack_ms}
            unit="ms"
            onChange={(x) => set({ attack_ms: x })}
          />
          <SliderField
            label={t("Abklingzeit")}
            min={10}
            max={5000}
            step={10}
            value={v.release_ms}
            unit="ms"
            onChange={(x) => set({ release_ms: x })}
          />
        </>
      );
    case "Karaoke":
      return (
        <>
          <Segmented<boolean>
            value={v.keep_voice}
            options={[
              [false, t("Stimme entfernen")],
              [true, t("Instrumente entfernen")],
            ]}
            onChange={(x) => set({ keep_voice: x })}
          />
          <Hint>{t("Funktioniert nur mit Stereo-Dateien.")}</Hint>
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
          <Check
            label={t("Im selben Ordner wie die Titel")}
            checked={v.same_folder}
            onChange={(x) => set({ same_folder: x })}
          />
        </>
      );
    case "RunExternal":
      return (
        <>
          <TextField
            label={t("Befehl")}
            value={v.command}
            placeholder={t("z. B. notepad [outfile]")}
            onChange={(x) => set({ command: x })}
          />
          <Hint>{t("[outfile] = neue Datei, [infile] = Quelle")}</Hint>
        </>
      );
    case "ConditionalEncoding":
      return (
        <>
          <TextField
            label={t("Wenn Quelle Endung hat:")}
            value={v.skip_ext}
            onChange={(x) => set({ skip_ext: x })}
          />
          <Segmented<boolean>
            value={v.copy_instead}
            options={[
              [false, t("Datei überspringen")],
              [true, t("1:1 kopieren")],
            ]}
            onChange={(x) => set({ copy_instead: x })}
          />
        </>
      );
    case "MoveDestOnError":
      return (
        <div className="field">
          <p className="lbl">{t("Ordner")}</p>
          <div className="row">
            <input
              className="text-field path-field"
              type="text"
              readOnly
              aria-label={t("Ordner")}
              value={v.folder}
              placeholder={t("Ordner wählen …")}
            />
            <button
              type="button"
              className="btn"
              onClick={async () => {
                const picked = await open({ directory: true, defaultPath: v.folder || undefined });
                if (typeof picked === "string") set({ folder: picked });
              }}
            >
              {t("Ändern")}
            </button>
          </div>
        </div>
      );
    case "TagProcessing":
      return (
        <>
          <Segmented<number>
            value={v.mode}
            options={[
              [0, t("Setzen")],
              [2, t("Setzen, wenn leer")],
              [1, t("Entfernen")],
            ]}
            onChange={(x) => set({ mode: x })}
          />
          <Select<string>
            label={t("Feld")}
            value={v.field}
            options={[
              ...(catalogue?.tag_fields ?? []).map(([k, n]) => [k, t(n)] as [string, string]),
              ["cover", t("Cover-Bild")] as [string, string],
              ["*", t("Alle Tags")] as [string, string],
            ]}
            onChange={(x) => set({ field: x })}
          />
          {v.mode !== 1 && (
            <TextField label={t("Wert")} value={v.value} onChange={(x) => set({ value: x })} />
          )}
        </>
      );
    case "Hdcd":
      return (
        <>
          <Check
            label={t("Um 6 dB anheben")}
            checked={v.amp_6db}
            onChange={(x) => set({ amp_6db: x })}
          />
          <Hint>
            {t(
              "Erkennt HDCDs (20-Bit-Audio-CDs) selbst und macht daraus 24 Bit. Normale CDs bleiben unverändert bei 16 Bit.",
            )}
          </Hint>
        </>
      );
    default:
      return <Hint>{t("Keine Einstellungen.")}</Hint>;
  }
}
