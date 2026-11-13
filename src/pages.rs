//! Batch converter, CD ripper, tag editor and settings pages.

use crate::app::{ffmpeg_state, Page, PsyApp, RipMsg, RipRun};
use crate::audio::{self, fmt_len};
use crate::cdrip::{self, DiscMeta, Pressing, Toc};
use crate::convert::{self, Job, Msg, Plan};
use crate::encode::{self, Codec};
use crate::naming;
use crate::settings::{OutputMode, RipMode, RipSettings, Settings};
use crate::tags::{self, Tags};
use crate::theme;
use crate::widgets::*;
use eframe::egui::{self, Color32, RichText};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;

/// One-word rip status for the track list (the full wording is in the tooltip).
fn status_short(st: &str, p: &theme::Palette) -> (String, Color32) {
    if st.starts_with(crate::i18n::t("Fehler")) {
        (icon_text(ph::X, crate::i18n::t("Fehler")), p.bad)
    } else if st.starts_with(crate::i18n::t("Unsicher")) {
        (icon_text(ph::WARNING, crate::i18n::t("Unsicher")), p.bad)
    } else if st.starts_with(crate::i18n::t("Sicher (Warnung")) {
        (icon_text(ph::CHECK, crate::i18n::t("Sicher*")), p.warn)
    } else if st.starts_with(crate::i18n::t("Sicher")) {
        (icon_text(ph::CHECK, crate::i18n::t("Sicher")), p.good)
    } else if st.starts_with(crate::i18n::t("Gerippt")) {
        (icon_text(ph::CHECK, crate::i18n::t("Gerippt")), p.good)
    } else {
        (st.chars().take(22).collect(), p.muted)
    }
}

/// Short AccurateRip result: "AR" with a check and the confidence, a cross on no match, "AR –" when not in the database.
fn ar_short(ar: &str, p: &theme::Palette) -> Option<(String, Color32)> {
    if ar.is_empty() {
        return None;
    }
    let conf: String = ar
        .split(crate::i18n::t("Vertrauen "))
        .nth(1)
        .map(|r| r.chars().take_while(|c| c.is_ascii_digit()).collect())
        .unwrap_or_default();
    Some(if ar.contains(crate::i18n::t(": Genau")) {
        (format!("AR {} {conf}", ph::CHECK).trim_end().to_string(), p.good)
    } else if ar.contains(crate::i18n::t(": Ungenau")) {
        (format!("AR {} {conf}", ph::X).trim_end().to_string(), p.warn)
    } else {
        ("AR –".into(), p.muted)
    })
}

impl PsyApp {
    /// Look up this CD in AccurateRip and find the drive's read offset in the background.
    pub(crate) fn start_offset_search(&mut self) {
        let (Some(toc), Some(letter)) = (self.cd.toc.clone(), self.cd.drive) else { return };
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let r = (|| {
                let db = cdrip::accuraterip_lookup_full(&toc)?;
                let drive = cdrip::Drive::open(letter)?;
                let cancel = AtomicBool::new(false);
                crate::cdrip_read::find_offset(&drive, &toc, &db, &cancel, &|_| {})
            })();
            let _ = tx.send(r);
        });
        self.cd.offset_rx = Some(rx);
    }

    /// Full path the first selected track would be saved to, using this disc's titles
    /// (or sample titles when no CD has been read yet).
    pub(crate) fn rip_example_path(&self) -> String {
        let sample = DiscMeta {
            artist: crate::i18n::t("Interpret").into(),
            album: "Album".into(),
            year: "2024".into(),
            disc: 1,
            disc_total: 1,
            tracks: vec![(crate::i18n::t("Titel").into(), String::new(), String::new())],
            ..Default::default()
        };
        let meta = if self.cd.toc.is_some() && !self.cd.meta.album.trim().is_empty() { &self.cd.meta } else { &sample };
        let i = self.cd.selected.iter().position(|s| *s).unwrap_or(0).min(meta.tracks.len().saturating_sub(1));
        let (title, artist, _) = meta.tracks.get(i).cloned().unwrap_or_default();
        let artist = if artist.trim().is_empty() { meta.artist.clone() } else { artist };
        let total = meta.tracks.len().max(1);
        let tg = cdrip::tags_for_track(meta, (i + 1) as u8, total, &title, &artist);
        let mut plan = self.plan(self.s.rip_codec);
        plan.output_mode = OutputMode::SingleFolder;
        plan.output_folder = PathBuf::from(&self.s.rip_folder);
        plan.scheme = self.s.rip.name_scheme.clone();
        let codec = plan.outputs().first().copied().unwrap_or(Codec::Flac);
        let src = PathBuf::from(format!("Track{:02}.wav", i + 1));
        plan.destination(&src, None, &tg, &tags::Info::default(), codec).display().to_string()
    }

    // ---------------- batch ----------------

    pub(crate) fn batch_page(&mut self, ui: &mut egui::Ui) {
        let p = self.pal;
        self.header(ui, crate::i18n::t("Stapel-Konverter"), crate::i18n::t("Ganze Ordner mit Unterordnern auf einmal umwandeln – die Ordnerstruktur bleibt erhalten."));
        theme::card(&p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            section(ui, &p, crate::i18n::t("Quellordner"));
            let w = ui.available_width();
            folder_field(ui, &p, &mut self.batch_folder, crate::i18n::t("Klicken, um einen Ordner zu wählen (oder hierher ziehen)"), w);
            ui.checkbox(&mut self.s.batch_subfolders, crate::i18n::t("Unterordner einbeziehen"));
            section(ui, &p, crate::i18n::t("Dateitypen"));
            ui.horizontal_wrapped(|ui| {
                for ext in ["mp3", "flac", "wav", "m4a", "aac", "ogg", "opus", "wma", "aiff", "ape", "wv", "mpc", "dsf", "dff", "spx", "caf"] {
                    let mut on = self.s.batch_extensions.iter().any(|e| e == ext);
                    if ui.toggle_value(&mut on, ext).changed() {
                        if on {
                            self.s.batch_extensions.push(ext.into());
                        } else {
                            self.s.batch_extensions.retain(|e| e != ext);
                        }
                    }
                }
            });
            ui.add_space(6.0);
            // the folder walk runs in the background, so big trees do not freeze the window
            if let Some(rx) = &self.batch_rx {
                match rx.try_recv() {
                    Ok((root, found)) => {
                        self.batch_root = root;
                        self.batch_found = found;
                        self.batch_rx = None;
                        let n = self.batch_found.len();
                        self.notify(crate::i18n::tf(crate::i18n::t("{n} Dateien gefunden"), &[("n", format!("{n}"))]));
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => self.batch_rx = None,
                    Err(std::sync::mpsc::TryRecvError::Empty) => ui.ctx().request_repaint_after(std::time::Duration::from_millis(100)),
                }
            }
            ui.horizontal(|ui| {
                let scanning = self.batch_rx.is_some();
                if ui.add_enabled(!scanning, theme::primary(&p, &icon_text(ph::MAGNIFYING_GLASS, crate::i18n::t("Dateien suchen")))).clicked() {
                    let root = PathBuf::from(self.batch_folder.trim());
                    let (sub, exts) = (self.s.batch_subfolders, self.s.batch_extensions.clone());
                    let (tx, rx) = channel();
                    std::thread::spawn(move || {
                        let found = crate::convert::scan_folder(&root, sub, &exts);
                        let _ = tx.send((root, found));
                    });
                    self.batch_rx = Some(rx);
                }
                if scanning {
                    ui.spinner();
                    ui.label(muted(&p, crate::i18n::t("Suche läuft…")));
                }
            });
        });
        ui.add_space(10.0);
        theme::card(&p).show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            section(ui, &p, &crate::i18n::tf(crate::i18n::t("Gefunden: {0} Dateien"), &[("0", format!("{}", self.batch_found.len()))]));
            ui.label(muted(
                &p,
                crate::i18n::tf(crate::i18n::t("Ziel: {0}  ·  Format: {1}  (Einstellungen und Effekte wie auf der Seite „Konvertieren“)"), &[("0", format!("{}", self.s.output_folder)), ("1", format!("{}", self.s.codec.name()))]),
            ));
            ui.horizontal(|ui| {
                if ui.add_enabled(!self.batch_found.is_empty(), theme::primary(&p, &icon_text(ph::PLAY, crate::i18n::t("Mit Ordnerstruktur umwandeln")))).clicked() {
                    let root = self.batch_root.clone();
                    let items = self.batch_found.iter().map(|f| (f.clone(), Some(root.clone()))).collect();
                    let codec = self.s.codec;
                    self.start_conversion_with(items, codec, true);
                }
                if ui.add_enabled(!self.batch_found.is_empty(), theme::ghost(crate::i18n::t("Zur Konverter-Liste"))).clicked() {
                    let root = self.batch_root.clone();
                    let found = self.batch_found.clone();
                    self.add_paths(found, Some(root));
                    self.page = Page::Convert;
                }
            });
            ui.separator();
            let folder = self.batch_root.clone();
            egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, 20.0, self.batch_found.len(), |ui, range| {
                for i in range {
                    let f = &self.batch_found[i];
                    let rel = f.strip_prefix(&folder).unwrap_or(f);
                    ui.label(RichText::new(rel.display().to_string()).size(12.5).color(p.muted));
                }
            });
        });
    }

    // ---------------- CD ripper ----------------

    fn read_disc(&mut self) {
        self.cd.toc = None;
        self.cd.error.clear();
        self.cd.ar = None;
        self.cd.candidates.clear();
        self.cd.cover = None;
        self.cd.cover_tex = None;
        self.cd.rip = None;
        let Some(letter) = self.cd.drive else {
            self.cd.error = crate::i18n::t("Kein CD-Laufwerk gefunden").into();
            return;
        };
        match cdrip::Drive::open(letter).and_then(|d| d.toc()) {
            Ok(toc) => {
                let audio = toc.audio_tracks();
                self.cd.selected = vec![true; audio.len()];
                let mut meta = DiscMeta { disc: 1, disc_total: 1, ..Default::default() };
                meta.tracks = audio.iter().map(|t| (crate::i18n::tf(crate::i18n::t("Titel {0}"), &[("0", format!("{:02}", t.number))]), String::new(), String::new())).collect();
                self.cd.meta = meta;
                self.cd.toc = Some(toc.clone());
                self.lookup_disc();
                if self.s.rip.accuraterip {
                    let (tx, rx) = channel();
                    std::thread::spawn(move || {
                        let _ = tx.send(cdrip::accuraterip_lookup(&toc));
                    });
                    self.cd.ar_rx = Some(rx);
                }
            }
            Err(e) => self.cd.error = e,
        }
    }

    fn lookup_disc(&mut self) {
        let Some(toc) = self.cd.toc.clone() else { return };
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let r = cdrip::musicbrainz_lookup(&toc).map(|c| {
                let cover = c.first().and_then(|m| cdrip::cover_art(&m.release_id));
                (c, cover)
            });
            let _ = tx.send(r);
        });
        self.cd.lookup = Some(rx);
    }

    pub(crate) fn cd_page(&mut self, ui: &mut egui::Ui) {
        let p = self.pal;
        self.header_with_side(ui, "CD-Ripper", crate::i18n::t("Audio-CDs sicher auslesen, mit AccurateRip prüfen und Titel-Infos von MusicBrainz holen."));
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("drive").height(dropdown_height(ui))
                .selected_text(self.cd.drive.map(|d| crate::i18n::tf(crate::i18n::t("Laufwerk {d}"), &[("d", cdrip::drive_name(d))])).unwrap_or_else(|| crate::i18n::t("Kein Laufwerk").into()))
                .show_ui(ui, |ui| {
                    for d in self.cd.drives.clone() {
                        ui.selectable_value(&mut self.cd.drive, Some(d), crate::i18n::tf(crate::i18n::t("Laufwerk {d}"), &[("d", cdrip::drive_name(d))]));
                    }
                });
            // while a rip or the offset search reads the drive, it is not read again or ejected
            let drive_busy = self.cd.rip.as_ref().is_some_and(|r| r.finished.is_none()) || self.cd.offset_rx.is_some();
            if ui.add_enabled(!drive_busy, theme::ghost(&icon_text(ph::ARROW_CLOCKWISE, crate::i18n::t("CD lesen")))).on_disabled_hover_text(crate::i18n::t("Erst das Rippen abwarten oder abbrechen")).clicked() {
                self.cd.drives = cdrip::drives();
                if self.cd.drive.is_none() {
                    self.cd.drive = self.cd.drives.first().copied();
                }
                self.read_disc();
            }
            if ui.add_enabled(!drive_busy, theme::ghost(&icon_text(ph::EJECT, crate::i18n::t("Auswerfen")))).on_disabled_hover_text(crate::i18n::t("Erst das Rippen abwarten oder abbrechen")).clicked() {
                if let Some(d) = self.cd.drive {
                    match cdrip::Drive::open(d).and_then(|dr| dr.eject()) {
                        Ok(()) => {
                            self.cd.toc = None;
                        }
                        Err(e) => self.notify(e),
                    }
                }
            }
            if ui.add_enabled(self.cd.toc.is_some() && self.cd.lookup.is_none(), theme::ghost(&icon_text(ph::GLOBE, crate::i18n::t("Titel-Infos suchen")))).clicked() {
                self.lookup_disc();
            }
            if self.cd.lookup.is_some() {
                ui.spinner();
                ui.label(muted(&p, crate::i18n::t("Suche bei MusicBrainz…")));
            }
        });
        if !self.cd.error.is_empty() {
            ui.label(RichText::new(&self.cd.error).color(p.warn));
        }
        ui.add_space(6.0);

        if self.s.side_open {
            let panel = egui::SidePanel::right("rip_opts")
                .show_separator_line(false)
                .exact_width(side_panel_width(ui))
                .resizable(false)
                .frame(egui::Frame::new().inner_margin(egui::Margin { left: 16, right: 0, top: 0, bottom: 0 }))
                .show_inside(ui, |ui| {
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        // room for the scroll bar, so nothing is drawn under or past it
                        ui.set_max_width(ui.available_width() - 14.0);
                        self.rip_options(ui)
                    });
                });
            self.side_handle(ui, panel.response.rect);
        }

        let Some(toc) = self.cd.toc.clone() else {
            theme::card(&p).show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                ui.vertical_centered(|ui| {
                    ui.add_space((ui.available_height() * 0.3).max(10.0));
                    ui.label(RichText::new(ph::DISC).size(48.0).color(p.accent));
                    ui.label(RichText::new(crate::i18n::t("Audio-CD einlegen und „CD lesen“ klicken")).size(18.0).strong());
                    if self.cd.drives.is_empty() {
                        ui.label(muted(&p, crate::i18n::t("Es wurde kein CD-Laufwerk gefunden.")));
                    } else {
                        ui.label(muted(&p, crate::i18n::tf(crate::i18n::t("{0} Laufwerk(e) gefunden"), &[("0", format!("{}", self.cd.drives.len()))])));
                    }
                });
            });
            return;
        };

        theme::card(&p).show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            ui.horizontal(|ui| {
                if self.cd.cover_tex.is_none() {
                    if let Some(c) = &self.cd.cover {
                        self.cd.cover_tex = image_texture(ui.ctx(), "cdcover", c);
                    }
                }
                match &self.cd.cover_tex {
                    Some(t) => {
                        ui.add(egui::Image::new(t).fit_to_exact_size(egui::vec2(120.0, 120.0)).corner_radius(10));
                    }
                    None => cover_placeholder(ui, &p, 120.0),
                }
                ui.vertical(|ui| {
                    egui::Grid::new("album").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                        ui.label(muted(&p, "Album"));
                        ui.add(egui::TextEdit::singleline(&mut self.cd.meta.album).desired_width(320.0));
                        ui.end_row();
                        ui.label(muted(&p, crate::i18n::t("Interpret")));
                        ui.add(egui::TextEdit::singleline(&mut self.cd.meta.artist).desired_width(320.0));
                        ui.end_row();
                        ui.label(muted(&p, crate::i18n::t("Jahr / Genre")));
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut self.cd.meta.year).desired_width(60.0));
                            ui.add(egui::TextEdit::singleline(&mut self.cd.meta.genre).hint_text("Genre").desired_width(250.0));
                        });
                        ui.end_row();
                        ui.label(muted(&p, "Disc"));
                        ui.horizontal(|ui| {
                            ui.add(egui::DragValue::new(&mut self.cd.meta.disc).range(1..=99));
                            ui.label(crate::i18n::t("von"));
                            ui.add(egui::DragValue::new(&mut self.cd.meta.disc_total).range(1..=99));
                        });
                        ui.end_row();
                    });
                    if self.cd.candidates.len() > 1 {
                        let mut pick = None;
                        egui::ComboBox::from_id_salt("cands").height(dropdown_height(ui)).selected_text(crate::i18n::tf(crate::i18n::t("{0} Treffer"), &[("0", format!("{}", self.cd.candidates.len()))])).show_ui(ui, |ui| {
                            for (i, c) in self.cd.candidates.iter().enumerate() {
                                if ui.selectable_label(false, format!("{} – {} ({})", c.artist, c.album, c.year)).clicked() {
                                    pick = Some(i);
                                }
                            }
                        });
                        if let Some(i) = pick {
                            self.cd.meta = self.cd.candidates[i].clone();
                        }
                    }
                    let ids = crate::i18n::tf(crate::i18n::t("{0} Titel · {1} · MusicBrainz {2} · freedb {3}"), &[("0", format!("{}", toc.audio_tracks().len())), ("1", format!("{}", fmt_len(toc.audio_leadout() as f64 / 75.0))), ("2", format!("{}", cdrip::musicbrainz_id(&toc))), ("3", format!("{:08x}", cdrip::freedb_id(&toc)))]);
                    ui.label(RichText::new(ids).size(11.0).color(p.muted));
                    let ar = match &self.cd.ar {
                        None if self.cd.ar_rx.is_some() => (crate::i18n::t("AccurateRip: wird abgefragt…").to_string(), p.muted),
                        None => (crate::i18n::t("AccurateRip: nicht abgefragt").to_string(), p.muted),
                        Some(Ok(v)) => (crate::i18n::tf(crate::i18n::t("AccurateRip: CD in der Datenbank ({0} Pressungen)"), &[("0", format!("{}", v.len()))]), p.good),
                        Some(Err(e)) => (format!("AccurateRip: {e}"), p.warn),
                    };
                    ui.label(RichText::new(ar.0).size(11.5).color(ar.1));
                });
            });
            ui.separator();
            let audio: Vec<cdrip::TocTrack> = toc.audio_tracks().into_iter().cloned().collect();
            let rip_rows = self.cd.rip.as_ref().map(|r| r.rows.clone());
            let artist_hint = self.cd.meta.artist.clone();
            egui::ScrollArea::vertical().auto_shrink([false, false]).max_height((ui.available_height() - 56.0).max(80.0)).show(ui, |ui| {
                // fixed columns for #, length and status; title and artist share the rest
                let (w_check, w_num, w_len, w_status, gap) = (26.0, 30.0, 52.0, 200.0, 8.0);
                ui.spacing_mut().item_spacing.y = 3.0;
                let free = (ui.available_width() - w_check - w_num - w_len - w_status - gap * 5.0).max(200.0);
                let (w_title, w_artist) = (free * 0.6, free * 0.4);
                let header = |ui: &mut egui::Ui, w: f32, t: &str| {
                    ui.add_sized([w, 18.0], egui::Label::new(muted(&p, t)).halign(egui::Align::Min));
                };
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    header(ui, w_check, "");
                    header(ui, w_num, "#");
                    header(ui, w_title, crate::i18n::t("Titel"));
                    header(ui, w_artist, crate::i18n::t("Interpret"));
                    header(ui, w_len, crate::i18n::t("Länge"));
                    header(ui, w_status, "Status");
                });
                for (i, t) in audio.iter().enumerate() {
                    if i >= self.cd.selected.len() || i >= self.cd.meta.tracks.len() {
                        break;
                    }
                    let fill = if i % 2 == 0 { p.card2 } else { p.card };
                    egui::Frame::new().fill(fill).corner_radius(egui::CornerRadius::same(6)).inner_margin(egui::Margin::symmetric(0, 3)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = gap;
                            ui.add_sized([w_check, 22.0], egui::Checkbox::without_text(&mut self.cd.selected[i]));
                            ui.add_sized([w_num, 22.0], egui::Label::new(format!("{:02}", t.number)));
                            ui.add_sized([w_title, 22.0], egui::TextEdit::singleline(&mut self.cd.meta.tracks[i].0));
                            ui.add_sized([w_artist, 22.0], egui::TextEdit::singleline(&mut self.cd.meta.tracks[i].1).hint_text(artist_hint.clone()));
                            let (s, e) = toc.span(t.number).unwrap_or((0, 0));
                            ui.add_sized([w_len, 22.0], egui::Label::new(fmt_len(e.saturating_sub(s) as f64 / 75.0)));
                            // status: always one line of fixed height; the full wording shows on hover
                            let (srect, sresp) = ui.allocate_exact_size(egui::vec2(w_status, 22.0), egui::Sense::hover());
                            if let Some((f, st, ar)) = rip_rows.as_ref().and_then(|r| r.get(i)).filter(|r| !r.1.is_empty()) {
                                let painter = ui.painter_at(srect);
                                let font = egui::FontId::proportional(12.5);
                                if *f < 1.0 {
                                    // reading / encoding: thin bar under a short label
                                    let bar = egui::Rect::from_min_size(srect.left_bottom() - egui::vec2(0.0, 4.0), egui::vec2(srect.width() - 8.0, 3.0));
                                    painter.rect_filled(bar, egui::CornerRadius::same(2), p.border);
                                    let done = egui::Rect::from_min_size(bar.min, egui::vec2(bar.width() * f.clamp(0.0, 1.0), bar.height()));
                                    painter.rect_filled(done, egui::CornerRadius::same(2), p.accent);
                                    painter.text(srect.left_center() - egui::vec2(0.0, 2.0), egui::Align2::LEFT_CENTER, st, font.clone(), p.muted);
                                } else {
                                    let (short, col) = status_short(st, &p);
                                    let g = painter.layout_no_wrap(short, font.clone(), col);
                                    let w = g.size().x;
                                    painter.galley(srect.left_center() - egui::vec2(0.0, g.size().y / 2.0), g, col);
                                    if let Some((ar_short, ar_col)) = ar_short(ar, &p) {
                                        painter.text(
                                            srect.left_center() + egui::vec2(w + 10.0, 0.0),
                                            egui::Align2::LEFT_CENTER,
                                            ar_short,
                                            font,
                                            ar_col,
                                        );
                                    }
                                }
                                let full = if ar.is_empty() { st.clone() } else { format!("{st}\n{ar}") };
                                sresp.on_hover_text(full);
                            }
                        });
                    });
                }
            });
            ui.horizontal(|ui| {
                let running = self.cd.rip.as_ref().map(|r| r.finished.is_none()).unwrap_or(false);
                if running {
                    if ui.add(theme::ghost(&icon_text(ph::STOP, crate::i18n::t("Abbrechen")))).clicked() {
                        if let Some(r) = &self.cd.rip {
                            r.cancel.store(true, Ordering::Relaxed);
                        }
                    }
                    ui.spinner();
                    ui.label(muted(&p, crate::i18n::t("Rippen läuft…")));
                } else {
                    let n = self.cd.selected.iter().filter(|x| **x).count();
                    // not while the offset search reads the same drive
                    let searching = self.cd.offset_rx.is_some();
                    if ui
                        .add_enabled(n > 0 && !searching, theme::primary(&p, &icon_text(ph::DISC, &crate::i18n::tf(crate::i18n::t("{n} Titel rippen"), &[("n", format!("{n}"))]))))
                        .on_disabled_hover_text(crate::i18n::t("Erst die Versatz-Suche abwarten"))
                        .clicked()
                    {
                        self.start_rip();
                    }
                    if let Some(r) = &self.cd.rip {
                        if let Some(s) = &r.finished {
                            ui.label(RichText::new(s).color(p.good));
                        }
                        for l in &r.log {
                            ui.label(muted(&p, l.clone()));
                        }
                    }
                }
            });
        });
    }

    fn rip_options(&mut self, ui: &mut egui::Ui) {
        let p = self.pal;
        theme::card(&p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            section(ui, &p, "Format");
            egui::ComboBox::from_id_salt("ripcodec").height(dropdown_height(ui)).selected_text(self.s.rip_codec.name()).width(ui.available_width() - 36.0).show_ui(ui, |ui| {
                for c in encode::AUDIO_CODECS {
                    ui.selectable_value(&mut self.s.rip_codec, *c, c.name());
                }
                ui.selectable_value(&mut self.s.rip_codec, Codec::Multi, Codec::Multi.name());
            });
            let c = self.s.rip_codec;
            codec_settings(ui, &p, c, &mut self.s.enc);
            if let Some(m) = plan_requirement(c, &self.s.enc, &self.env()) {
                ui.label(RichText::new(format!("{}  {m}", ph::WARNING)).color(p.warn));
            }
        });
        ui.add_space(10.0);
        theme::card(&p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            section(ui, &p, crate::i18n::t("Speichern in"));
            let w = ui.available_width();
            folder_field(ui, &p, &mut self.s.rip_folder, crate::i18n::t("Ordner wählen …"), w);
            ui.add_space(4.0);
            scheme_editor(ui, &p, &mut self.s.rip.name_scheme, &naming::DATA[1..], true, naming::CD_SCHEME);
            // what the first track's file will be called, with this disc's (or sample) titles
            let example = self.rip_example_path();
            ui.label(RichText::new(crate::i18n::t("So wird gespeichert:")).size(11.5).color(p.muted));
            ui.add(egui::Label::new(RichText::new(example).size(12.0).color(p.accent2)).wrap());
        });
        ui.add_space(10.0);
        theme::card(&p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            section(ui, &p, crate::i18n::t("Lesemodus"));
            segmented(ui, &p, &mut self.s.rip.mode, &[(RipMode::Secure, crate::i18n::t("Sicher")), (RipMode::Burst, crate::i18n::t("Schnell (Burst)"))]);
            ui.label(muted(
                &p,
                if self.s.rip.mode == RipMode::Secure {
                    crate::i18n::t("Liest jeden Titel in einem Zug. Bestätigt AccurateRip das Ergebnis, ist der Titel fertig; sonst folgt ein zweiter Durchgang, und nur abweichende Stellen werden wiederholt.")
                } else {
                    crate::i18n::t("Schnellstes Rippen ohne Fehlerkorrektur – AccurateRip zeigt trotzdem, ob alles stimmt.")
                },
            ));
            let secure = self.s.rip.mode == RipMode::Secure;
            ui.add_enabled_ui(secure, |ui| {
                slider_row(ui, crate::i18n::t("Max. Neu-Lesungen fehlerhafter Stellen"), egui::Slider::new(&mut self.s.rip.reread_limit, 1..=100));
                ui.checkbox(&mut self.s.rip.extra_passes, crate::i18n::t("Zusätzliche Kontroll-Durchgänge"));
                if self.s.rip.extra_passes {
                    slider_row(ui, crate::i18n::t("Mindestens Durchgänge"), egui::Slider::new(&mut self.s.rip.extra_min, 1..=20));
                    slider_row(ui, crate::i18n::t("Höchstens Durchgänge"), egui::Slider::new(&mut self.s.rip.extra_max, 1..=20));
                    slider_row(ui, crate::i18n::t("Ende nach sauberen Durchgängen"), egui::Slider::new(&mut self.s.rip.extra_clean, 1..=10));
                }
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("Laufwerks-Cache"));
                    ui.add(egui::DragValue::new(&mut self.s.rip.cache_kb).range(0..=16384).suffix(" KB"));
                });
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("Abbrechen nach"));
                    ui.add(egui::DragValue::new(&mut self.s.rip.abort_unrecoverable).range(0..=100000));
                    ui.label(if self.s.rip.abort_unrecoverable == 0 { crate::i18n::t("(nie)") } else { crate::i18n::t("unlesbaren Sektoren") });
                });
                ui.checkbox(&mut self.s.rip.interpolate, crate::i18n::t("Unlesbare Stellen glätten (interpolieren)"));
                ui.label(crate::i18n::t("Titel als Fehler markieren, wenn"));
                egui::ComboBox::from_id_salt("markerror").height(dropdown_height(ui))
                    .selected_text([crate::i18n::t("(aus)"), crate::i18n::t("unsicher"), crate::i18n::t("nicht von AccurateRip bestätigt")][self.s.rip.mark_error as usize % 3])
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.s.rip.mark_error, 0, crate::i18n::t("(aus)"));
                        ui.selectable_value(&mut self.s.rip.mark_error, 1, crate::i18n::t("unsicher"));
                        ui.selectable_value(&mut self.s.rip.mark_error, 2, crate::i18n::t("nicht von AccurateRip bestätigt"));
                    });
            });
            ui.horizontal(|ui| {
                ui.label(crate::i18n::t("Laufwerks-Versatz"));
                ui.add(egui::DragValue::new(&mut self.s.rip.drive_offset).range(-3000..=3000).suffix(" Samples"));
            });
            // automatic search with this CD and the AccurateRip database
            let busy = self.cd.offset_rx.is_some() || self.cd.rip.as_ref().map(|r| r.finished.is_none()).unwrap_or(false);
            let ready = self.cd.toc.is_some() && self.cd.drive.is_some() && matches!(self.cd.ar, Some(Ok(_)));
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(ready && !busy, egui::Button::new(icon_text(ph::MAGNIFYING_GLASS, crate::i18n::t("Versatz automatisch finden"))))
                    .on_hover_text(crate::i18n::t("Liest ein kurzes Stück jedes Titels und vergleicht es mit AccurateRip. Braucht eine CD, die in der AccurateRip-Datenbank ist."))
                    .on_disabled_hover_text(crate::i18n::t("Zuerst eine CD lesen, die in der AccurateRip-Datenbank ist."))
                    .clicked()
                {
                    self.start_offset_search();
                }
                if self.cd.offset_rx.is_some() {
                    ui.spinner();
                    ui.label(muted(&p, crate::i18n::t("suche…")));
                }
            });
            ui.checkbox(&mut self.s.rip.accuraterip, crate::i18n::t("Mit AccurateRip prüfen"));
            ui.checkbox(&mut self.s.rip.cancel_on_insecure, crate::i18n::t("Abbrechen nach unsicherem Titel"));
            section(ui, &p, "Extras");
            ui.checkbox(&mut self.s.rip.embed_art, crate::i18n::t("Cover in Dateien einbetten"));
            ui.checkbox(&mut self.s.rip.folder_jpg, crate::i18n::t("folder.jpg speichern"));
            ui.checkbox(&mut self.s.rip.write_log, crate::i18n::t("Protokoll (.log) schreiben"));
            ui.checkbox(&mut self.s.rip.write_cue, crate::i18n::t("CUE-Datei schreiben"));
            ui.checkbox(&mut self.s.rip.eject_after, crate::i18n::t("CD danach auswerfen"));
            ui.label(muted(&p, crate::i18n::t("Effekte von der Seite „Konvertieren“ werden ebenfalls angewendet.")));
        });
    }

    fn start_rip(&mut self) {
        let Some(toc) = self.cd.toc.clone() else { return };
        let Some(letter) = self.cd.drive else { return };
        let codec = self.s.rip_codec;
        if let Some(m) = plan_requirement(codec, &self.s.enc, &self.env()) {
            self.notify(m);
            return;
        }
        let mut plan = self.plan(codec);
        plan.output_mode = OutputMode::SingleFolder;
        plan.output_folder = PathBuf::from(&self.s.rip_folder);
        // check the output drive before starting
        if let Some(m) = crate::convert::output_location_problem(&plan.output_folder) {
            self.notify(m);
            return;
        }
        plan.scheme = self.s.rip.name_scheme.clone();
        plan.overwrite = true;
        plan.copy_art = self.s.rip.embed_art;
        plan.copy_tags = true;
        plan.threads = 1;
        let audio: Vec<cdrip::TocTrack> = toc.audio_tracks().into_iter().cloned().collect();
        let meta = self.cd.meta.clone();
        let total = audio.len();
        let mut tracks = Vec::new();
        for (i, t) in audio.iter().enumerate() {
            if !self.cd.selected.get(i).copied().unwrap_or(false) {
                continue;
            }
            let (title, artist, isrc) = meta.tracks.get(i).cloned().unwrap_or_default();
            let artist = if artist.trim().is_empty() { meta.artist.clone() } else { artist };
            let mut tg = cdrip::tags_for_track(&meta, t.number, total, &title, &artist);
            tg.set("isrc", isrc);
            if let Some(c) = &self.cd.cover {
                if self.s.rip.embed_art {
                    tg.set_front_cover(tags::Art::new(c.clone(), tags::FRONT));
                }
            }
            tracks.push((i, t.number, tg));
        }
        let rs = self.s.rip.clone();
        // no AccurateRip data: say why in every row instead of "not in the database"
        let ar_db = match self.cd.ar.clone() {
            Some(r) => r,
            None if self.cd.ar_rx.is_some() => Err(crate::i18n::t("Abfrage lief beim Start noch").to_string()),
            None => Err(crate::i18n::t("nicht abgefragt").to_string()),
        };
        let cover = self.cd.cover.clone();
        let (tx, rx) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let c2 = cancel.clone();
        let rows = vec![(0.0, String::new(), String::new()); total];
        let first_audio = audio.first().map(|t| t.number).unwrap_or(1);
        let last_audio = audio.last().map(|t| t.number).unwrap_or(1);
        std::thread::spawn(move || {
            let ctx = RipContext { letter, toc, plan, rs, ar_db, cover, first_audio, last_audio, meta };
            rip_thread(ctx, tracks, c2, tx)
        });
        self.cd.rip = Some(RipRun { rx, cancel, rows, log: Vec::new(), finished: None });
    }

    // ---------------- tags ----------------

    /// Do `action` now, or first ask whether the changed tags are saved.
    fn tag_ask_or(&mut self, action: crate::app::TagPending) {
        if self.tag_loaded.is_some() && !self.tag_edit.same_as(&self.tag_saved) {
            self.tag_pending = Some(action);
        } else {
            self.tag_do(action);
        }
    }

    fn tag_do(&mut self, action: crate::app::TagPending) {
        match action {
            crate::app::TagPending::Select(i) => self.tag_sel = i,
            crate::app::TagPending::Replace(list) => {
                self.tag_files = list;
                self.tag_sel = 0;
                self.tag_loaded = None;
            }
        }
    }

    /// "Save the changed tags?" with Save, Discard and Cancel.
    pub(crate) fn tag_pending_window(&mut self, ctx: &egui::Context) {
        if self.tag_pending.is_none() {
            return;
        }
        let name = self.tag_loaded.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let mut answer = None;
        egui::Window::new(crate::i18n::t("Ungespeicherte Änderungen"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label(crate::i18n::tf(crate::i18n::t("Die Tags von {0} wurden geändert. Speichern?"), &[("0", name)]));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.add(theme::primary(&self.pal, crate::i18n::t("Speichern"))).clicked() {
                        answer = Some(1);
                    }
                    if ui.button(crate::i18n::t("Verwerfen")).clicked() {
                        answer = Some(2);
                    }
                    if ui.button(crate::i18n::t("Abbrechen")).clicked() {
                        answer = Some(0);
                    }
                });
            });
        let Some(a) = answer else { return };
        let action = self.tag_pending.take().unwrap();
        if a == 0 {
            return;
        }
        if a == 1 {
            if let Some(path) = self.tag_loaded.clone() {
                if let Err(e) = tags::write(&path, &self.tag_edit, true) {
                    self.notify(e);
                    return; // not saved: stay on this file
                }
                self.tag_saved = self.tag_edit.clone();
            }
        }
        self.tag_do(action);
    }

    fn load_tag_file(&mut self, ctx: &egui::Context) {
        let Some(path) = self.tag_files.get(self.tag_sel).cloned() else { return };
        if self.tag_loaded.as_ref() == Some(&path) {
            return;
        }
        let (t, i) = tags::read(&path);
        self.tag_saved = t.clone();
        self.tag_edit = t;
        self.tag_info = i;
        self.tag_cover_tex = self.tag_edit.front_cover().and_then(|a| image_texture(ctx, "tagcover", &a.data));
        self.tag_loaded = Some(path);
    }

    pub(crate) fn tags_page(&mut self, ui: &mut egui::Ui) {
        let p = self.pal;
        self.header(ui, "Tags", crate::i18n::t("Titel, Interpret, Album und Cover bearbeiten – einzeln oder für alle Dateien."));
        ui.horizontal(|ui| {
            if ui.add(theme::ghost(&icon_text(ph::PLUS, crate::i18n::t("Dateien öffnen")))).clicked() {
                if let Some(f) = rfd::FileDialog::new().add_filter("Audio", audio::INPUT_EXTENSIONS).pick_files() {
                    for x in f {
                        if !self.tag_files.contains(&x) {
                            self.tag_files.push(x);
                        }
                    }
                }
            }
            if ui.add(theme::ghost(crate::i18n::t("Aus Konverter-Liste"))).clicked() {
                let list = self.files.iter().map(|f| f.path.clone()).collect();
                self.tag_ask_or(crate::app::TagPending::Replace(list));
            }
            if ui.add_enabled(!self.tag_files.is_empty(), theme::ghost(crate::i18n::t("Liste leeren"))).clicked() {
                self.tag_ask_or(crate::app::TagPending::Replace(Vec::new()));
            }
        });
        ui.add_space(6.0);
        if self.tag_files.is_empty() {
            theme::card(&p).show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                ui.vertical_centered(|ui| {
                    ui.add_space((ui.available_height() * 0.3).max(10.0));
                    ui.label(RichText::new(ph::TAG).size(46.0).color(p.accent));
                    ui.label(RichText::new(crate::i18n::t("Dateien hierher ziehen, um ihre Tags zu bearbeiten")).size(18.0).strong());
                });
            });
            return;
        }
        self.tag_sel = self.tag_sel.min(self.tag_files.len() - 1);
        let ctx = ui.ctx().clone();
        self.load_tag_file(&ctx);

        egui::SidePanel::left("tag_list")
            .show_separator_line(false)
            .exact_width(300.0)
            .resizable(false)
            .frame(egui::Frame::new().inner_margin(egui::Margin { left: 0, right: 16, top: 0, bottom: 0 }))
            .show_inside(ui, |ui| {
                theme::card(&p).show(ui, |ui| {
                    ui.set_min_size(ui.available_size());
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        let mut pick = None;
                        for (i, f) in self.tag_files.iter().enumerate() {
                            let name = f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                            if ui.selectable_label(i == self.tag_sel, name).clicked() {
                                pick = Some(i);
                            }
                        }
                        if let Some(i) = pick.filter(|i| *i != self.tag_sel) {
                            self.tag_ask_or(crate::app::TagPending::Select(i));
                        }
                    });
                });
            });

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            theme::card(&p).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        match &self.tag_cover_tex {
                            Some(t) => {
                                ui.add(egui::Image::new(t).fit_to_exact_size(egui::vec2(160.0, 160.0)).corner_radius(10));
                            }
                            None => cover_placeholder(ui, &p, 160.0),
                        }
                        ui.horizontal(|ui| {
                            if ui.add(theme::ghost(crate::i18n::t("Bild laden…"))).clicked() {
                                if let Some(f) = rfd::FileDialog::new().add_filter(crate::i18n::t("Bild"), &["jpg", "jpeg", "png"]).pick_file() {
                                    if let Ok(data) = std::fs::read(&f) {
                                        self.tag_cover_tex = image_texture(ui.ctx(), "tagcover", &data);
                                        self.tag_edit.set_front_cover(tags::Art::new(data, tags::FRONT));
                                    }
                                }
                            }
                            if ui.add(theme::ghost(crate::i18n::t("Entfernen"))).clicked() {
                                // only the cover shown here; back covers and booklets stay
                                if let Some(front) = self.tag_edit.front_cover().map(|a| (a.kind, a.data.clone())) {
                                    self.tag_edit.art.retain(|a| (a.kind, &a.data) != (front.0, &front.1));
                                }
                                self.tag_cover_tex = self.tag_edit.front_cover().and_then(|a| image_texture(ui.ctx(), "tagcover", &a.data));
                            }
                        });
                        if let Some(a) = self.tag_edit.front_cover() {
                            if ui.add(theme::ghost(crate::i18n::t("Bild speichern…"))).clicked() {
                                let ext = if a.mime.contains("png") { "png" } else { "jpg" };
                                if let Some(f) = rfd::FileDialog::new().set_file_name(format!("cover.{ext}")).save_file() {
                                    let _ = std::fs::write(f, &a.data);
                                }
                            }
                        }
                    });
                    ui.vertical(|ui| {
                        let path = self.tag_files[self.tag_sel].clone();
                        ui.label(RichText::new(path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()).size(17.0).strong());
                        let i = &self.tag_info;
                        ui.label(muted(
                            &p,
                            crate::i18n::tf(crate::i18n::t("{0} · {1} Hz · {2} Bit · {3} Kanäle · {4} kbit/s"), &[("0", format!("{}", fmt_len(i.seconds))), ("1", format!("{}", i.rate.unwrap_or(0))), ("2", format!("{}", i.bits.map(|b| b.to_string()).unwrap_or_else(|| "–".into()))), ("3", format!("{}", i.channels.unwrap_or(0))), ("4", format!("{}", i.bitrate.unwrap_or(0)))]),
                        ));
                        let rg = self.tag_edit.get("replaygain_track_gain");
                        if !rg.is_empty() {
                            ui.label(muted(&p, crate::i18n::tf(crate::i18n::t("ReplayGain: Titel {rg}, Album {0}"), &[("rg", format!("{rg}")), ("0", format!("{}", self.tag_edit.get("replaygain_album_gain")))])));
                        }
                        ui.add_space(6.0);
                        egui::Grid::new("tagfields").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                            for (k, n) in tags::FIELDS {
                                ui.label(muted(&p, crate::i18n::t(n)));
                                let mut v = self.tag_edit.get(k).to_string();
                                let w = if matches!(*k, "track" | "track_total" | "disc" | "disc_total" | "year" | "compilation") { 80.0 } else { 380.0 };
                                if ui.add(egui::TextEdit::singleline(&mut v).desired_width(w)).changed() {
                                    self.tag_edit.set(k, v);
                                }
                                ui.end_row();
                            }
                        });
                    });
                });
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    if ui.add(theme::primary(&p, &icon_text(ph::FLOPPY_DISK, crate::i18n::t("Speichern")))).clicked() {
                        let path = self.tag_files[self.tag_sel].clone();
                        match tags::write(&path, &self.tag_edit, true) {
                            Ok(()) => {
                                self.tag_saved = self.tag_edit.clone();
                                self.notify(crate::i18n::t("Tags gespeichert"))
                            }
                            Err(e) => self.notify(e),
                        }
                    }
                    if ui
                        .add(theme::ghost(crate::i18n::t("Album-Felder für alle übernehmen")))
                        .on_hover_text(crate::i18n::t("Album, Albuminterpret, Jahr, Genre, Disc, Komponist, Sampler, Label und Cover"))
                        .clicked()
                    {
                        let mut ok = 0;
                        for f in self.tag_files.clone() {
                            let (mut t, _) = tags::read(&f);
                            tags::take_album_fields(&mut t, &self.tag_edit);
                            if tags::write(&f, &t, true).is_ok() {
                                ok += 1;
                            }
                        }
                        self.notify(crate::i18n::tf(crate::i18n::t("{ok} Dateien aktualisiert"), &[("ok", format!("{ok}"))]));
                    }
                    if ui.add(theme::ghost(crate::i18n::t("Titel durchnummerieren"))).clicked() {
                        let n = self.tag_files.len();
                        let mut ok = 0;
                        for (i, f) in self.tag_files.clone().iter().enumerate() {
                            let (mut t, _) = tags::read(f);
                            t.set("track", (i + 1).to_string());
                            t.set("track_total", n.to_string());
                            if tags::write(f, &t, false).is_ok() {
                                ok += 1;
                            }
                        }
                        let (track, total) = ((self.tag_sel + 1).to_string(), n.to_string());
                        for t in [&mut self.tag_edit, &mut self.tag_saved] {
                            t.set("track", track.clone());
                            t.set("track_total", total.clone());
                        }
                        self.notify(crate::i18n::tf(crate::i18n::t("{ok} Dateien nummeriert"), &[("ok", format!("{ok}"))]));
                    }
                });
            });
            ui.add_space(10.0);
            theme::card(&p).show(ui, |ui| {
                ui.set_width(ui.available_width());
                section(ui, &p, crate::i18n::t("Werkzeuge für alle Dateien der Liste"));
                ui.label(RichText::new(crate::i18n::t("Tags aus Dateiname")).strong());
                scheme_editor(ui, &p, &mut self.s.tag_scheme, &naming::DATA[1..], false, naming::TAG_SCHEME);
                if let Some(f) = self.tag_files.get(self.tag_sel) {
                    let stem = f.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                    let r = convert::tags_from_scheme(&stem, &self.s.tag_scheme);
                    let field = |k: &str| tags::FIELDS.iter().find(|f| f.0 == k).map(|f| crate::i18n::t(f.1).to_string()).unwrap_or_else(|| k.to_string());
                    let shown = r.iter().map(|(k, v)| format!("{} = {v}", field(k))).collect::<Vec<_>>().join(" · ");
                    ui.label(muted(&p, crate::i18n::tf(crate::i18n::t("Vorschau: {0}"), &[("0", shown)])));
                }
                if ui.add(theme::ghost(crate::i18n::t("Tags aus Dateiname übernehmen"))).clicked() {
                    let items = self.tag_files.iter().map(|f| (f.clone(), None)).collect();
                    self.start_conversion(items, Codec::TagFromFilename);
                }
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    let all: Vec<(PathBuf, Option<PathBuf>)> = self.tag_files.iter().map(|f| (f.clone(), None)).collect();
                    if ui.button(crate::i18n::t("ReplayGain berechnen")).clicked() {
                        self.start_conversion(all.clone(), Codec::ReplayGainOnly);
                    }
                    if ui.button("Audio-Info").clicked() {
                        self.start_conversion(all.clone(), Codec::AudioInfo);
                    }
                    if ui.button(crate::i18n::t("Audio-Prüfsumme (CRC)")).clicked() {
                        self.start_conversion(all.clone(), Codec::AudioCrc);
                    }
                    if ui.button(crate::i18n::t("Testumwandlung (Fehler finden)")).clicked() {
                        self.start_conversion(all, Codec::TestConversion);
                    }
                });
            });
        });
    }

    // ---------------- settings ----------------

    pub(crate) fn settings_page(&mut self, ui: &mut egui::Ui) {
        let p = self.pal;
        self.header(ui, crate::i18n::t("Einstellungen"), crate::i18n::t("Programme, Leistung, Aussehen und Windows-Integration."));
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            theme::card(&p).show(ui, |ui| {
                ui.set_width(ui.available_width());
                section(ui, &p, crate::i18n::t("Encoder-Programme"));
                ui.label(muted(
                    &p,
                    crate::i18n::t("WAV und AIFF schreibt PsyAudio selbst. MP3, FLAC, AAC, ALAC, Opus, Vorbis, WavPack, WMA und Speex laufen über FFmpeg; Monkey's Audio, Musepack und DSD über ihre eigenen Programme."),
                ));
                let mut changed = false;
                for (label, field, name) in
                    [("FFmpeg", 0usize, "ffmpeg"), ("Monkey's Audio (mac.exe)", 1, "mac"), ("Musepack (mpcenc.exe)", 2, "mpcenc"), (crate::i18n::t("SoX für DSD (sox.exe)"), 3, "sox")]
                {
                    ui.horizontal(|ui| {
                        let val = match field {
                            0 => &mut self.s.tools.ffmpeg,
                            1 => &mut self.s.tools.mac,
                            2 => &mut self.s.tools.mpcenc,
                            _ => &mut self.s.tools.sox,
                        };
                        let found = audio::find_tool(val, name);
                        ui.add_sized([200.0, 20.0], egui::Label::new(label));
                        let hint = found.as_ref().map(|f| f.display().to_string()).unwrap_or_else(|| crate::i18n::t("nicht gefunden").into());
                        // FFmpeg is asked for its encoders once the path is typed, not on every key
                        if ui.add(egui::TextEdit::singleline(val).hint_text(hint).desired_width(380.0)).lost_focus() {
                            changed = true;
                        }
                        if ui.button("…").clicked() {
                            if let Some(f) = pick_exe() {
                                *val = f;
                                changed = true;
                            }
                        }
                        ui.label(if found.is_some() { RichText::new(ph::CHECK).color(p.good) } else { RichText::new(ph::X).color(p.bad) });
                    });
                }
                if changed {
                    let (f, e) = ffmpeg_state(&self.s);
                    self.ffmpeg = f;
                    self.ffmpeg_encoders = e;
                }
                if let Some(f) = &self.ffmpeg {
                    ui.label(muted(&p, crate::i18n::tf(crate::i18n::t("FFmpeg: {0} ({1} Encoder)"), &[("0", format!("{}", f.display())), ("1", format!("{}", self.ffmpeg_encoders.len()))])));
                }
                if ui.button(crate::i18n::t("Erneut suchen")).clicked() {
                    let (f, e) = ffmpeg_state(&self.s);
                    self.ffmpeg = f;
                    self.ffmpeg_encoders = e;
                }
            });
            ui.add_space(10.0);
            theme::card(&p).show(ui, |ui| {
                ui.set_width(ui.available_width());
                section(ui, &p, crate::i18n::t("Leistung"));
                let max = (std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8) * 2).min(64); // at most 64 parallel conversions
                slider_row(ui, crate::i18n::t("Gleichzeitige Umwandlungen (CPU-Kerne)"), egui::Slider::new(&mut self.s.threads, 1..=max));
                ui.checkbox(&mut self.s.low_priority, crate::i18n::t("Mit niedriger Priorität arbeiten (PC bleibt flüssig)"));
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("Temporärer Ordner"));
                    let hint = std::env::temp_dir().display().to_string();
                    folder_field(ui, &p, &mut self.s.temp_folder, &hint, 420.0);
                    if !self.s.temp_folder.is_empty() && ui.add(theme::ghost("Standard")).clicked() {
                        self.s.temp_folder.clear();
                    }
                });
            });
            ui.add_space(10.0);
            theme::card(&p).show(ui, |ui| {
                ui.set_width(ui.available_width());
                section(ui, &p, crate::i18n::t("Aussehen"));
                egui::Grid::new("look").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                    ui.label(crate::i18n::t("Sprache"));
                    let mut lang: &str = crate::i18n::LANGUAGES.iter().map(|(c, _)| *c).find(|c| *c == self.s.lang).unwrap_or("en");
                    let options: Vec<(&str, &str)> = crate::i18n::LANGUAGES.iter().map(|(c, n)| (*c, *n)).collect();
                    segmented(ui, &p, &mut lang, &options);
                    if lang != self.s.lang {
                        self.s.lang = lang.to_string();
                        crate::i18n::set_lang(lang);
                    }
                    ui.end_row();
                    ui.label(crate::i18n::t("Thema"));
                    segmented(ui, &p, &mut self.s.dark, &[(true, crate::i18n::t("Mocha, dunkel")), (false, crate::i18n::t("Latte, hell"))]);
                    ui.end_row();
                });
            });
            ui.add_space(10.0);
            theme::card(&p).show(ui, |ui| {
                ui.set_width(ui.available_width());
                #[cfg(not(target_os = "linux"))]
                {
                    section(ui, &p, crate::i18n::t("Windows-Integration"));
                    ui.label(muted(&p, crate::i18n::t("Fügt im Explorer-Rechtsklickmenü von Audiodateien „Mit PsyAudio konvertieren“ hinzu (nur für Ihr Benutzerkonto).")));
                }
                #[cfg(target_os = "linux")]
                {
                    section(ui, &p, crate::i18n::t("Dolphin-Integration"));
                    ui.label(muted(&p, crate::i18n::t("Fügt im Dolphin-Rechtsklickmenü von Audiodateien „Mit PsyAudio konvertieren“ hinzu (nur für Ihr Benutzerkonto).")));
                }
                ui.horizontal(|ui| {
                    if ui.button(crate::i18n::t("Kontextmenü einrichten")).clicked() {
                        let r = context_menu(true);
                        self.notify(r);
                    }
                    if ui.button(crate::i18n::t("Kontextmenü entfernen")).clicked() {
                        let r = context_menu(false);
                        self.notify(r);
                    }
                });
            });
            ui.add_space(10.0);
            theme::card(&p).show(ui, |ui| {
                ui.set_width(ui.available_width());
                section(ui, &p, crate::i18n::t("Zurücksetzen"));
                if ui.button(crate::i18n::t("Alle Einstellungen auf Standard")).clicked() {
                    let profiles = self.s.profiles.clone();
                    self.s = Settings::default();
                    self.s.profiles = profiles;
                    self.notify(crate::i18n::t("Einstellungen zurückgesetzt"));
                }
            });
        });
    }
}

/// What AccurateRip says about one ripped track: the text for its row and log, and whether it
/// confirms the rip (a flag, so no check depends on the wording of a language).
pub(crate) fn ar_verdict(db: &Result<Vec<Pressing>, String>, idx: usize, v1: u32, v2: u32) -> (String, bool) {
    let db = match db {
        Ok(db) => db,
        Err(why) => return (format!("AccurateRip: {why}"), false),
    };
    let mut best: Option<(u8, &str)> = None;
    for pressing in db {
        if let Some((conf, crc)) = pressing.get(idx) {
            let ver = if *crc == v2 { Some("v2") } else if *crc == v1 { Some("v1") } else { None };
            if let Some(v) = ver {
                if best.map(|b| *conf > b.0).unwrap_or(true) {
                    best = Some((*conf, v));
                }
            }
        }
    }
    match best {
        Some((c, v)) => (crate::i18n::tf(crate::i18n::t("AccurateRip: Genau (Vertrauen {c}, {v})"), &[("c", format!("{c}")), ("v", v.to_string())]), true),
        // nothing matches: report the confidence of the first entry
        None => match db.first().and_then(|p| p.get(idx)).filter(|(_, crc)| *crc != 0) {
            Some((c, _)) => (crate::i18n::tf(crate::i18n::t("AccurateRip: Ungenau (Vertrauen {c}) – oder andere Pressung"), &[("c", format!("{c}"))]), false),
            None => (crate::i18n::t("AccurateRip: Titel nicht in der Datenbank").to_string(), false),
        },
    }
}

/// The status shown for a track; `mark_error` 1 marks insecure tracks, 2 unconfirmed ones as failed.
pub(crate) fn rip_status(mark_error: u8, status: String, insecure: bool, confirmed: bool) -> String {
    match mark_error {
        1 if insecure => crate::i18n::tf(crate::i18n::t("Fehler – {status}"), &[("status", status)]),
        2 if !confirmed => crate::i18n::tf(crate::i18n::t("Fehler – nicht von AccurateRip bestätigt ({status})"), &[("status", status)]),
        _ => status,
    }
}

/// "Stop after an insecure track": decided on the rip itself, not on the status text.
pub(crate) fn stop_after_track(cancel_on_insecure: bool, insecure_sectors: u32) -> bool {
    cancel_on_insecure && insecure_sectors > 0
}

struct RipContext {
    letter: char,
    toc: Toc,
    plan: Plan,
    rs: RipSettings,
    /// The disc's AccurateRip data, or why there is none.
    ar_db: Result<Vec<Pressing>, String>,
    cover: Option<Vec<u8>>,
    first_audio: u8,
    last_audio: u8,
    meta: DiscMeta,
}

/// Rip the chosen tracks one after another, check them against AccurateRip and encode each.
fn rip_thread(c: RipContext, tracks: Vec<(usize, u8, Tags)>, cancel: Arc<AtomicBool>, tx: Sender<RipMsg>) {
    let RipContext { letter, toc, plan, rs, ar_db, cover, first_audio, last_audio, meta } = c;
    let drive = match cdrip::Drive::open(letter) {
        Ok(d) => d,
        Err(e) => {
            let _ = tx.send(RipMsg::Finished(e));
            return;
        }
    };
    let batch = convert::Batch::new();
    let mut log = vec![
        crate::i18n::t("PsyAudio – Protokoll der Audio-Extraktion").to_string(),
        crate::i18n::tf(crate::i18n::t("Album: {0} – {1}"), &[("0", format!("{}", meta.artist)), ("1", format!("{}", meta.album))]),
        crate::i18n::tf(crate::i18n::t("Laufwerk {letter}  Versatz {0} Samples  Modus: {1}"), &[("letter", cdrip::drive_name(letter)), ("0", format!("{:+}", rs.drive_offset)), ("1", format!("{}", if rs.mode == RipMode::Secure { crate::i18n::t("Sicher") } else { "Burst" }))]),
        format!("MusicBrainz Disc-ID: {}   freedb: {:08x}", cdrip::musicbrainz_id(&toc), cdrip::freedb_id(&toc)),
        String::new(),
    ];
    let mut outputs: Vec<(u8, PathBuf)> = Vec::new();
    let (ctx, crx) = channel::<Msg>();
    let mut accurate = 0;
    let mut insecure_tracks = 0;
    let audio_numbers: Vec<u8> = toc.audio_tracks().iter().map(|t| t.number).collect();
    for (row, number, tg) in tracks {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let _ = tx.send(RipMsg::Progress(row, 0.0, crate::i18n::t("Lesen…").into()));
        let txp = tx.clone();
        let prog = move |f: f32| {
            let _ = txp.send(RipMsg::Progress(row, f * 0.8, crate::i18n::tf(crate::i18n::t("Lesen {0} %"), &[("0", format!("{:.0}", f * 100.0))])));
        };
        // a first pass that AccurateRip confirms needs no second pass
        let idx = audio_numbers.iter().position(|n| *n == number).unwrap_or(0);
        let ar_check = |raw: &[u8]| -> bool {
            let Ok(db) = ar_db.as_ref().map_err(|_| ()).and_then(|d| if rs.accuraterip { Ok(d) } else { Err(()) }) else { return false };
            // the 2940-sample edge applies to track 1 and the last track
            let (v1, v2) = cdrip::accuraterip_crc(raw, number == first_audio, number == last_audio);
            // any matching CRC counts, whatever its confidence
            db.iter().any(|p| p.get(idx).map(|(_, crc)| *crc != 0 && (*crc == v1 || *crc == v2)).unwrap_or(false))
        };
        let rp = crate::cdrip_read::ReadParams {
            secure: rs.mode == RipMode::Secure,
            reread_limit: rs.reread_limit,
            extra_passes: rs.extra_passes,
            extra_min: rs.extra_min,
            extra_max: rs.extra_max,
            extra_clean: rs.extra_clean,
            cache_kb: rs.cache_kb,
            abort_unrecoverable: rs.abort_unrecoverable,
            interpolate: rs.interpolate,
        };
        let r = crate::cdrip_read::rip_track(&drive, &toc, number, rs.drive_offset, rp, &cancel, &prog, &ar_check);
        let rip = match r {
            Ok(r) => r,
            Err(e) => {
                let _ = tx.send(RipMsg::TrackDone(row, crate::i18n::tf(crate::i18n::t("Fehler: {e}"), &[("e", format!("{e}"))]), String::new()));
                log.push(crate::i18n::tf(crate::i18n::t("Titel {number}: Fehler {e}"), &[("number", format!("{number:02}")), ("e", format!("{e}"))]));
                continue;
            }
        };
        let status = if rs.mode == RipMode::Burst {
            crate::i18n::t("Gerippt (Burst)").to_string()
        } else if rip.insecure_sectors > 0 {
            insecure_tracks += 1;
            crate::i18n::tf(crate::i18n::t("Unsicher ({0} Sektoren)"), &[("0", format!("{}", rip.insecure_sectors))])
        } else if rip.verified_first_pass {
            crate::i18n::t("Sicher (1 Durchgang, von AccurateRip bestätigt)").to_string()
        } else if rip.rereads > 0 {
            crate::i18n::tf(crate::i18n::t("Sicher (Warnung: {0} Durchgänge, {1} Blöcke neu gelesen)"), &[("0", format!("{}", rip.passes)), ("1", format!("{}", rip.rereads))])
        } else {
            crate::i18n::tf(crate::i18n::t("Sicher ({0} Durchgänge identisch)"), &[("0", format!("{}", rip.passes))])
        };
        let (v1, v2) = cdrip::accuraterip_crc(&rip.raw, number == first_audio, number == last_audio);
        let idx = audio_numbers.iter().position(|n| *n == number).unwrap_or(0);
        let (ar, confirmed) = if rs.accuraterip { ar_verdict(&ar_db, idx, v1, v2) } else { (String::new(), false) };
        if confirmed {
            accurate += 1;
        }
        let status = rip_status(rs.mark_error, status, rip.insecure_sectors > 0, confirmed);
        log.push(crate::i18n::tf(crate::i18n::t("Titel {number}  {0}  CRC32 {1}  AR v1 {v1} v2 {v2}  {status}  {ar}"), &[("number", format!("{number:02}")), ("0", format!("{}", tg.get("title"))), ("1", format!("{:08X}", rip.crc)), ("v1", format!("{v1:08X}")), ("v2", format!("{v2:08X}")), ("status", format!("{status}")), ("ar", format!("{ar}"))]));
        let _ = tx.send(RipMsg::Progress(row, 0.85, crate::i18n::t("Kodieren…").into()));
        let audio = Arc::new(cdrip::raw_to_audio(&rip.raw));
        let job = Job { id: 0, src: PathBuf::from(format!("Track{number:02}.wav")), root: None, tags: Some(tg), audio: Some(audio) };
        match batch.run(&plan, &job, &cancel, &ctx) {
            Ok(outs) => {
                for o in outs {
                    outputs.push((number, o));
                }
                let _ = tx.send(RipMsg::TrackDone(row, status.clone(), ar));
            }
            Err(e) => {
                let _ = tx.send(RipMsg::TrackDone(row, crate::i18n::tf(crate::i18n::t("Fehler beim Kodieren: {e}"), &[("e", format!("{e}"))]), ar));
                log.push(crate::i18n::tf(crate::i18n::t("   Kodierfehler: {e}"), &[("e", format!("{e}"))]));
            }
        }
        while crx.try_recv().is_ok() {}
        if stop_after_track(rs.cancel_on_insecure, rip.insecure_sectors) {
            log.push(crate::i18n::t("Abgebrochen nach unsicherem Titel").into());
            break;
        }
    }
    batch.finish(&plan, &ctx);
    while let Ok(m) = crx.try_recv() {
        if let Msg::Report(r) = m {
            let _ = tx.send(RipMsg::Log(r));
        }
    }
    let folder = outputs.first().and_then(|o| o.1.parent().map(|p| p.to_path_buf()));
    if let Some(folder) = &folder {
        let base = if meta.album.trim().is_empty() { "CD".to_string() } else { naming::clean_value(&meta.album) };
        if rs.write_log {
            log.push(String::new());
            log.push(crate::i18n::tf(crate::i18n::t("{accurate} Titel von AccurateRip bestätigt, {insecure_tracks} unsicher"), &[("accurate", format!("{accurate}")), ("insecure_tracks", format!("{insecure_tracks}"))]));
            let _ = std::fs::write(folder.join(format!("{base}.log")), log.join("\r\n"));
        }
        if rs.write_cue {
            let mut cue = format!("PERFORMER \"{}\"\r\nTITLE \"{}\"\r\n", meta.artist, meta.album);
            for (n, o) in &outputs {
                let name = o.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
                let i = audio_numbers.iter().position(|x| x == n).unwrap_or(0);
                let title = meta.tracks.get(i).map(|t| t.0.clone()).unwrap_or_default();
                cue += &format!("FILE \"{name}\" WAVE\r\n  TRACK {n:02} AUDIO\r\n    TITLE \"{title}\"\r\n    INDEX 01 00:00:00\r\n");
            }
            let _ = std::fs::write(folder.join(format!("{base}.cue")), cue);
        }
        if rs.folder_jpg {
            if let Some(c) = &cover {
                let _ = std::fs::write(folder.join("folder.jpg"), c);
            }
        }
    }
    if rs.eject_after && !cancel.load(Ordering::Relaxed) {
        let _ = drive.eject();
    }
    let summary = if cancel.load(Ordering::Relaxed) {
        crate::i18n::t("Abgebrochen").to_string()
    } else {
        crate::i18n::tf(crate::i18n::t("Fertig: {0} Dateien, {accurate} von AccurateRip bestätigt"), &[("0", format!("{}", outputs.len())), ("accurate", format!("{accurate}"))])
    };
    let _ = tx.send(RipMsg::Finished(summary));
}

#[cfg(test)]
mod rip_status_tests {
    use super::*;

    #[test]
    fn missing_accuraterip_data_says_why() {
        let (text, ok) = ar_verdict(&Err("nicht abgefragt".into()), 0, 1, 2);
        assert_eq!(text, "AccurateRip: nicht abgefragt");
        assert!(!ok);
    }

    #[test]
    fn a_matching_crc_confirms_in_any_language() {
        let db: Vec<Pressing> = vec![vec![(12, 0xAAAA)], vec![(70, 0xBBBB)]];
        let (_, ok) = ar_verdict(&Ok(db.clone()), 0, 0x1111, 0xBBBB);
        assert!(ok);
        let (_, ok) = ar_verdict(&Ok(db), 0, 0x1111, 0x2222);
        assert!(!ok);
    }

    #[test]
    fn marking_as_failed_does_not_hide_an_insecure_track() {
        assert!(rip_status(2, "Sicher".into(), false, false).starts_with("Fehler"));
        assert_eq!(rip_status(2, "Sicher".into(), false, true), "Sicher");
        assert!(rip_status(1, "Unsicher (3 Sektoren)".into(), true, true).starts_with("Fehler"));
        assert!(stop_after_track(true, 3));
        assert!(!stop_after_track(true, 0));
    }
}
