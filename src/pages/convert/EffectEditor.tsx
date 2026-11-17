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
  ["Auf 0 dB", [0, 0, 0, 0, 0, 0, 0, 0, 0]],
  ["Konzertsaal", [4, 3, 1, 0, -1, 0, 2, 3, 4]],
  ["Mehr Bass", [8, 6, 4, 1, 0, 0, 0, 0, 0]],
  ["Mehr Höhen", [0, 0, 0, 0, 0, 1, 4, 6, 8]],
];

/** What each kind of "Lautstärke normalisieren" does, in the order of NORMALIZE_MODES. */
const NORMALIZE_HINTS = [
  "Hebt oder senkt jede Datei, bis ihre lauteste Stelle den Zielpegel erreicht. Wie laut sie wirkt, kann trotzdem verschieden bleiben.",
  "Gleicht die gehörte Lautheit an (gemessen nach EBU R128, wie im Rundfunk und bei Streamingdiensten) und ändert den Ton dauerhaft.",
  "Gleicht die gehörte Lautheit an (gemessen nach EBU R128, wie im Rundfunk und bei Streamingdiensten) und ändert den Ton dauerhaft.",
  "Regelt laufend nach, auch innerhalb eines Titels: leise Stellen werden lauter. Für Hörbücher und Mitschnitte, nicht für Musik mit gewollter Dynamik.",
  "Regelt laufend nach den Spitzen nach, auch innerhalb eines Titels. Für Hörbücher und Mitschnitte, nicht für Musik mit gewollter Dynamik.",
  "Ändert jede Datei um denselben Wert; zu viel Anheben verzerrt.",
  "Senkt nur Dateien, deren lauteste Stelle über dem Zielpegel liegt; leisere bleiben unverändert.",
];

/** Tags a value can take over, as `<name>` (the names of file naming). */
const PLACEHOLDERS: [string, string][] = [
  ["artist", "Interpret"],
  ["albumartist", "Albuminterpret"],
  ["album", "Album"],
  ["title", "Titel"],
  ["year", "Jahr"],
  ["genre", "Genre"],
];

/** Fields Tag-Bearbeitung fills in by itself (see AUTO_FIELDS in the core). */
const AUTO_FIELDS = ["track", "track_total"];

/** An example value per field, as the placeholder of the value field. */
const FIELD_EXAMPLES: Record<string, string> = {
  title: "z. B. <title> (Remastered)",
  artist: "z. B. Band",
  album: "z. B. <album> (Deluxe)",
  "album artist": "z. B. <artist>",
  disc: "z. B. 1",
  disc_total: "z. B. 2",
  year: "z. B. 1994",
  genre: "z. B. Rock",
  composer: "z. B. Komponist",
  conductor: "z. B. Dirigent",
  comment: "z. B. Gerippt mit PsyAudio",
  label: "z. B. Label",
  isrc: "z. B. DEA123456789",
  compilation: "1 = Sampler, 0 = kein Sampler",
  lyrics: "Liedtext",
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
        label={t("Wert")}
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
          "Gilt für jede Datei. Ein Knopf fügt den Wert eines anderen Tags ein, z. B. wird <artist> zum Interpreten der Datei.",
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
            "Legt die Quelldatei in den Papierkorb, sobald alle ihre Zieldateien geschrieben sind. Schlägt die Umwandlung fehl, bleibt sie liegen.",
          )}
        </Hint>
      );
    case "PreserveAttributes":
      return (
        <Hint>{t("Die neue Datei bekommt das Änderungsdatum der Quelle statt des heutigen.")}</Hint>
      );
    case "FolderJpgPreserve":
      return (
        <Hint>
          {t(
            "Liegt im Quellordner ein folder.jpg oder cover.jpg, kommt es mit in den Zielordner; ein vorhandenes Bild dort bleibt.",
          )}
        </Hint>
      );
    case "PlaySound":
      return <Hint>{t("Ein kurzer Ton, wenn alle Dateien fertig sind.")}</Hint>;
    case "DeEmphasis":
      return (
        <Hint warn>
          {t(
            "Nur für CDs mit Pre-Emphasis, meist aus den frühen 80ern: Dort wurden die Höhen beim Mastern angehoben, dieser Effekt senkt sie wieder. Bei allen anderen Aufnahmen klingt es danach dumpf.",
          )}
        </Hint>
      );
    case "Resample":
      return (
        <>
          <Select<number>
            label={t("Abtastrate")}
            value={v.rate}
            options={RATES.map((r) => [r, `${r} Hz`] as [number, string])}
            onChange={(x) => set({ rate: x })}
          />
          <Hint>{t("44100 Hz ist CD-Qualität, 48000 Hz der Standard bei Video.")}</Hint>
        </>
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
          <Hint>
            {t(
              "16 Bit ist CD-Qualität. Dither legt beim Verringern ein unhörbares Rauschen darüber, damit leise Stellen nicht verzerren.",
            )}
          </Hint>
        </>
      );
    case "ChannelCount":
      return (
        <>
          <Segmented<number>
            label={t("Kanäle")}
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
            {t(
              "Mono mischt beide Seiten zu einer zusammen, Stereo macht aus Mono zwei gleiche Seiten.",
            )}
          </Hint>
        </>
      );
    case "ChannelMapper":
      return (
        <Select<number>
          label={t("Zuordnung")}
          value={v.mode}
          options={[
            [0, t("Links/Rechts tauschen")],
            [1, t("Links auf beide Seiten")],
            [2, t("Rechts auf beide Seiten")],
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
            label={t("Art")}
            value={mode}
            options={(catalogue?.normalize_modes ?? [])
              .map((n, i) => [i, t(n)] as [number, string])
              // "ReplayGain (Titel)" measures like "EBU R128 (Titel)": shown only for old profiles
              .filter(([i]) => i !== 1 || mode === 1)}
            onChange={(x) => set({ mode: x })}
          />
          <Hint>{t(NORMALIZE_HINTS[mode] ?? "")}</Hint>
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
            <SliderField
              label={t("Ziellautheit")}
              min={-38}
              max={-8}
              step={0.5}
              value={v.target_db - offset}
              unit="LUFS"
              onChange={(x) => set({ target_db: x + offset })}
            />
          ) : (
            <SliderField
              label={t("Zielpegel der lautesten Stelle")}
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
              label={t("Höchstens anheben um")}
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
                {t(
                  "Über diese Zeitspanne mittelt PsyAudio die Lautstärke; kürzer regelt schneller nach.",
                )}
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
            label={t("Ziellautheit")}
            min={-30}
            max={-5}
            step={0.5}
            value={v.target_lufs}
            unit="LUFS"
            onChange={(x) => set({ target_lufs: x })}
          />
          <SliderField
            label={t("Höchstens anheben um")}
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
            label={t("Nie so weit anheben, dass es übersteuert")}
            checked={v.clip_prevention}
            onChange={(x) => set({ clip_prevention: x })}
          />
          <Check
            label={t("Spitzen genauer messen (True Peak, langsamer)")}
            checked={v.true_peak_ratio > 1}
            onChange={(x) => set({ true_peak_ratio: x ? 4 : 1 })}
          />
          <Hint>
            {t(
              "Der Ton bleibt unverändert; Player mit ReplayGain spielen danach alle Titel gleich laut. −18 LUFS ist der übliche Wert. Album-Gain hält die Lautstärke-Unterschiede innerhalb eines Albums.",
            )}
          </Hint>
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
            label={t("Nur so weit anheben, dass nichts übersteuert")}
            checked={v.limit_peaks}
            onChange={(x) => set({ limit_peaks: x })}
          />
          <SliderField
            label={t("Dateien ohne ReplayGain ändern um")}
            min={-40}
            max={0}
            step={0.5}
            value={v.missing_db}
            unit="dB"
            onChange={(x) => set({ missing_db: x })}
          />
          <Hint>
            {t(
              "Rechnet die ReplayGain-Werte aus den Tags fest in den Ton ein, für Player ohne ReplayGain. Dateien ohne Werte ändern sich um den eingestellten Betrag; 0 dB lässt sie, wie sie sind.",
            )}
          </Hint>
        </>
      );
    case "Fade":
      return (
        <>
          {ms("Einblenden", "in_ms", 20000)}
          {ms("Ausblenden (vom Ende)", "out_ms", 20000)}
          <Hint>{t("0 ms schaltet die jeweilige Blende aus.")}</Hint>
        </>
      );
    case "Trim":
      return (
        <>
          {ms("Vom Anfang", "start_ms", 120000)}
          {ms("Vom Ende", "end_ms", 120000)}
          <Hint>{t("Schneidet bei jeder Datei dieselbe Zeit ab; 1000 ms = 1 Sekunde.")}</Hint>
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
            label={t("Nur völlige Stille (digital null)")}
            checked={v.digital_only}
            onChange={(x) => set({ digital_only: x })}
          />
          <SliderField
            label={t("Als Stille gilt leiser als")}
            min={-96}
            max={-20}
            value={v.threshold_db}
            unit="dB"
            disabled={v.digital_only}
            onChange={(x) => set({ threshold_db: x })}
          />
          <Hint>
            {t(
              "−60 dB passt für die meisten Aufnahmen; bei verrauschten Platten etwas höher, etwa −45 dB.",
            )}
          </Hint>
        </>
      );
    case "Lowpass":
    case "Highpass":
      return (
        <>
          <SliderField
            label={t("Grenzfrequenz")}
            log
            min={10}
            max={22000}
            value={v.hz}
            unit="Hz"
            onChange={(x) => set({ hz: x })}
          />
          <Hint>
            {kind === "Highpass"
              ? t(
                  "Entfernt alles unterhalb der Grenze, etwa Rumpeln von Plattenspielern: 20 bis 40 Hz.",
                )
              : t("Entfernt alles oberhalb der Grenze, etwa Rauschen: 12000 bis 16000 Hz.")}
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
            label={t("Stärke")}
            min={1}
            max={20}
            step={0.5}
            value={v.ratio}
            format={(x) => `${x}:1`}
            onChange={(x) => set({ ratio: x })}
          />
          <SliderField
            label={t("Reaktionszeit")}
            min={0.1}
            max={200}
            step={0.1}
            value={v.attack_ms}
            unit="ms"
            onChange={(x) => set({ attack_ms: x })}
          />
          <SliderField
            label={t("Erholzeit")}
            min={10}
            max={5000}
            step={10}
            value={v.release_ms}
            unit="ms"
            onChange={(x) => set({ release_ms: x })}
          />
          <Hint>
            {t(
              "Alles, was lauter ist als die Schwelle, wird leiser: bei 3:1 werden aus 3 dB darüber 1 dB. Die Reaktionszeit sagt, wie schnell das einsetzt, die Erholzeit, wie schnell es wieder nachlässt. Danach ist alles leiser; mit „Lautstärke normalisieren“ dahinter werden die leisen Stellen lauter. Gut für Hörbücher und Mitschnitte mit großen Lautstärkesprüngen.",
            )}
          </Hint>
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
          <Hint>
            {t(
              "Dekodierte HDCDs sind 6 dB leiser als normale CDs, damit ihre erweiterten Spitzen Platz haben. Anheben gleicht das aus; diese Spitzen können dann übersteuern.",
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
            label={t("Speicherort")}
            value={v.same_folder}
            options={[
              [true, t("Eine je Ordner")],
              [false, t("Eine im Zielordner")],
            ]}
            onChange={(x) => set({ same_folder: x })}
          />
          <Hint>
            {t(
              "Die Liste heißt wie ihr Ordner, etwa „Album.m3u8“, und enthält die neuen Dateien in Listenreihenfolge. M3U8 versteht fast jeder Player.",
            )}
          </Hint>
        </>
      );
    case "RunExternal":
      return (
        <>
          <TextField
            label={t("Befehl")}
            value={v.command}
            placeholder={
              os === "windows"
                ? t("z. B. mein-skript.bat [outfile]")
                : t("z. B. ~/bin/mein-skript.sh [outfile]")
            }
            onChange={(x) => set({ command: x })}
          />
          <Hint>
            {t(
              "Läuft nach jeder fertigen Datei, unter Windows über cmd, unter Linux und macOS über sh; PsyAudio wartet, bis der Befehl fertig ist. [outfile] ist die neue Datei, [infile] die Quelle; die Anführungszeichen setzt PsyAudio selbst.",
            )}
          </Hint>
        </>
      );
    case "ConditionalEncoding":
      return (
        <>
          <TextField
            label={t("Quellen mit der Endung")}
            value={v.skip_ext}
            placeholder="mp3"
            onChange={(x) => set({ skip_ext: x })}
          />
          <Segmented<boolean>
            value={v.copy_instead}
            options={[
              [false, t("überspringen")],
              [true, t("unverändert kopieren")],
            ]}
            onChange={(x) => set({ copy_instead: x })}
          />
          <Hint>
            {t(
              "Eine Endung ohne Punkt. Beispiel: Aus einem gemischten Ordner nach MP3 umwandeln, vorhandene MP3s aber nur kopieren statt sie ein zweites Mal verlustbehaftet zu kodieren.",
            )}
          </Hint>
        </>
      );
    case "MoveDestOnError":
      return (
        <>
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
                  const picked = await open({
                    directory: true,
                    defaultPath: v.folder || undefined,
                  });
                  if (typeof picked === "string") set({ folder: picked });
                }}
              >
                {t("Ändern")}
              </button>
            </div>
          </div>
          <Hint>
            {t(
              "Schlägt eine Umwandlung fehl, landet die halbfertige Datei zum Nachsehen hier. Ohne diesen Effekt löscht PsyAudio sie.",
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
            ["cover", t("Cover-Bild")] as [string, string],
            ["*", t("Alle Tags")] as [string, string],
          ]
        : fields;
      return (
        <>
          <Segmented<number>
            value={auto && v.mode === 2 ? 0 : v.mode}
            options={[
              [0, t("Setzen")] as [number, string],
              // numbering only some files would give gaps and doubles
              ...(auto ? [] : [[2, t("Setzen, wenn leer")] as [number, string]]),
              [1, t("Entfernen")] as [number, string],
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
            label={t("Feld")}
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
                      "Wird automatisch eingetragen: 1, 2, 3 … je Ordner, in der Reihenfolge der Dateiliste.",
                    )
                  : t(
                      "Wird automatisch eingetragen: die Anzahl der Dateien dieses Ordners in der Liste.",
                    )}
              </Hint>
            ) : (
              <TagValue field={v.field} value={v.value} onChange={(x) => set({ value: x })} />
            ))}
        </>
      );
    }
    default:
      return <Hint>{t("Keine Einstellungen.")}</Hint>;
  }
}
