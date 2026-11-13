//! Small UI helpers and the settings editors for formats and effects.

use crate::audio;
use crate::dsp::Effect;
use crate::encode::{self, Codec, EncSettings, Env, RateMode};
use crate::tags;
use crate::theme::{self, Palette};
use eframe::egui::{self, Color32, RichText};
use std::path::Path;

pub fn plan_requirement(codec: Codec, enc: &EncSettings, env: &Env) -> Option<String> {
    let codecs = match codec {
        Codec::Multi => enc.multi.clone(),
        Codec::ChannelSplit | Codec::LengthSplit => vec![enc.split_codec],
        c if c.is_tool() => vec![],
        c => vec![c],
    };
    if codec == Codec::Multi && codecs.is_empty() {
        return Some(crate::i18n::t("Im Mehrfach-Encoder ist kein Format gewählt").into());
    }
    codecs.into_iter().find_map(|c| encode::missing_requirement(c, env).map(|m| format!("{}: {m}", c.name())))
}

pub fn section(ui: &mut egui::Ui, p: &Palette, title: &str) {
    ui.add_space(2.0);
    ui.label(RichText::new(title.to_uppercase()).size(11.5).color(p.muted).strong());
}

pub fn muted(p: &Palette, t: impl Into<String>) -> RichText {
    RichText::new(t.into()).color(p.muted)
}

/// One chip of a naming scheme: data chips filled, separators quiet, "new folder" in the accent.
fn scheme_chip(ui: &mut egui::Ui, p: &Palette, token: &str, sense: egui::Sense, lifted: bool) -> egui::Response {
    let folder = token == crate::naming::FOLDER;
    let data = crate::naming::DATA.contains(&token);
    let (fill, stroke, color) = if folder {
        (p.card2, egui::Stroke::new(1.0_f32, p.accent), p.accent)
    } else if data {
        (p.card2, egui::Stroke::new(1.0_f32, p.field_border), p.text)
    } else {
        (p.card2, egui::Stroke::new(1.0_f32, p.field_border), p.muted)
    };
    let stroke = if lifted { egui::Stroke::new(1.5_f32, p.accent) } else { stroke };
    ui.add(
        egui::Button::new(RichText::new(crate::i18n::t(crate::naming::label(token))).size(12.5).color(color))
            .fill(fill)
            .stroke(stroke)
            .corner_radius(10)
            .min_size(egui::vec2(26.0, 24.0))
            .sense(sense),
    )
}

/// Builds folder and file names from chips: clicking a chip above appends it, clicking a chip in
/// the scheme removes it, dragging sorts. `data` are the data chips offered here.
pub fn scheme_editor(ui: &mut egui::Ui, p: &Palette, scheme: &mut Vec<String>, data: &[&str], folders: bool, default: &[&str]) {
    ui.label(muted(p, crate::i18n::t("Zum Anfügen anklicken:")));
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
        for token in data {
            if scheme_chip(ui, p, token, egui::Sense::click(), false).clicked() {
                scheme.push(token.to_string());
            }
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
        let extra: &[&str] = if folders { &[crate::naming::FOLDER] } else { &[] };
        for token in crate::naming::SEPARATORS.iter().chain(extra) {
            if scheme_chip(ui, p, token, egui::Sense::click(), false).clicked() {
                scheme.push(token.to_string());
            }
        }
    });
    ui.add_space(2.0);
    ui.label(muted(p, crate::i18n::t("So wird benannt (anklicken entfernt, ziehen sortiert):")));
    let mut rects = Vec::with_capacity(scheme.len());
    let (mut remove, mut dragged, mut dropped) = (None, None, false);
    egui::Frame::new()
        .fill(p.field)
        .stroke(egui::Stroke::new(1.0_f32, p.field_border))
        .corner_radius(theme::R_CTRL)
        .inner_margin(egui::Margin::same(6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                for (i, token) in scheme.iter().enumerate() {
                    let r = scheme_chip(ui, p, token, egui::Sense::click_and_drag(), false);
                    if r.dragged() || r.drag_stopped() {
                        dragged = Some(i);
                        dropped = r.drag_stopped();
                    } else if r.clicked() {
                        remove = Some(i);
                    }
                    rects.push(r.rect);
                }
                if scheme.is_empty() {
                    ui.label(muted(p, crate::i18n::t("Noch keine Bausteine.")));
                }
            });
        });
    if let (Some(from), Some(pos)) = (dragged, ui.ctx().pointer_interact_pos()) {
        // where the chip would land: before the chip under the pointer, or after it past its middle
        let target = rects
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.distance_sq_to_pos(pos).total_cmp(&b.1.distance_sq_to_pos(pos)))
            .map(|(i, r)| if pos.x > r.center().x { i + 1 } else { i });
        if let Some(to) = target {
            let x = match rects.get(to) {
                Some(r) => r.left() - 2.0,
                None => rects.last().map(|r| r.right() + 2.0).unwrap_or(pos.x),
            };
            let row = rects.get(to).or(rects.last()).map(|r| r.y_range()).unwrap_or(egui::Rangef::point(pos.y));
            ui.painter().vline(x, row, egui::Stroke::new(2.0_f32, p.accent));
            if dropped {
                let item = scheme.remove(from);
                let to = if to > from { to - 1 } else { to };
                scheme.insert(to.min(scheme.len()), item);
            }
        }
    } else if let Some(i) = remove {
        scheme.remove(i);
    }
    if ui.add(theme::ghost(crate::i18n::t("Vorgabe"))).on_hover_text(crate::i18n::t("Bausteine auf die Vorgabe zurücksetzen")).clicked() {
        *scheme = default.iter().map(|s| s.to_string()).collect();
    }
}

/// The Phosphor icons (one stroke and weight for every icon in the app).
pub use egui_phosphor::regular as ph;

/// One choice out of a few, as one control (as in PsyTags): the options sit in a field, the
/// chosen one is filled in the accent colour, a hovered one gets an outline.
pub fn segmented<T: PartialEq + Copy>(ui: &mut egui::Ui, p: &Palette, value: &mut T, options: &[(T, &str)]) -> egui::Response {
    egui::Frame::new()
        .fill(p.field)
        .stroke(egui::Stroke::new(1.0_f32, p.border))
        .corner_radius(theme::R_CTRL)
        .inner_margin(egui::Margin::same(2))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(2.0, 2.0);
                for (v, label) in options {
                    let on = *value == *v;
                    let text = RichText::new(*label).color(if on { p.on_accent } else { p.muted });
                    let text = if on { text.family(theme::semibold()) } else { text };
                    let r = ui.add(
                        egui::Button::new(text)
                            .fill(if on { p.accent } else { Color32::TRANSPARENT })
                            .stroke(egui::Stroke::NONE)
                            .corner_radius(4)
                            .min_size(egui::vec2(0.0, 26.0)),
                    );
                    if r.hovered() && !on {
                        ui.painter().rect_stroke(r.rect, 4, egui::Stroke::new(1.0_f32, p.field_border), egui::StrokeKind::Inside);
                    }
                    if r.clicked() {
                        *value = *v;
                    }
                }
            })
        })
        .response
}

/// Icon and text for a button or label.
pub fn icon_text(icon: &str, text: &str) -> String {
    format!("{icon}  {text}")
}

pub fn pick_folder(current: &str) -> Option<String> {
    let mut d = rfd::FileDialog::new();
    if !current.is_empty() && Path::new(current).exists() {
        d = d.set_directory(current);
    }
    d.pick_folder().map(|p| p.display().to_string())
}

pub fn pick_exe() -> Option<String> {
    rfd::FileDialog::new().add_filter(crate::i18n::t("Programm"), &["exe"]).pick_file().map(|p| p.display().to_string())
}

pub fn image_texture(ctx: &egui::Context, name: &str, data: &[u8]) -> Option<egui::TextureHandle> {
    let img = image::load_from_memory(data).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    let ci = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], img.as_raw());
    Some(ctx.load_texture(name, ci, egui::TextureOptions::LINEAR))
}

/// A gradient square with a disc, used where no cover picture exists.
pub fn cover_placeholder(ui: &mut egui::Ui, p: &Palette, size: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let painter = ui.painter();
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.left_top(), p.accent);
    mesh.colored_vertex(rect.right_top(), p.accent2);
    mesh.colored_vertex(rect.right_bottom(), p.accent.gamma_multiply(0.5));
    mesh.colored_vertex(rect.left_bottom(), p.accent2.gamma_multiply(0.4));
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(egui::Shape::mesh(mesh));
    painter.circle_stroke(rect.center(), size * 0.28, egui::Stroke::new(3.0_f32, Color32::from_white_alpha(170)));
    painter.circle_filled(rect.center(), size * 0.06, Color32::from_white_alpha(200));
}

/// Format-specific settings.
pub fn codec_settings(ui: &mut egui::Ui, p: &Palette, codec: Codec, e: &mut EncSettings) {
    match codec {
        Codec::Mp3 => {
            segmented(ui, p, &mut e.mp3_mode, &[(RateMode::Vbr, crate::i18n::t("Variabel (VBR)")), (RateMode::Cbr, crate::i18n::t("Konstant (CBR)")), (RateMode::Abr, crate::i18n::t("Durchschnitt (ABR)"))]);
            if e.mp3_mode == RateMode::Vbr {
                let mut q = 9 - e.mp3_vbr as i32;
                slider_row(ui, crate::i18n::tf(crate::i18n::t("Qualität -V {0}"), &[("0", format!("{}", e.mp3_vbr))]), egui::Slider::new(&mut q, 0..=9).show_value(false));
                e.mp3_vbr = (9 - q) as u8;
                ui.label(muted(p, crate::i18n::tf(crate::i18n::t("ca. {0} kbit/s · rechts = bessere Qualität"), &[("0", format!("{:.0}", encode::estimate_bps(Codec::Mp3, e, 44100, 2, 16) / 125.0))])));
            } else {
                let mut idx = encode::MP3_RATES.iter().position(|r| *r == e.mp3_kbps).unwrap_or(13);
                slider_row(ui, crate::i18n::t("Bitrate"), rate_slider(&mut idx, encode::MP3_RATES));
                e.mp3_kbps = encode::MP3_RATES[idx];
            }
            egui::ComboBox::from_label(crate::i18n::t("Kanäle")).height(dropdown_height(ui)).selected_text(crate::i18n::t(encode::STEREO_MODES[e.mp3_stereo as usize % 5])).show_ui(ui, |ui| {
                for (i, n) in encode::STEREO_MODES.iter().enumerate() {
                    ui.selectable_value(&mut e.mp3_stereo, i as u8, crate::i18n::t(n));
                }
            });
            ui.checkbox(&mut e.mp3_q_custom, crate::i18n::t("Encoder-Sorgfalt selbst festlegen"));
            if e.mp3_q_custom {
                let mut q = 9 - e.mp3_q as i32;
                slider_row(ui, crate::i18n::tf(crate::i18n::t("Encoder-Sorgfalt -q {0}"), &[("0", format!("{}", e.mp3_q))]), egui::Slider::new(&mut q, 0..=9).show_value(false));
                e.mp3_q = (9 - q) as u8;
            } else {
                ui.label(muted(p, crate::i18n::t("LAME wählt die Sorgfalt selbst.")));
            }
        }
        Codec::Flac => {
            slider_row(ui, crate::i18n::t("Kompressionsstufe"), egui::Slider::new(&mut e.flac_level, 0..=8));
            let hint = match e.flac_level {
                0 => crate::i18n::t("Stufe 0 (am schnellsten)"),
                5 => crate::i18n::t("Stufe 5 (empfohlen)"),
                8 => crate::i18n::t("Stufe 8 (am stärksten)"),
                _ => "",
            };
            if !hint.is_empty() {
                ui.label(muted(p, hint));
            }
            ui.checkbox(&mut e.flac_verify, crate::i18n::t("Geschriebene Datei prüfen"));
        }
        Codec::AacM4a | Codec::AudioBook => {
            segmented(ui, p, &mut e.aac_vbr, &[(true, crate::i18n::t("Qualität (VBR)")), (false, crate::i18n::t("Bitrate (CBR)"))]);
            if e.aac_vbr {
                slider_row(ui, crate::i18n::t("Qualität"), egui::Slider::new(&mut e.aac_quality, 1..=5));
                ui.label(muted(p, crate::i18n::tf(crate::i18n::t("geschätzt {0} kbit/s"), &[("0", format!("{}", encode::AAC_VBR_KBPS[(e.aac_quality.clamp(1, 5) - 1) as usize]))])));
            } else {
                let mut idx = encode::AAC_RATES.iter().position(|r| *r == e.aac_kbps).unwrap_or(13);
                slider_row(ui, crate::i18n::t("Bitrate"), rate_slider(&mut idx, encode::AAC_RATES));
                e.aac_kbps = encode::AAC_RATES[idx];
            }
            egui::ComboBox::from_label(crate::i18n::t("Profil")).height(dropdown_height(ui)).selected_text(["LC AAC", "HE AAC", "HE v2 AAC"][e.aac_profile as usize % 3]).show_ui(ui, |ui| {
                ui.selectable_value(&mut e.aac_profile, 0, "LC AAC");
                ui.selectable_value(&mut e.aac_profile, 1, crate::i18n::t("HE AAC (bis 128 kbit/s)"));
                ui.selectable_value(&mut e.aac_profile, 2, crate::i18n::t("HE v2 AAC (bis 56 kbit/s)"));
            });
            ui.label(muted(p, crate::i18n::t("HE-Profile brauchen ein FFmpeg mit FDK-AAC.")));
        }
        Codec::Alac => {
            ui.label(muted(p, crate::i18n::t("Verlustfrei – keine Einstellungen nötig.")));
            ui.checkbox(&mut e.alac_verify, crate::i18n::t("Geschriebene Datei prüfen"));
        }
        Codec::Opus => {
            slider_row(ui, crate::i18n::t("Bitrate"), egui::Slider::new(&mut e.opus_kbps, 6..=512).suffix(" kbit/s"));
            segmented(ui, p, &mut e.opus_mode, &[(0, crate::i18n::t("VBR (beste Qualität)")), (1, crate::i18n::t("Eingeschränktes VBR")), (2, "CBR")]);
        }
        Codec::Vorbis => {
            segmented(ui, p, &mut e.vorbis_mode, &[(0, crate::i18n::t("Qualität (VBR)")), (1, crate::i18n::t("Durchschnitts-Bitrate")), (2, crate::i18n::t("Feste Bitrate"))]);
            if e.vorbis_mode == 0 {
                slider_row(ui, crate::i18n::t("Qualität -q"), egui::Slider::new(&mut e.vorbis_q, -1.0..=10.0).step_by(1.0));
            } else {
                let mut idx = encode::VORBIS_KBPS.iter().position(|k| *k >= e.vorbis_kbps).unwrap_or(5);
                slider_row(ui, crate::i18n::t("Bitrate"), rate_slider(&mut idx, &encode::VORBIS_KBPS));
                e.vorbis_kbps = encode::VORBIS_KBPS[idx];
            }
            ui.label(muted(p, crate::i18n::tf(crate::i18n::t("ca. {0} kbit/s"), &[("0", format!("{:.0}", encode::estimate_bps(Codec::Vorbis, e, 44100, 2, 16) / 125.0))])));
        }
        Codec::Wave => {
            ui.checkbox(&mut e.wav_match_source, crate::i18n::t("Bittiefe der Quelle übernehmen"));
            ui.add_enabled_ui(!e.wav_match_source, |ui| {
                egui::ComboBox::from_label(crate::i18n::t("Bittiefe")).height(dropdown_height(ui)).selected_text(e.wav_format.label()).show_ui(ui, |ui| {
                    use audio::PcmFormat::*;
                    for f in [U8, S16, S24, S32, F32, F64] {
                        ui.selectable_value(&mut e.wav_format, f, f.label());
                    }
                });
            });
        }
        Codec::Aiff => {
            ui.checkbox(&mut e.aiff_match_source, crate::i18n::t("Bittiefe der Quelle übernehmen"));
            ui.add_enabled_ui(!e.aiff_match_source, |ui| {
                ui.horizontal(|ui| {
                    for b in [8u16, 16, 24, 32] {
                        ui.selectable_value(&mut e.aiff_bits, b, format!("{b} Bit"));
                    }
                });
            });
        }
        Codec::WavPack => {
            ui.horizontal_wrapped(|ui| {
                for (i, n) in encode::WAVPACK_MODES.iter().enumerate() {
                    ui.selectable_value(&mut e.wavpack_mode, i as u8, crate::i18n::t(n));
                }
            });
            ui.checkbox(&mut e.wavpack_verify, crate::i18n::t("Geschriebene Datei prüfen"));
        }
        Codec::MonkeysAudio => {
            ui.horizontal_wrapped(|ui| {
                for (i, (n, _)) in encode::APE_LEVELS.iter().enumerate() {
                    ui.selectable_value(&mut e.ape_level, i as u8, crate::i18n::t(n));
                }
            });
        }
        Codec::Wma => {
            slider_row(ui, crate::i18n::t("Bitrate"), egui::Slider::new(&mut e.wma_kbps, 32..=320).suffix(" kbit/s"));
        }
        Codec::Musepack => {
            let (n, _, k) = encode::MPC_PROFILES[e.mpc_profile as usize % encode::MPC_PROFILES.len()];
            ui.add(
                egui::Slider::new(&mut e.mpc_profile, 0..=(encode::MPC_PROFILES.len() - 1) as u8)
                    .show_value(false)
                    .text(format!("{} ({})", crate::i18n::t(n), crate::i18n::tf(crate::i18n::t("ca. {0} kbit/s"), &[("0", k.to_string())]))),
            );
        }
        Codec::Speex => {
            slider_row(ui, crate::i18n::t("Qualität"), egui::Slider::new(&mut e.speex_q, 0..=10));
        }
        Codec::Dsd => {
            ui.horizontal_wrapped(|ui| {
                for (i, (n, _)) in encode::DSD_MODES.iter().enumerate() {
                    ui.selectable_value(&mut e.dsd_mode, i as u8, *n);
                }
            });
            ui.label(muted(p, crate::i18n::t("Schreibt .dsf-Dateien (über SoX).")));
        }
        Codec::Multi => {
            ui.label(muted(p, crate::i18n::t("Jede Datei wird in alle gewählten Formate umgewandelt (mit deren Einstellungen):")));
            ui.horizontal_wrapped(|ui| {
                for c in encode::AUDIO_CODECS {
                    let mut on = e.multi.contains(c);
                    if ui.checkbox(&mut on, c.short()).changed() {
                        if on {
                            e.multi.push(*c);
                        } else {
                            e.multi.retain(|x| x != c);
                        }
                    }
                }
            });
        }
        Codec::ChannelSplit | Codec::LengthSplit => {
            if codec == Codec::LengthSplit {
                let mut m = (e.split_seconds / 60).max(1);
                slider_row(ui, crate::i18n::t("Minuten pro Teil"), egui::Slider::new(&mut m, 1..=180));
                e.split_seconds = m * 60;
            }
            egui::ComboBox::from_label(crate::i18n::t("Format der Teile")).height(dropdown_height(ui)).selected_text(e.split_codec.name()).show_ui(ui, |ui| {
                for c in encode::AUDIO_CODECS {
                    ui.selectable_value(&mut e.split_codec, *c, c.name());
                }
            });
        }
        Codec::TagFromFilename => {
            ui.label(muted(p, crate::i18n::t("Das Muster steht auf der Seite Tags.")));
        }
        Codec::ReplayGainOnly => {
            ui.label(muted(p, crate::i18n::t("Schreibt Titel- und Album-Gain (−18 LUFS) in die Quelldateien.")));
        }
        Codec::TagUpdate => {
            ui.label(muted(p, crate::i18n::t("Wendet die Effekte „Tag-Bearbeitung“ auf die Quelldateien an.")));
        }
        _ => {
            ui.label(muted(p, codec.description()));
        }
    }
}

/// Editor for one effect's settings.
pub fn effect_editor(ui: &mut egui::Ui, p: &Palette, e: &mut Effect) {
    match e {
        Effect::Resample { rate } => {
            ui.horizontal_wrapped(|ui| {
                for r in [8000u32, 11025, 16000, 22050, 32000, 44100, 48000, 88200, 96000, 176400, 192000] {
                    ui.selectable_value(rate, r, format!("{r}"));
                }
            });
        }
        Effect::BitDepth { bits, dither } => {
            ui.horizontal(|ui| {
                for b in [8u16, 16, 24, 32] {
                    ui.selectable_value(bits, b, format!("{b} Bit"));
                }
            });
            ui.checkbox(dither, crate::i18n::t("Dither (empfohlen beim Reduzieren auf 16 Bit)"));
        }
        Effect::ChannelCount { channels } => {
            segmented(ui, p, channels, &[(1, "Mono"), (2, "Stereo"), (6, "5.1")]);
        }
        Effect::ChannelMapper { mode } => {
            segmented(ui, p, mode, &[(0, crate::i18n::t("Links/Rechts tauschen")), (1, crate::i18n::t("Nur links")), (2, crate::i18n::t("Nur rechts"))]);
        }
        Effect::VolumeNormalize { mode, target_db, max_amp, window_ms, fixed_db } => {
            egui::ComboBox::from_label(crate::i18n::t("Art")).height(dropdown_height(ui))
                .selected_text(crate::i18n::t(crate::dsp::NORMALIZE_MODES[*mode as usize % 7]))
                .show_ui(ui, |ui| {
                    for (i, n) in crate::dsp::NORMALIZE_MODES.iter().enumerate() {
                        ui.selectable_value(mode, i as u8, crate::i18n::t(n));
                    }
                });
            match *mode {
                5 => {
                    slider_row(ui, crate::i18n::t("dB Verstärkung"), egui::Slider::new(fixed_db, -12.0..=12.0).step_by(0.1));
                }
                1..=3 => {
                    slider_row(ui, crate::i18n::t("Zielpegel dB"), egui::Slider::new(target_db, -20.0..=10.0).step_by(0.5));
                    ui.label(muted(p, format!("= {:.1} LUFS", *target_db - crate::dsp::NORMALIZE_EBU_OFFSET)));
                }
                _ => {
                    slider_row(ui, crate::i18n::t("Zielpegel dB (Spitze)"), egui::Slider::new(target_db, -20.0..=0.0).step_by(0.1));
                }
            }
            if *mode != 5 && *mode != 6 {
                slider_row(ui, crate::i18n::t("Max. Verstärkung ×"), egui::Slider::new(max_amp, 1.0..=20.0));
            }
            if *mode == 3 || *mode == 4 {
                slider_row(ui, crate::i18n::t("Fensterlänge ms"), egui::Slider::new(window_ms, 500..=30000));
            }
        }
        Effect::ReplayGain { target_lufs, max_gain, album, clip_prevention, true_peak_ratio } => {
            slider_row(ui, crate::i18n::t("Ziel LUFS"), egui::Slider::new(target_lufs, -30.0..=-5.0));
            slider_row(ui, crate::i18n::t("Max. Verstärkung dB"), egui::Slider::new(max_gain, 0.0..=40.0));
            ui.checkbox(album, crate::i18n::t("Auch Album-Gain berechnen"));
            ui.checkbox(clip_prevention, crate::i18n::t("Übersteuerung verhindern"));
            slider_row(ui, crate::i18n::t("True-Peak-Überabtastung ×"), egui::Slider::new(true_peak_ratio, 1..=16));
        }
        Effect::ReplayGainApply { album, limit_peaks, missing_db } => {
            segmented(ui, p, album, &[(false, crate::i18n::t("Titel-Gain")), (true, crate::i18n::t("Album-Gain"))]);
            ui.checkbox(limit_peaks, crate::i18n::t("Spitzen begrenzen"));
            slider_row(ui, crate::i18n::t("dB, wenn ReplayGain fehlt"), egui::Slider::new(missing_db, -40.0..=0.0));
        }
        Effect::Fade { in_ms, out_ms } => {
            slider_row(ui, crate::i18n::t("Einblenden ms"), egui::Slider::new(in_ms, 0..=20000));
            slider_row(ui, crate::i18n::t("Ausblenden ms (vom Ende)"), egui::Slider::new(out_ms, 0..=20000));
        }
        Effect::Trim { start_ms, end_ms } => {
            slider_row(ui, crate::i18n::t("Vom Anfang ms"), egui::Slider::new(start_ms, 0..=120000));
            slider_row(ui, crate::i18n::t("Vom Ende ms"), egui::Slider::new(end_ms, 0..=120000));
        }
        Effect::TrimSilence { begin, end, digital_only, threshold_db } => {
            ui.checkbox(begin, crate::i18n::t("Stille am Anfang entfernen"));
            ui.checkbox(end, crate::i18n::t("Stille am Ende entfernen"));
            ui.checkbox(digital_only, crate::i18n::t("Nur digitale Stille (Nullen)"));
            ui.add_enabled_ui(!*digital_only, |ui| slider_row(ui, crate::i18n::t("Schwelle dB"), egui::Slider::new(threshold_db, -96.0..=-20.0))).inner;
        }
        Effect::WriteSilence { start_ms, end_ms } => {
            slider_row(ui, crate::i18n::t("Vorne ms"), egui::Slider::new(start_ms, 0..=60000));
            slider_row(ui, crate::i18n::t("Hinten ms"), egui::Slider::new(end_ms, 0..=60000));
        }
        Effect::Loop { times } => {
            slider_row(ui, crate::i18n::t("Wiederholungen"), egui::Slider::new(times, 1..=50));
        }
        Effect::Speed { percent } => {
            slider_row(ui, crate::i18n::t("% Geschwindigkeit"), egui::Slider::new(percent, 25.0..=400.0));
        }
        Effect::MinimumLength { seconds } => {
            slider_row(ui, crate::i18n::t("Sekunden"), egui::Slider::new(seconds, 1.0..=600.0));
        }
        Effect::MaximumLength { percent, seconds } => {
            slider_row(ui, crate::i18n::t("Sekunden (0 = Prozent)"), egui::Slider::new(seconds, 0.0..=3600.0));
            ui.add_enabled_ui(*seconds == 0.0, |ui| slider_row(ui, crate::i18n::t("% des Originals"), egui::Slider::new(percent, 1.0..=100.0))).inner;
        }
        Effect::Lowpass { hz } | Effect::Highpass { hz } => {
            slider_row(ui, "Hz", egui::Slider::new(hz, 10.0..=22000.0).logarithmic(true));
        }
        Effect::Bandpass { low, high } => {
            slider_row(ui, crate::i18n::t("Untere Hz"), egui::Slider::new(low, 10.0..=22000.0).logarithmic(true));
            slider_row(ui, crate::i18n::t("Obere Hz"), egui::Slider::new(high, 10.0..=22000.0).logarithmic(true));
        }
        Effect::GraphicEq { gains } => {
            ui.horizontal(|ui| {
                let labels = ["60", "130", "260", "500", "1k", "2k", "4k", "8k", "16k"];
                for (i, g) in gains.iter_mut().enumerate() {
                    ui.vertical(|ui| {
                        ui.add(egui::Slider::new(g, -12.0..=12.0).vertical().show_value(false));
                        ui.label(RichText::new(labels[i]).size(11.0).color(p.muted));
                    });
                }
            });
            ui.horizontal_wrapped(|ui| {
                if ui.button(crate::i18n::t("Auf 0 dB")).clicked() {
                    *gains = [0.0; 9];
                }
                if ui.button(crate::i18n::t("Konzertsaal")).clicked() {
                    *gains = [4.0, 3.0, 1.0, 0.0, -1.0, 0.0, 2.0, 3.0, 4.0];
                }
                if ui.button(crate::i18n::t("Mehr Bass")).clicked() {
                    *gains = [8.0, 6.0, 4.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0];
                }
                if ui.button(crate::i18n::t("Mehr Höhen")).clicked() {
                    *gains = [0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 4.0, 6.0, 8.0];
                }
            });
        }
        Effect::Compressor { threshold_db, ratio, attack_ms, release_ms } => {
            slider_row(ui, crate::i18n::t("Schwelle dB"), egui::Slider::new(threshold_db, -40.0..=0.0));
            slider_row(ui, crate::i18n::t("Verhältnis 1:x"), egui::Slider::new(ratio, 1.0..=20.0));
            slider_row(ui, crate::i18n::t("Ansprechzeit ms"), egui::Slider::new(attack_ms, 0.1..=200.0));
            slider_row(ui, crate::i18n::t("Abklingzeit ms"), egui::Slider::new(release_ms, 10.0..=5000.0));
        }
        Effect::Karaoke { keep_voice } => {
            segmented(ui, p, keep_voice, &[(false, crate::i18n::t("Stimme entfernen (Karaoke)")), (true, crate::i18n::t("Instrumente entfernen"))]);
            ui.label(muted(p, crate::i18n::t("Funktioniert nur mit Stereo-Dateien.")));
        }
        Effect::PlaylistWriter { kind, same_folder } => {
            segmented(ui, p, kind, &[(0, "M3U8"), (1, "M3U"), (2, "PLS")]);
            ui.checkbox(same_folder, crate::i18n::t("Im selben Ordner wie die Titel"));
        }
        Effect::RunExternal { command } => {
            ui.add(egui::TextEdit::singleline(command).hint_text(crate::i18n::t("z. B. notepad [outfile]")).desired_width(f32::INFINITY));
            ui.label(muted(p, crate::i18n::t("[outfile] = neue Datei, [infile] = Quelle")));
        }
        Effect::ConditionalEncoding { skip_ext, copy_instead } => {
            ui.horizontal(|ui| {
                ui.label(crate::i18n::t("Wenn Quelle Endung hat:"));
                ui.add(egui::TextEdit::singleline(skip_ext).desired_width(80.0));
            });
            segmented(ui, p, copy_instead, &[(false, crate::i18n::t("Datei überspringen")), (true, crate::i18n::t("1:1 kopieren"))]);
        }
        Effect::MoveDestOnError { folder } => {
            let w = ui.available_width();
            folder_field(ui, p, folder, crate::i18n::t("Ordner wählen …"), w);
        }
        Effect::TagProcessing { field, value, mode } => {
            segmented(ui, p, mode, &[(0, crate::i18n::t("Setzen")), (2, crate::i18n::t("Setzen, wenn leer")), (1, crate::i18n::t("Entfernen"))]);
            egui::ComboBox::from_label(crate::i18n::t("Feld")).height(dropdown_height(ui)).selected_text(field.clone()).show_ui(ui, |ui| {
                for (k, n) in tags::FIELDS {
                    ui.selectable_value(field, k.to_string(), crate::i18n::t(n));
                }
                ui.selectable_value(field, "cover".to_string(), crate::i18n::t("Cover-Bild"));
                ui.selectable_value(field, "*".to_string(), crate::i18n::t("Alle Tags"));
            });
            if *mode != 1 {
                ui.add(egui::TextEdit::singleline(value).hint_text(crate::i18n::t("Wert")).desired_width(f32::INFINITY));
            }
        }
        Effect::Hdcd { amp_6db } => {
            ui.checkbox(amp_6db, crate::i18n::t("Um 6 dB anheben"));
            ui.label(muted(p, crate::i18n::t("Erkennt HDCDs (20-Bit-Audio-CDs) selbst und macht daraus 24 Bit. Normale CDs bleiben unverändert bei 16 Bit.")));
        }
        _ => {
            ui.label(muted(p, crate::i18n::t("Keine Einstellungen.")));
        }
    }
}

/// Add or remove the Explorer right-click entry for audio files (current user only).
pub fn context_menu(add: bool) -> String {
    #[cfg(windows)]
    {
        let Ok(exe) = std::env::current_exe() else { return crate::i18n::t("Programmpfad unbekannt").into() };
        let key = r"HKCU\Software\Classes\SystemFileAssociations\audio\shell\PsyAudio";
        let run = |args: &[&str]| {
            let mut c = std::process::Command::new("reg");
            c.args(args);
            audio::hidden(&mut c);
            c.status().map(|s| s.success()).unwrap_or(false)
        };
        if add {
            let cmd = format!("\"{}\" \"%1\"", exe.display());
            let icon = exe.display().to_string();
            let ok = run(&["add", key, "/ve", "/d", crate::i18n::t("Mit PsyAudio konvertieren"), "/f"])
                && run(&["add", key, "/v", "Icon", "/d", &icon, "/f"])
                && run(&["add", &format!(r"{key}\command"), "/ve", "/d", &cmd, "/f"]);
            if ok {
                crate::i18n::t("Kontextmenü eingerichtet").into()
            } else {
                crate::i18n::t("Kontextmenü konnte nicht eingerichtet werden").into()
            }
        } else if run(&["delete", key, "/f"]) {
            crate::i18n::t("Kontextmenü entfernt").into()
        } else {
            crate::i18n::t("Kein Kontextmenü vorhanden").into()
        }
    }
    #[cfg(target_os = "linux")]
    {
        let Some(data) = dirs::data_dir() else { return crate::i18n::t("Kontextmenü konnte nicht eingerichtet werden").into() };
        let menu = data.join("kio/servicemenus/psyaudio.desktop");
        let icon = data.join("PsyAudio/PsyAudio.png");
        if !add {
            let _ = std::fs::remove_file(&icon);
            return if std::fs::remove_file(&menu).is_ok() {
                crate::i18n::t("Kontextmenü entfernt").into()
            } else {
                crate::i18n::t("Kein Kontextmenü vorhanden").into()
            };
        }
        let Ok(exe) = std::env::current_exe() else { return crate::i18n::t("Programmpfad unbekannt").into() };
        match write_service_menu(&menu, &icon, &exe) {
            Ok(()) => crate::i18n::t("Kontextmenü eingerichtet").into(),
            Err(_) => crate::i18n::t("Kontextmenü konnte nicht eingerichtet werden").into(),
        }
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = add;
        crate::i18n::t("Nur unter Windows verfügbar").into()
    }
}

/// Dolphin service menu for audio files; Dolphin only offers local service menus that are executable.
#[cfg(target_os = "linux")]
fn write_service_menu(menu: &std::path::Path, icon: &std::path::Path, exe: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    for dir in [menu.parent(), icon.parent()].into_iter().flatten() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(icon, crate::APP_ICON_PNG)?;
    std::fs::write(menu, service_menu_text(exe, icon, crate::i18n::t("Mit PsyAudio konvertieren")))?;
    std::fs::set_permissions(menu, std::fs::Permissions::from_mode(0o755))
}

/// The `.desktop` text: one entry for audio files that hands the whole selection to PsyAudio.
#[cfg(target_os = "linux")]
pub(crate) fn service_menu_text(exe: &std::path::Path, icon: &std::path::Path, label: &str) -> String {
    // Exec quoting: inside double quotes, `"`, `` ` ``, `$` and `\` need a backslash; then the
    // string escape doubles every backslash once more, and `%` is written `%%`
    let quoted: String = exe.to_string_lossy().chars().flat_map(|c| if matches!(c, '"' | '`' | '$' | '\\') { vec!['\\', c] } else { vec![c] }).collect();
    let quoted = quoted.replace('\\', "\\\\").replace('%', "%%");
    format!(
        "[Desktop Entry]\nType=Service\nMimeType=audio/*;\nActions=psyaudio;\nX-KDE-Priority=TopLevel\n\n[Desktop Action psyaudio]\nName={label}\nIcon={}\nExec=\"{quoted}\" %F\n",
        icon.display()
    )
}

/// A slider with its label on its own line and the slider filling the width,
/// so long labels never push the panel wider than the window.
pub fn slider_row(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>, slider: egui::Slider<'_>) -> egui::Response {
    ui.add(egui::Label::new(text).wrap());
    ui.spacing_mut().slider_width = (ui.available_width() - 120.0).max(60.0);
    ui.add(slider)
}

/// A slider over a list of bit rates: the value box shows "192 kbit/s" and accepts a typed rate.
fn rate_slider<'a>(idx: &'a mut usize, rates: &'static [u32]) -> egui::Slider<'a> {
    egui::Slider::new(idx, 0..=rates.len() - 1)
        .custom_formatter(move |v, _| format!("{} kbit/s", rates[(v as usize).min(rates.len() - 1)]))
        .custom_parser(move |s| {
            let n: f64 = s.trim().trim_end_matches("kbit/s").trim().parse().ok()?;
            // the nearest offered rate
            rates.iter().enumerate().min_by_key(|(_, r)| (**r as f64 - n).abs() as u64).map(|(i, _)| i as f64)
        })
}

/// How tall a dropdown list may open: all the room down to the window's bottom edge,
/// so long lists (formats, fields) show completely instead of in a short scrolling box.
pub fn dropdown_height(ui: &egui::Ui) -> f32 {
    (ui.ctx().screen_rect().bottom() - ui.cursor().top() - 70.0).max(220.0)
}

/// A folder box: shows the folder, and a click anywhere on it opens the Windows folder dialog.
/// Returns true when a new folder was chosen.
pub fn folder_field(ui: &mut egui::Ui, p: &Palette, value: &mut String, hint: &str, width: f32) -> bool {
    let width = width.max(80.0);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, 28.0), egui::Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(crate::i18n::t("Klicken, um einen Ordner im Explorer auszuwählen"));
    let painter = ui.painter_at(rect);
    let (fill, stroke) = if resp.hovered() { (p.border, p.accent) } else { (p.card2, p.border) };
    painter.rect_filled(rect, egui::CornerRadius::same(theme::R_CTRL), fill);
    painter.rect_stroke(rect, egui::CornerRadius::same(theme::R_CTRL), egui::Stroke::new(1.0_f32, stroke), egui::StrokeKind::Inside);
    painter.text(rect.left_center() + egui::vec2(10.0, 0.0), egui::Align2::LEFT_CENTER, ph::FOLDER, egui::FontId::proportional(14.0), p.accent);
    let (text, col) = if value.trim().is_empty() { (hint.to_string(), p.muted) } else { (value.clone(), p.text) };
    // keep the end of long paths visible (that's the part that differs)
    let max_chars = ((width - 70.0) / 7.2).max(8.0) as usize;
    let shown = if text.chars().count() > max_chars {
        let tail: String = text.chars().rev().take(max_chars.saturating_sub(1)).collect::<Vec<_>>().into_iter().rev().collect();
        format!("…{tail}")
    } else {
        text
    };
    painter.text(rect.left_center() + egui::vec2(32.0, 0.0), egui::Align2::LEFT_CENTER, shown, egui::FontId::proportional(14.0), col);
    painter.text(rect.right_center() - egui::vec2(10.0, 0.0), egui::Align2::RIGHT_CENTER, crate::i18n::t("Ändern"), egui::FontId::proportional(12.0), p.muted);
    if resp.clicked() {
        if let Some(f) = pick_folder(value) {
            *value = f;
            return true;
        }
    }
    false
}

/// Width of the settings column on the right: a third of the page, at least 440 px, at most 560 px.
pub fn side_panel_width(ui: &egui::Ui) -> f32 {
    (ui.available_width() * 0.36).clamp(440.0, 560.0)
}

#[cfg(all(test, target_os = "linux"))]
mod service_menu_tests {
    use super::service_menu_text;
    use std::path::Path;

    #[test]
    fn hands_the_selection_to_psyaudio() {
        let t = service_menu_text(Path::new("/opt/Psy Audio/PsyAudio"), Path::new("/tmp/i.png"), "Convert");
        assert!(t.contains("MimeType=audio/*;"));
        assert!(t.contains("Name=Convert\n"));
        assert!(t.contains("Exec=\"/opt/Psy Audio/PsyAudio\" %F\n"));
    }

    #[test]
    fn special_characters_in_the_path_are_escaped() {
        let t = service_menu_text(Path::new("/a$b/50%\"x"), Path::new("/i.png"), "C");
        assert!(t.contains(r#"Exec="/a\\$b/50%%\\"x" %F"#), "{t}");
    }
}
