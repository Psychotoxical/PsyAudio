import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { Codec } from "../../ipc/bindings/Codec";
import type { EncSettings } from "../../ipc/bindings/EncSettings";
import type { PcmFormat } from "../../ipc/bindings/PcmFormat";
import type { RateMode } from "../../ipc/bindings/RateMode";
import { estimateKbps } from "../../ipc/encode";
import { codecInfo, useEncode } from "../../store/encode";
import { useSettings } from "../../store/settings";
import { Check, Disclosure, Hint, Segmented, Select } from "../../ui/controls";
import { SliderField } from "../../ui/SliderField";
import { TagSchemeEditor } from "./TagSchemeEditor";

function useEnc(): [EncSettings | undefined, (patch: Partial<EncSettings>) => void] {
  const enc = useSettings((s) => s.settings?.enc);
  const update = useSettings((s) => s.update);
  return [enc, (patch) => update((s) => void Object.assign(s.enc, patch))];
}

/** Rust's estimate of the bit rate (44.1 kHz stereo) for formats with a quality setting. */
function useEstimate(codec: Codec, enc: EncSettings | undefined, on: boolean): number | null {
  const [kbps, setKbps] = useState<number | null>(null);
  useEffect(() => {
    if (!on || !enc) return;
    let live = true;
    estimateKbps(codec, enc)
      .then((k) => live && setKbps(k))
      .catch(() => live && setKbps(null));
    return () => {
      live = false;
    };
  }, [codec, enc, on]);
  return on ? kbps : null;
}

const KBPS = "kbit/s";

/** The settings of one format, as in the egui version; pro options sit under "Erweitert". */
export function CodecSettings({ codec }: { codec: Codec }) {
  const { t } = useTranslation();
  const options = useEncode((s) => s.options);
  const [e, set] = useEnc();
  const estimate = useEstimate(
    codec,
    e,
    (codec === "Mp3" && e?.mp3_mode === "Vbr") || codec === "Vorbis",
  );
  if (!e) return null;

  const rateHint = <Hint>{t("Mehr kbit/s heißt bessere Qualität und größere Dateien.")}</Hint>;
  const vbrHint = t(
    "VBR passt die Bitrate jedem Stück an und klingt bei gleicher Dateigröße am besten.",
  );
  const cbrHint = t("CBR hält die Bitrate fest; das brauchen nur alte Geräte und manche Streams.");
  const verify = (key: "flac_verify" | "alac_verify" | "wavpack_verify") => (
    <Disclosure title={t("Erweitert")} summary={t("Geschriebene Datei prüfen")}>
      <div className="field">
        <Check
          label={t("Geschriebene Datei prüfen")}
          checked={e[key]}
          onChange={(v) => set({ [key]: v } as Partial<EncSettings>)}
        />
        <Hint>
          {t(
            "Liest die fertige Datei noch einmal und vergleicht sie Sample für Sample mit dem Original.",
          )}
        </Hint>
      </div>
    </Disclosure>
  );

  switch (codec) {
    case "Mp3":
      return (
        <>
          <div className="field">
            <Segmented<RateMode>
              label={t("Modus")}
              value={e.mp3_mode}
              options={[
                ["Vbr", "VBR"],
                ["Cbr", "CBR"],
                ["Abr", "ABR"],
              ]}
              onChange={(v) => set({ mp3_mode: v })}
            />
            <Hint>
              {e.mp3_mode === "Vbr"
                ? vbrHint
                : e.mp3_mode === "Cbr"
                  ? cbrHint
                  : t("ABR schwankt um die gewählte Bitrate, so bleibt die Dateigröße planbar.")}
            </Hint>
          </div>
          {e.mp3_mode === "Vbr" ? (
            <div className="field">
              <SliderField
                label={t("Qualität")}
                min={0}
                max={9}
                value={9 - e.mp3_vbr}
                format={(v) => `-V ${9 - v}`}
                readOnly
                onChange={(v) => set({ mp3_vbr: 9 - v })}
              />
              <Hint>
                {t("ca. {0} kbit/s · rechts = bessere Qualität", {
                  0: estimate === null ? "…" : String(Math.round(estimate)),
                })}
              </Hint>
            </div>
          ) : (
            <div className="field">
              <SliderField
                label={t("Bitrate")}
                values={options?.mp3_rates}
                value={e.mp3_kbps}
                unit={KBPS}
                onChange={(v) => set({ mp3_kbps: v })}
              />
              {rateHint}
            </div>
          )}
          <Disclosure title={t("Erweitert")} summary={t("Kanäle, Encoder-Sorgfalt")}>
            <div className="field">
              <Select
                label={t("Kanäle")}
                value={e.mp3_stereo}
                options={(options?.stereo_modes ?? []).map((n, i) => [i, t(n)] as [number, string])}
                onChange={(v) => set({ mp3_stereo: v })}
              />
              <Hint>
                {t(
                  "Joint Stereo spart Platz, wenn beide Kanäle ähnlich klingen; „automatisch“ lässt LAME wählen.",
                )}
              </Hint>
            </div>
            <div className="field">
              <Check
                label={t("Encoder-Sorgfalt selbst festlegen")}
                checked={e.mp3_q_custom}
                onChange={(v) => set({ mp3_q_custom: v })}
              />
              {e.mp3_q_custom ? (
                <>
                  <SliderField
                    label={t("Encoder-Sorgfalt")}
                    min={0}
                    max={9}
                    value={9 - e.mp3_q}
                    format={(v) => `-q ${9 - v}`}
                    readOnly
                    onChange={(v) => set({ mp3_q: 9 - v })}
                  />
                  <Hint>
                    {t("Mehr Sorgfalt rechnet länger; hörbar ist der Unterschied selten.")}
                  </Hint>
                </>
              ) : (
                <Hint>{t("LAME wählt die Sorgfalt selbst.")}</Hint>
              )}
            </div>
          </Disclosure>
        </>
      );

    case "Flac":
      return (
        <>
          <div className="field">
            <SliderField
              label={t("Kompressionsstufe")}
              min={0}
              max={8}
              value={e.flac_level}
              onChange={(v) => set({ flac_level: v })}
            />
            <Hint>
              {t(
                "Höhere Stufen machen kleinere Dateien und rechnen länger, der Klang bleibt gleich (üblich: 5).",
              )}
            </Hint>
          </div>
          {verify("flac_verify")}
        </>
      );

    case "AacM4a":
    case "AudioBook":
      return (
        <>
          <div className="field">
            <Segmented<boolean>
              label={t("Modus")}
              value={e.aac_vbr}
              options={[
                [true, "VBR"],
                [false, "CBR"],
              ]}
              onChange={(v) => set({ aac_vbr: v })}
            />
            <Hint>{e.aac_vbr ? vbrHint : cbrHint}</Hint>
          </div>
          {e.aac_vbr ? (
            <div className="field">
              <SliderField
                label={t("Qualität")}
                min={1}
                max={5}
                value={e.aac_quality}
                onChange={(v) => set({ aac_quality: v })}
              />
              <Hint>
                {t("geschätzt {0} kbit/s", {
                  0: String(
                    options?.aac_vbr_kbps[Math.min(4, Math.max(0, e.aac_quality - 1))] ?? "",
                  ),
                })}
              </Hint>
            </div>
          ) : (
            <div className="field">
              <SliderField
                label={t("Bitrate")}
                values={options?.aac_rates}
                value={e.aac_kbps}
                unit={KBPS}
                onChange={(v) => set({ aac_kbps: v })}
              />
              {rateHint}
            </div>
          )}
          <Disclosure title={t("Erweitert")} summary={t("Profil")}>
            <div className="field">
              <Select
                label={t("Profil")}
                value={e.aac_profile}
                options={[
                  [0, "LC AAC"],
                  [1, t("HE AAC (bis 128 kbit/s)")],
                  [2, t("HE v2 AAC (bis 56 kbit/s)")],
                ]}
                onChange={(v) => set({ aac_profile: v })}
              />
              <Hint>{t("HE-Profile brauchen ein FFmpeg mit FDK-AAC.")}</Hint>
            </div>
          </Disclosure>
        </>
      );

    case "Alac":
      return (
        <>
          <Hint>{t("Verlustfrei – keine Einstellungen nötig.")}</Hint>
          {verify("alac_verify")}
        </>
      );

    case "Opus":
      return (
        <>
          <div className="field">
            <SliderField
              label={t("Bitrate")}
              min={6}
              max={512}
              value={e.opus_kbps}
              unit={KBPS}
              onChange={(v) => set({ opus_kbps: v })}
            />
            {rateHint}
          </div>
          <div className="field">
            <Segmented<number>
              label={t("Modus")}
              value={e.opus_mode}
              options={[
                [0, "VBR"],
                [1, t("Eingeschränktes VBR")],
                [2, "CBR"],
              ]}
              onChange={(v) => set({ opus_mode: v })}
            />
            <Hint>
              {e.opus_mode === 0
                ? vbrHint
                : e.opus_mode === 1
                  ? t("Eingeschränktes VBR bleibt nahe an der Bitrate, gut für Streams.")
                  : cbrHint}
            </Hint>
          </div>
        </>
      );

    case "Vorbis":
      return (
        <>
          <div className="field">
            <Segmented<number>
              label={t("Modus")}
              value={e.vorbis_mode}
              options={[
                [0, "VBR"],
                [1, "ABR"],
                [2, "CBR"],
              ]}
              onChange={(v) => set({ vorbis_mode: v })}
            />
            <Hint>
              {e.vorbis_mode === 0
                ? vbrHint
                : e.vorbis_mode === 1
                  ? t(
                      "Die Bitrate schwankt um den gewählten Wert, so bleibt die Dateigröße planbar.",
                    )
                  : cbrHint}
            </Hint>
          </div>
          <div className="field">
            {e.vorbis_mode === 0 ? (
              <SliderField
                label={t("Qualität -q")}
                min={-1}
                max={10}
                value={e.vorbis_q}
                onChange={(v) => set({ vorbis_q: v })}
              />
            ) : (
              <SliderField
                label={t("Bitrate")}
                values={options?.vorbis_kbps}
                value={e.vorbis_kbps}
                unit={KBPS}
                onChange={(v) => set({ vorbis_kbps: v })}
              />
            )}
            <Hint>
              {t("ca. {0} kbit/s", { 0: estimate === null ? "…" : String(Math.round(estimate)) })}
            </Hint>
          </div>
        </>
      );

    case "Wave":
      return (
        <div className="field">
          <Check
            label={t("Bittiefe der Quelle übernehmen")}
            checked={e.wav_match_source}
            onChange={(v) => set({ wav_match_source: v })}
          />
          <Hint>{t("Behält 16, 24 oder 32 Bit der Quelle bei, statt umzurechnen.")}</Hint>
          <div style={{ marginTop: 12 }}>
            <Select<PcmFormat>
              label={t("Bittiefe")}
              value={e.wav_format}
              disabled={e.wav_match_source}
              options={(options?.pcm_formats ?? []).map(
                (p) => [p.format, p.label] as [PcmFormat, string],
              )}
              onChange={(v) => set({ wav_format: v })}
            />
          </div>
        </div>
      );

    case "Aiff":
      return (
        <div className="field">
          <Check
            label={t("Bittiefe der Quelle übernehmen")}
            checked={e.aiff_match_source}
            onChange={(v) => set({ aiff_match_source: v })}
          />
          <Hint>{t("Behält 16, 24 oder 32 Bit der Quelle bei, statt umzurechnen.")}</Hint>
          <div style={{ marginTop: 12 }}>
            <Segmented<number>
              label={t("Bittiefe")}
              value={e.aiff_bits}
              disabled={e.aiff_match_source}
              options={[8, 16, 24, 32].map((b) => [b, `${b} Bit`] as [number, string])}
              onChange={(v) => set({ aiff_bits: v })}
            />
          </div>
        </div>
      );

    case "WavPack":
      return (
        <>
          <div className="field">
            <SliderField
              label={t("Modus")}
              min={0}
              max={Math.max(0, (options?.wavpack_modes.length ?? 1) - 1)}
              value={e.wavpack_mode}
              format={(v) => t(options?.wavpack_modes[v] ?? "")}
              readOnly
              onChange={(v) => set({ wavpack_mode: v })}
            />
            <Hint>
              {t("Höhere Qualität packt kleiner und rechnet länger; verlustfrei bleibt es immer.")}
            </Hint>
          </div>
          {verify("wavpack_verify")}
        </>
      );

    case "MonkeysAudio":
      return (
        <div className="field">
          <SliderField
            label={t("Kompression")}
            min={0}
            max={Math.max(0, (options?.ape_levels.length ?? 1) - 1)}
            value={e.ape_level}
            format={(v) => t(options?.ape_levels[v] ?? "")}
            readOnly
            onChange={(v) => set({ ape_level: v })}
          />
          <Hint>
            {t("Höhere Stufen packen kleiner, schwache Geräte spielen sie aber schwerer ab.")}
          </Hint>
        </div>
      );

    case "Wma":
      return (
        <div className="field">
          <SliderField
            label={t("Bitrate")}
            min={32}
            max={320}
            value={e.wma_kbps}
            unit={KBPS}
            onChange={(v) => set({ wma_kbps: v })}
          />
          {rateHint}
        </div>
      );

    case "Musepack": {
      const profiles = options?.mpc_profiles ?? [];
      const current = profiles[e.mpc_profile % Math.max(1, profiles.length)];
      return (
        <div className="field">
          <SliderField
            label={t("Profil")}
            min={0}
            max={Math.max(0, profiles.length - 1)}
            value={e.mpc_profile}
            format={(v) => t(profiles[v]?.name ?? "")}
            readOnly
            onChange={(v) => set({ mpc_profile: v })}
          />
          <Hint>{t("ca. {0} kbit/s", { 0: String(current?.kbps ?? "") })}</Hint>
        </div>
      );
    }

    case "Speex":
      return (
        <div className="field">
          <SliderField
            label={t("Qualität")}
            min={0}
            max={10}
            value={e.speex_q}
            onChange={(v) => set({ speex_q: v })}
          />
          <Hint>{t("Speex ist für Sprache gemacht; für Musik ist Opus die bessere Wahl.")}</Hint>
        </div>
      );

    case "Dsd":
      return (
        <div className="field">
          <SliderField
            label={t("Abtastrate")}
            min={0}
            max={Math.max(0, (options?.dsd_modes.length ?? 1) - 1)}
            value={e.dsd_mode}
            format={(v) => options?.dsd_modes[v] ?? ""}
            readOnly
            onChange={(v) => set({ dsd_mode: v })}
          />
          <Hint>{t("Schreibt .dsf-Dateien (über SoX).")}</Hint>
        </div>
      );

    case "Multi":
      return (
        <div className="field">
          <p className="lbl">{t("Formate")}</p>
          <div className="chips">
            {(options?.audio_codecs ?? []).map((c) => (
              <Check
                key={c}
                label={codecInfo(options, c)?.short ?? c}
                checked={e.multi.includes(c)}
                onChange={(on) =>
                  set({ multi: on ? [...e.multi, c] : e.multi.filter((x) => x !== c) })
                }
              />
            ))}
          </div>
          <Hint>
            {t(
              "Jede Datei wird in alle gewählten Formate umgewandelt, jeweils mit deren Einstellungen.",
            )}
          </Hint>
        </div>
      );

    case "ChannelSplit":
    case "LengthSplit":
      return (
        <>
          {codec === "LengthSplit" && (
            <div className="field">
              <SliderField
                label={t("Minuten pro Teil")}
                min={1}
                max={180}
                value={Math.max(1, Math.round(e.split_seconds / 60))}
                unit="min"
                onChange={(v) => set({ split_seconds: v * 60 })}
              />
              <Hint>{t("Teilt lange Aufnahmen in gleich lange Stücke.")}</Hint>
            </div>
          )}
          <div className="field">
            <Select<Codec>
              label={t("Format der Teile")}
              value={e.split_codec}
              options={(options?.audio_codecs ?? []).map(
                (c) => [c, codecInfo(options, c)?.name ?? c] as [Codec, string],
              )}
              onChange={(v) => set({ split_codec: v })}
            />
            <Hint>{t("Die Teile bekommen die Einstellungen dieses Formats.")}</Hint>
          </div>
        </>
      );

    case "TagFromFilename":
      return <TagSchemeEditor />;

    case "ReplayGainOnly":
      return <Hint>{t("Schreibt Titel- und Album-Gain (−18 LUFS) in die Quelldateien.")}</Hint>;

    case "TagUpdate":
      return <Hint>{t("Wendet die Effekte „Tag-Bearbeitung“ auf die Quelldateien an.")}</Hint>;

    default:
      return <Hint>{codecInfo(options, codec)?.description ?? ""}</Hint>;
  }
}
