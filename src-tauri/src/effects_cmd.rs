//! Effects: the searchable catalogue and the short line each effect shows in the list.

use psyaudio_core::dsp::{self, Effect, EffectEntry};
use psyaudio_core::tags;
use serde::Serialize;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_name_each_effect() {
        let lines = effect_lines(vec![Effect::DeEmphasis, Effect::Fade { in_ms: 0, out_ms: 3000 }]);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].name, Effect::DeEmphasis.name());
        assert!(!lines[1].summary.is_empty());
    }
}

#[derive(Serialize)]
pub struct EffectCatalogue {
    entries: Vec<EffectEntry>,
    /// Kinds of "Normalize volume", in German (translated in the frontend).
    normalize_modes: Vec<&'static str>,
    /// dB to add for LUFS ("= −18 LUFS").
    ebu_offset: f32,
    /// Tag fields "Tag processing" can change: (key, German label).
    tag_fields: Vec<(&'static str, &'static str)>,
}

/// Everything the effect picker and the effect editors offer.
#[tauri::command]
pub fn effect_catalogue() -> EffectCatalogue {
    EffectCatalogue {
        entries: dsp::catalogue_entries(),
        normalize_modes: dsp::NORMALIZE_MODES.to_vec(),
        ebu_offset: dsp::NORMALIZE_EBU_OFFSET,
        tag_fields: tags::FIELDS.to_vec(),
    }
}

#[derive(Serialize)]
pub struct EffectLine {
    name: String,
    summary: String,
}

/// Name and settings line of each effect, in list order.
#[tauri::command]
pub fn effect_lines(effects: Vec<Effect>) -> Vec<EffectLine> {
    effects.iter().map(|e| EffectLine { name: e.name(), summary: e.summary() }).collect()
}
