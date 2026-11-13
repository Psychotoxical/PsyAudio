//! PsyAudio's look, shared with the other Psy apps: Catppuccin Mocha (dark) and Latte (light),
//! Mauve as the one accent, flat tonal layers, thin borders, no shadows. Values follow PsyTags.

use eframe::egui::{self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Stroke, TextStyle};

#[derive(Clone, Copy)]
pub struct Palette {
    /// Page background.
    pub bg: Color32,
    /// Sidebar.
    pub panel: Color32,
    /// Cards and dialogs.
    pub card: Color32,
    /// Buttons and other raised controls.
    pub card2: Color32,
    /// Text fields and other inputs.
    pub field: Color32,
    /// Hairlines and card borders.
    pub border: Color32,
    /// Outline of buttons and fields.
    pub field_border: Color32,
    /// Hovered controls.
    pub hover: Color32,
    /// Selected items (rows, chosen options).
    pub selection: Color32,
    /// Every other list row.
    pub alternate: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub disabled: Color32,
    pub accent: Color32,
    /// Text drawn on top of the accent colour.
    pub on_accent: Color32,
    /// Second, quiet colour for paths and previews.
    pub accent2: Color32,
    pub good: Color32,
    pub warn: Color32,
    pub bad: Color32,
}

const fn hex(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// Catppuccin Mocha.
pub const MOCHA: Palette = Palette {
    bg: hex(0x1e1e2e),
    panel: hex(0x181825),
    card: hex(0x181825),
    card2: hex(0x313244),
    field: hex(0x11111b),
    border: hex(0x313244),
    field_border: hex(0x585b70),
    hover: hex(0x45475a),
    selection: hex(0x45385c),
    alternate: hex(0x1e1e2e),
    text: hex(0xcdd6f4),
    muted: hex(0xa6adc8),
    disabled: hex(0x7f849c),
    accent: hex(0xcba6f7),
    on_accent: hex(0x1e1e2e),
    accent2: hex(0xb4befe),
    good: hex(0xa6e3a1),
    warn: hex(0xf9e2af),
    bad: hex(0xf38ba8),
};

/// Catppuccin Latte. The accent and state colours use darker shades where the Latte hue is too
/// light for text on the cards (accent: PsyTags' Latte hover mauve).
pub const LATTE: Palette = Palette {
    bg: hex(0xeff1f5),
    panel: hex(0xe6e9ef),
    card: hex(0xe6e9ef),
    card2: hex(0xccd0da),
    field: hex(0xdce0e8),
    border: hex(0xccd0da),
    field_border: hex(0xacb0be),
    hover: hex(0xbcc0cc),
    selection: hex(0xd8c9f4),
    alternate: hex(0xeff1f5),
    text: hex(0x4c4f69),
    muted: hex(0x5c5f77),
    disabled: hex(0x8c8fa1),
    accent: hex(0x7a2fd8),
    on_accent: hex(0xeff1f5),
    accent2: hex(0x1a56d6),
    good: hex(0x2b6e1c),
    warn: hex(0x8a5a00),
    bad: hex(0x9c0b2b),
};

pub fn palette(dark: bool) -> Palette {
    if dark {
        MOCHA
    } else {
        LATTE
    }
}

/// Corner radius used everywhere: cards 8, controls 6.
pub const R_CARD: u8 = 8;
pub const R_CTRL: u8 = 6;

/// Family for semibold text (headings, main buttons); falls back to the normal face.
pub fn semibold() -> FontFamily {
    FontFamily::Name("semibold".into())
}

/// Segoe UI, the Windows UI font PsyTags uses as well, plus the Phosphor icons; egui's own fonts
/// stay as fallback for other symbols.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    let dir = std::env::var_os("WINDIR").map(std::path::PathBuf::from).unwrap_or_else(|| "C:\\Windows".into()).join("Fonts");
    let mut regular = false;
    for (name, file) in [("segoe", "segoeui.ttf"), ("segoe-sb", "seguisb.ttf")] {
        if let Ok(bytes) = std::fs::read(dir.join(file)) {
            fonts.font_data.insert(name.into(), std::sync::Arc::new(FontData::from_owned(bytes)));
            regular |= name == "segoe";
        }
    }
    let fallback: Vec<String> = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let mut sb = Vec::new();
    if fonts.font_data.contains_key("segoe-sb") {
        sb.push("segoe-sb".to_string());
    }
    if regular {
        fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "segoe".into());
        sb.push("segoe".into());
    }
    sb.extend(fallback);
    fonts.families.insert(semibold(), sb);
    ctx.set_fonts(fonts);
}

pub fn apply(ctx: &egui::Context, p: &Palette, dark: bool) {
    let mut style = (*ctx.style()).clone();
    let mut v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.panel_fill = p.bg;
    v.window_fill = p.card;
    v.extreme_bg_color = p.field;
    v.faint_bg_color = p.alternate;
    v.code_bg_color = p.field;
    v.override_text_color = Some(p.text);
    v.window_corner_radius = CornerRadius::same(R_CARD);
    v.menu_corner_radius = CornerRadius::same(R_CTRL);
    v.window_stroke = Stroke::new(1.0_f32, p.border);
    v.window_shadow = egui::epaint::Shadow::NONE;
    v.popup_shadow = egui::epaint::Shadow::NONE;
    v.selection.bg_fill = p.selection;
    v.selection.stroke = Stroke::new(1.0_f32, p.text);
    v.hyperlink_color = p.accent;
    v.warn_fg_color = p.warn;
    v.error_fg_color = p.bad;
    v.text_cursor.stroke = Stroke::new(2.0_f32, p.accent);
    for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = CornerRadius::same(R_CTRL);
        w.expansion = 0.0;
    }
    v.widgets.noninteractive.bg_fill = p.card;
    v.widgets.noninteractive.weak_bg_fill = p.card;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.border);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.text);
    v.widgets.inactive.bg_fill = p.card2;
    v.widgets.inactive.weak_bg_fill = p.card2;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, p.field_border);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, p.text);
    v.widgets.hovered.bg_fill = p.hover;
    v.widgets.hovered.weak_bg_fill = p.hover;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, p.field_border);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, p.text);
    v.widgets.active.bg_fill = p.selection;
    v.widgets.active.weak_bg_fill = p.selection;
    v.widgets.active.bg_stroke = Stroke::new(1.0_f32, p.accent);
    // egui draws bold ("strong") text in this colour, so it must stay the normal text colour
    v.widgets.active.fg_stroke = Stroke::new(1.0_f32, p.text);
    v.widgets.open.bg_fill = p.card2;
    v.widgets.open.weak_bg_fill = p.card2;
    v.widgets.open.bg_stroke = Stroke::new(1.0_f32, p.accent);
    style.visuals = v;
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    // one button size for every text button
    style.spacing.button_padding = egui::vec2(12.0, 5.0);
    style.spacing.interact_size.y = 28.0;
    style.spacing.slider_width = 180.0;
    style.text_styles.insert(TextStyle::Heading, FontId::new(24.0, semibold()));
    style.text_styles.insert(TextStyle::Body, FontId::proportional(14.0));
    style.text_styles.insert(TextStyle::Button, FontId::proportional(14.0));
    style.text_styles.insert(TextStyle::Small, FontId::proportional(12.0));
    ctx.set_style(style);
}

pub fn card(p: &Palette) -> egui::Frame {
    egui::Frame::new()
        .fill(p.card)
        .corner_radius(CornerRadius::same(R_CARD))
        .stroke(Stroke::new(1.0_f32, p.border))
        .inner_margin(Margin::same(16))
}

/// The one filled button per page (the main action).
pub fn primary(p: &Palette, text: &str) -> egui::Button<'static> {
    egui::Button::new(egui::RichText::new(text.to_string()).color(p.on_accent).family(semibold()).size(15.0))
        .fill(p.accent)
        .stroke(Stroke::NONE)
        .corner_radius(CornerRadius::same(R_CARD))
        .min_size(egui::vec2(160.0, 36.0))
}

/// Every other text button: same height and corners as egui's own buttons.
pub fn ghost(text: &str) -> egui::Button<'static> {
    egui::Button::new(text.to_string()).corner_radius(CornerRadius::same(R_CTRL)).min_size(egui::vec2(0.0, 28.0))
}

/// Contrast ratio after WCAG 2.
#[cfg(test)]
fn contrast(a: Color32, b: Color32) -> f32 {
    let lin = |c: u8| {
        let c = c as f32 / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    let lum = |c: Color32| 0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b());
    let (x, y) = (lum(a), lum(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn text_colours_are_readable_on_every_surface() {
        for p in [MOCHA, LATTE] {
            for surface in [p.bg, p.panel, p.card] {
                for (name, c) in [("text", p.text), ("muted", p.muted), ("accent", p.accent), ("accent2", p.accent2), ("good", p.good), ("warn", p.warn), ("bad", p.bad)] {
                    let r = contrast(c, surface);
                    assert!(r >= 4.5, "{name} {c:?} on {surface:?}: {r:.2}");
                }
            }
            assert!(contrast(p.on_accent, p.accent) >= 4.5);
            assert!(contrast(p.text, p.card2) >= 4.5);
            assert!(contrast(p.text, p.selection) >= 4.5);
        }
    }
}
