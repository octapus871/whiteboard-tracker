use egui::{FontData, FontDefinitions, FontFamily};
use std::path::Path;
use std::sync::{Arc, OnceLock};
use unicode_bidi::BidiInfo;

/// Bundled fallback so Arabic always renders even when no system font is available.
const BUNDLED_FONT: &[u8] = include_bytes!("../assets/NotoSansArabic-Regular.ttf");

/// Installs the Arabic font (preferring system Noto Sans Arabic) as the primary
/// proportional (and monospace) font in `egui::FontDefinitions`.
pub fn install_arabic_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    let candidate_paths = [
        "/usr/share/fonts/google-noto/NotoSansArabic-Regular.ttf",
        "/usr/share/fonts/google-noto/NotoSansArabic-CondensedSemiBold.ttf",
        "/usr/share/fonts/noto-arabic/NotoSansArabic-Regular.ttf",
        "/usr/share/fonts/google-noto-vf/NotoSansArabic-VF.ttf",
        "/usr/share/fonts/truetype/noto/NotoSansArabic-Regular.ttf",
        "/usr/share/fonts/NotoSansArabic-Regular.ttf",
        "/Library/Fonts/NotoSansArabic-Regular.ttf",
        "C:\\Windows\\Fonts\\NotoSansArabic-Regular.ttf",
    ];

    let system_font = candidate_paths
        .iter()
        .filter(|path| Path::new(path).exists())
        .find_map(|path| std::fs::read(path).ok());

    let font_data = match system_font {
        Some(bytes) => FontData::from_owned(bytes),
        None => FontData::from_static(BUNDLED_FONT),
    };

    fonts
        .font_data
        .insert("NotoSansArabic".to_string(), Arc::new(font_data));

    // Primary (first) font so connected Arabic glyphs win over the Latin default.
    if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
        family.insert(0, "NotoSansArabic".to_string());
    }
    if let Some(family) = fonts.families.get_mut(&FontFamily::Monospace) {
        family.insert(0, "NotoSansArabic".to_string());
    }

    ctx.set_fonts(fonts);
}

/// Checks if string contains Arabic characters.
pub fn contains_arabic(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(c,
            '\u{0600}'..='\u{06FF}' |
            '\u{0750}'..='\u{077F}' |
            '\u{08A0}'..='\u{08FF}' |
            '\u{FB50}'..='\u{FDFF}' |
            '\u{FE70}'..='\u{FEFF}'
        )
    })
}

/// Shapes Arabic text for correct visual rendering in egui.
///
/// Runs the reshaper (connects letters) followed by the Unicode Bidirectional
/// Algorithm (`unicode_bidi::BidiInfo`) so the glyphs are reversed into proper
/// right-to-left visual order. Bracket pairs at RTL levels are mirrored, since
/// `reorder_line` does not perform rule L4. Non-Arabic text is returned
/// untouched, and line breaks are preserved.
///
/// The returned string always has the same number of `char`s as the input (the
/// reshaper is configured without count-changing ligatures/harakat deletion).
/// This is required for `TextEdit` layouters, whose cursor indices are
/// character-based into the original, unshaped string.
pub fn shape_arabic(text: &str) -> String {
    if text.is_empty() || !contains_arabic(text) {
        return text.to_string();
    }

    let shaped = text
        .split('\n')
        .map(shape_arabic_line)
        .collect::<Vec<_>>()
        .join("\n");

    if shaped.chars().count() == text.chars().count() {
        shaped
    } else {
        // Never desync editable text from its galley (e.g. exotic ZWJ input).
        text.to_string()
    }
}

/// Reshaper configured to preserve the input character count so editable text
/// can be mapped back to its logical `String` by `TextEdit`.
fn reshaper() -> &'static arabic_reshaper::ArabicReshaper<'static> {
    static RESHAPER: OnceLock<arabic_reshaper::ArabicReshaper<'static>> = OnceLock::new();
    RESHAPER.get_or_init(|| {
        let mut reshaper = arabic_reshaper::ArabicReshaper::new();
        reshaper.configuration.insert("support_ligatures", false);
        reshaper.configuration.insert("delete_harakat", false);
        reshaper
    })
}

fn shape_arabic_line(line: &str) -> String {
    if line.is_empty() {
        return String::new();
    }

    let reshaped = reshaper().reshape(line);

    // `None` lets unicode-bidi auto-detect the base direction (RTL for Arabic).
    let bidi_info = BidiInfo::new(&reshaped, None);
    if bidi_info.paragraphs.is_empty() {
        return reshaped;
    }

    let mut result = String::with_capacity(reshaped.len());
    for para in &bidi_info.paragraphs {
        // Reorder manually so Rule L4 (mirroring) can be applied in visual
        // order. `reorder_line` deliberately does not mirror brackets.
        let (levels, runs) = bidi_info.visual_runs(para, para.range.clone());
        for run in runs {
            if levels[run.start].is_rtl() {
                for ch in reshaped[run.clone()].chars().rev() {
                    result.push(mirror_char(ch));
                }
            } else {
                result.push_str(&reshaped[run]);
            }
        }
    }
    result
}

/// Applies Unicode mirroring to bracket pairs drawn in RTL runs.
fn mirror_char(ch: char) -> char {
    match ch {
        '(' => ')',
        ')' => '(',
        '[' => ']',
        ']' => '[',
        '{' => '}',
        '}' => '{',
        '<' => '>',
        '>' => '<',
        '«' => '»',
        '»' => '«',
        '‹' => '›',
        '›' => '‹',
        _ => ch,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_arabic() {
        assert!(contains_arabic("علاج بابا"));
        assert!(contains_arabic("Task علاج"));
        assert!(!contains_arabic("plain english 123"));
        assert!(!contains_arabic(""));
    }

    #[test]
    fn reshapes_arabic_into_presentation_forms() {
        let shaped = shape_arabic("سلام");
        assert!(!shaped.is_empty());
        assert!(shaped
            .chars()
            .any(|c| ('\u{FE70}'..='\u{FEFF}').contains(&c)));
    }

    #[test]
    fn keeps_latin_text_readable() {
        assert_eq!(shape_arabic("hello world"), "hello world");
    }

    #[test]
    fn preserves_line_breaks() {
        let shaped = shape_arabic("سلام\nhello");
        assert!(shaped.contains('\n'));
    }

    #[test]
    fn mirrors_brackets_in_rtl_runs() {
        let shaped = shape_arabic("معاينة الألواح (1)");
        // Visual order with Rule L4 applied: the parentheses surround `1`
        // with the correct mirror glyphs.
        assert!(
            shaped.contains("(1)"),
            "expected mirrored parenthesis, got {shaped:?}"
        );
        let shaped_word = shape_arabic("سلام (مرحبا)");
        assert!(
            shaped_word.contains("(ﺎﺒﺣﺮﻣ)"),
            "expected mirrored parenthesis around RTL word, got {shaped_word:?}"
        );
    }

    #[test]
    fn shaping_preserves_char_count_for_editing() {
        for text in [
            "سلام",
            "لا",
            "معاينة الألواح (1)",
            "hello علاج 123",
            "مُحَمَّد",
            "خطة / مراجعة\nسلام",
        ] {
            assert_eq!(
                shape_arabic(text).chars().count(),
                text.chars().count(),
                "shaping changed char count for {text:?}"
            );
        }
    }

    #[test]
    fn arabic_font_loads_and_renders_a_frame() {
        // Headless egui run exercises font parsing/layout for shaped Arabic glyphs.
        let ctx = egui::Context::default();
        install_arabic_fonts(&ctx);
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.label(shape_arabic("علاج بابا / كتابة اللائحة للمرافعة"));
            });
        });
        let shapes = ctx.tessellate(output.shapes, output.pixels_per_point);
        assert!(
            !shapes.is_empty(),
            "expected shaped Arabic text to produce geometry"
        );
    }
}
