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

/** The settings of one format, as in the egui version; pro options sit under "Advanced". */
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

  const rateHint = <Hint>{t("More kbit/s means better quality and larger files.")}</Hint>;
  const vbrHint = t("VBR adapts the bit rate to each track and sounds best at the same file size.");
  const cbrHint = t("CBR keeps the bit rate fixed; only old devices and some streams need that.");
  const verify = (key: "flac_verify" | "alac_verify" | "wavpack_verify") => (
    <Disclosure title={t("Advanced")} summary={t("Verify the written file")}>
      <div className="field">
        <Check
          label={t("Verify the written file")}
          checked={e[key]}
          onChange={(v) => set({ [key]: v } as Partial<EncSettings>)}
        />
        <Hint>
          {t("Reads the finished file again and compares it sample by sample with the original.")}
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
              label={t("Mode")}
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
                  : t("ABR varies around the chosen bit rate, so the file size stays predictable.")}
            </Hint>
          </div>
          {e.mp3_mode === "Vbr" ? (
            <div className="field">
              <SliderField
                label={t("Quality")}
                min={0}
                max={9}
                value={9 - e.mp3_vbr}
                format={(v) => `-V ${9 - v}`}
                readOnly
                onChange={(v) => set({ mp3_vbr: 9 - v })}
              />
              <Hint>
                {t("approx. {0} kbit/s · right = better quality", {
                  0: estimate === null ? "…" : String(Math.round(estimate)),
                })}
              </Hint>
            </div>
          ) : (
            <div className="field">
              <SliderField
                label={t("Bit rate")}
                values={options?.mp3_rates}
                value={e.mp3_kbps}
                unit={KBPS}
                onChange={(v) => set({ mp3_kbps: v })}
              />
              {rateHint}
            </div>
          )}
          <Disclosure title={t("Advanced")} summary={t("Channels, encoder effort")}>
            <div className="field">
              <Select
                label={t("Channels")}
                value={e.mp3_stereo}
                options={(options?.stereo_modes ?? []).map((n, i) => [i, t(n)] as [number, string])}
                onChange={(v) => set({ mp3_stereo: v })}
              />
              <Hint>
                {t(
                  "Joint stereo saves space when both channels sound alike; “automatic” lets LAME choose.",
                )}
              </Hint>
            </div>
            <div className="field">
              <Check
                label={t("Set encoder effort yourself")}
                checked={e.mp3_q_custom}
                onChange={(v) => set({ mp3_q_custom: v })}
              />
              {e.mp3_q_custom ? (
                <>
                  <SliderField
                    label={t("Encoder effort")}
                    min={0}
                    max={9}
                    value={9 - e.mp3_q}
                    format={(v) => `-q ${9 - v}`}
                    readOnly
                    onChange={(v) => set({ mp3_q: 9 - v })}
                  />
                  <Hint>{t("More effort takes longer; the difference is rarely audible.")}</Hint>
                </>
              ) : (
                <Hint>{t("LAME chooses the effort itself.")}</Hint>
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
              label={t("Compression level")}
              min={0}
              max={8}
              value={e.flac_level}
              onChange={(v) => set({ flac_level: v })}
            />
            <Hint>
              {t(
                "Higher levels make smaller files and take longer; the sound stays the same (usual: 5).",
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
              label={t("Mode")}
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
                label={t("Quality")}
                min={1}
                max={5}
                value={e.aac_quality}
                onChange={(v) => set({ aac_quality: v })}
              />
              <Hint>
                {t("estimated {0} kbit/s", {
                  0: String(
                    options?.aac_vbr_kbps[Math.min(4, Math.max(0, e.aac_quality - 1))] ?? "",
                  ),
                })}
              </Hint>
            </div>
          ) : (
            <div className="field">
              <SliderField
                label={t("Bit rate")}
                values={options?.aac_rates}
                value={e.aac_kbps}
                unit={KBPS}
                onChange={(v) => set({ aac_kbps: v })}
              />
              {rateHint}
            </div>
          )}
          <Disclosure title={t("Advanced")} summary={t("Profile")}>
            <div className="field">
              <Select
                label={t("Profile")}
                value={e.aac_profile}
                options={[
                  [0, "LC AAC"],
                  [1, t("HE AAC (up to 128 kbit/s)")],
                  [2, t("HE v2 AAC (up to 56 kbit/s)")],
                ]}
                onChange={(v) => set({ aac_profile: v })}
              />
              <Hint>{t("HE profiles need an FFmpeg with FDK-AAC.")}</Hint>
            </div>
          </Disclosure>
        </>
      );

    case "Alac":
      return (
        <>
          <Hint>{t("Lossless – no settings needed.")}</Hint>
          {verify("alac_verify")}
        </>
      );

    case "Opus":
      return (
        <>
          <div className="field">
            <SliderField
              label={t("Bit rate")}
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
              label={t("Mode")}
              value={e.opus_mode}
              options={[
                [0, "VBR"],
                [1, t("Constrained VBR")],
                [2, "CBR"],
              ]}
              onChange={(v) => set({ opus_mode: v })}
            />
            <Hint>
              {e.opus_mode === 0
                ? vbrHint
                : e.opus_mode === 1
                  ? t("Constrained VBR stays close to the bit rate, good for streams.")
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
              label={t("Mode")}
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
                      "The bit rate varies around the chosen value, so the file size stays predictable.",
                    )
                  : cbrHint}
            </Hint>
          </div>
          <div className="field">
            {e.vorbis_mode === 0 ? (
              <SliderField
                label={t("Quality -q")}
                min={-1}
                max={10}
                value={e.vorbis_q}
                onChange={(v) => set({ vorbis_q: v })}
              />
            ) : (
              <SliderField
                label={t("Bit rate")}
                values={options?.vorbis_kbps}
                value={e.vorbis_kbps}
                unit={KBPS}
                onChange={(v) => set({ vorbis_kbps: v })}
              />
            )}
            <Hint>
              {t("approx. {0} kbit/s", {
                0: estimate === null ? "…" : String(Math.round(estimate)),
              })}
            </Hint>
          </div>
        </>
      );

    case "Wave":
      return (
        <div className="field">
          <Check
            label={t("Keep the source bit depth")}
            checked={e.wav_match_source}
            onChange={(v) => set({ wav_match_source: v })}
          />
          <Hint>{t("Keeps the source's 16, 24 or 32 bits instead of converting.")}</Hint>
          <div style={{ marginTop: 12 }}>
            <Select<PcmFormat>
              label={t("Bit depth")}
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
            label={t("Keep the source bit depth")}
            checked={e.aiff_match_source}
            onChange={(v) => set({ aiff_match_source: v })}
          />
          <Hint>{t("Keeps the source's 16, 24 or 32 bits instead of converting.")}</Hint>
          <div style={{ marginTop: 12 }}>
            <Segmented<number>
              label={t("Bit depth")}
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
              label={t("Mode")}
              min={0}
              max={Math.max(0, (options?.wavpack_modes.length ?? 1) - 1)}
              value={e.wavpack_mode}
              format={(v) => t(options?.wavpack_modes[v] ?? "")}
              readOnly
              onChange={(v) => set({ wavpack_mode: v })}
            />
            <Hint>
              {t("Higher quality packs smaller and takes longer; it always stays lossless.")}
            </Hint>
          </div>
          {verify("wavpack_verify")}
        </>
      );

    case "MonkeysAudio":
      return (
        <div className="field">
          <SliderField
            label={t("Compression")}
            min={0}
            max={Math.max(0, (options?.ape_levels.length ?? 1) - 1)}
            value={e.ape_level}
            format={(v) => t(options?.ape_levels[v] ?? "")}
            readOnly
            onChange={(v) => set({ ape_level: v })}
          />
          <Hint>
            {t("Higher levels pack smaller, but weak devices have a harder time playing them.")}
          </Hint>
        </div>
      );

    case "Wma":
      return (
        <div className="field">
          <SliderField
            label={t("Bit rate")}
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
            label={t("Profile")}
            min={0}
            max={Math.max(0, profiles.length - 1)}
            value={e.mpc_profile}
            format={(v) => t(profiles[v]?.name ?? "")}
            readOnly
            onChange={(v) => set({ mpc_profile: v })}
          />
          <Hint>{t("approx. {0} kbit/s", { 0: String(current?.kbps ?? "") })}</Hint>
        </div>
      );
    }

    case "Speex":
      return (
        <div className="field">
          <SliderField
            label={t("Quality")}
            min={0}
            max={10}
            value={e.speex_q}
            onChange={(v) => set({ speex_q: v })}
          />
          <Hint>{t("Speex is made for speech; for music, Opus is the better choice.")}</Hint>
        </div>
      );

    case "Dsd":
      return (
        <div className="field">
          <SliderField
            label={t("Sample rate")}
            min={0}
            max={Math.max(0, (options?.dsd_modes.length ?? 1) - 1)}
            value={e.dsd_mode}
            format={(v) => options?.dsd_modes[v] ?? ""}
            readOnly
            onChange={(v) => set({ dsd_mode: v })}
          />
          <Hint>{t("Writes .dsf files (via SoX).")}</Hint>
        </div>
      );

    case "Multi":
      return (
        <div className="field">
          <p className="lbl">{t("Formats")}</p>
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
            {t("Each file is converted to all chosen formats, each with its own settings.")}
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
                label={t("Minutes per part")}
                min={1}
                max={180}
                value={Math.max(1, Math.round(e.split_seconds / 60))}
                unit="min"
                onChange={(v) => set({ split_seconds: v * 60 })}
              />
              <Hint>{t("Splits long recordings into pieces of equal length.")}</Hint>
            </div>
          )}
          <div className="field">
            <Select<Codec>
              label={t("Format of the parts")}
              value={e.split_codec}
              options={(options?.audio_codecs ?? []).map(
                (c) => [c, codecInfo(options, c)?.name ?? c] as [Codec, string],
              )}
              onChange={(v) => set({ split_codec: v })}
            />
            <Hint>{t("The pieces get this format's settings.")}</Hint>
          </div>
        </>
      );

    case "TagFromFilename":
      return <TagSchemeEditor />;

    case "ReplayGainOnly":
      return <Hint>{t("Writes track and album gain (−18 LUFS) to the source files.")}</Hint>;

    case "TagUpdate":
      return <Hint>{t("Applies the “Tag processing” effects to the source files.")}</Hint>;

    default:
      return <Hint>{codecInfo(options, codec)?.description ?? ""}</Hint>;
  }
}
