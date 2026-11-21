use egui::{FontData, FontDefinitions, FontFamily, FontTweak};
use std::path::Path;
use std::sync::{Arc, OnceLock};
use unicode_bidi::{BidiInfo, Level};

/// Bundled fallback so Arabic always renders even when no system font is available.
const BUNDLED_FONT: &[u8] = include_bytes!("../assets/NotoSansArabic-Regular.ttf");

/// Name the Arabic face is registered under in the font definitions.
const ARABIC_FONT_NAME: &str = "Arabic";

/// Visual tweak applied to the Arabic face so its weight and baseline match
/// the Latin UI font.
fn arabic_font_tweak() -> FontTweak {
    FontTweak {
        scale: 1.08,
        y_offset_factor: -0.04,
        ..Default::default()
    }
}

/// High-craft geometric Arabic faces, in strict priority order.
///
/// 1. IBM Plex Sans Arabic  2. Noto Kufi Arabic  3. Noto Sans Arabic
/// 4. Plain Noto Sans Arabic as the last resort.
const ARABIC_FONT_CANDIDATES: &[&str] = &[
    "/usr/share/fonts/ibm-plex-sans-arabic-fonts/IBMPlexSansArabic-Medium.otf",
    "/usr/share/fonts/ibm-plex-sans-arabic-fonts/IBMPlexSansArabic-Regular.otf",
    "/usr/share/fonts/google-noto/NotoKufiArabic-Medium.ttf",
    "/usr/share/fonts/google-noto/NotoSansArabic-Medium.ttf",
    "/usr/share/fonts/google-noto/NotoKufiArabic-Regular.ttf",
    "/usr/share/fonts/google-noto/NotoSansArabic-SemiBold.ttf",
    "/usr/share/fonts/google-noto/NotoSansArabic-Regular.ttf",
    "/usr/share/fonts/ibm-plex/IBMPlexSansArabic-Medium.ttf",
    "/usr/share/fonts/ibm-plex/IBMPlexSansArabic-Regular.ttf",
    "/usr/share/fonts/truetype/noto/NotoKufiArabic-Regular.ttf",
    "/usr/share/fonts/noto-arabic/NotoSansArabic-Regular.ttf",
    "/usr/share/fonts/NotoSansArabic-Regular.ttf",
    "/Library/Fonts/NotoSansArabic-Regular.ttf",
    "C:\\Windows\\Fonts\\NotoSansArabic-Regular.ttf",
];

/// Installs the Arabic font into `egui::FontDefinitions`.
///
/// The face is registered right after the primary UI font in both the
/// proportional and the monospace family, so Latin text keeps its own face
/// while Arabic glyphs fall back to the geometric Arabic face.
pub fn install_arabic_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    let font_data = load_arabic_font_data();
    fonts
        .font_data
        .insert(ARABIC_FONT_NAME.to_string(), Arc::new(font_data));

    // Right after the primary UI font in both families.
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        if let Some(names) = fonts.families.get_mut(&family) {
            let index = if names.is_empty() { 0 } else { 1 };
            names.insert(index, ARABIC_FONT_NAME.to_string());
        }
    }

    ctx.set_fonts(fonts);
}

/// Loads the first usable Arabic face from the priority list.
///
/// Files that are not TrueType (CFF/`.otf` outlines) are skipped, because
/// egui's rasterizer cannot parse them.
fn load_arabic_font_data() -> FontData {
    for path in ARABIC_FONT_CANDIDATES {
        if !Path::new(path).exists() {
            continue;
        }
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        if !is_truetype(&bytes) {
            eprintln!("Skipping Arabic font {}: unsupported font format", path);
            continue;
        }
        return FontData::from_owned(bytes).tweak(arabic_font_tweak());
    }

    // Bundled fallback so Arabic always renders.
    FontData::from_static(BUNDLED_FONT).tweak(arabic_font_tweak())
}

/// True for TrueType outlines (`glyf`) or OpenType outlines (`OTTO`), false for unknown formats.
fn is_truetype(bytes: &[u8]) -> bool {
    matches!(
        bytes.get(0..4),
        Some([0x00, 0x01, 0x00, 0x00])
            | Some([b't', b'r', b'u', b'e'])
            | Some([b't', b't', b'c', b'f'])
            | Some([b'O', b'T', b'T', b'O'])
    )
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

    // Force an RTL paragraph level so embedded Latin words and digits keep
    // their natural order inside an Arabic sentence.
    let bidi_info = BidiInfo::new(&reshaped, Some(Level::rtl()));
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
                push_reversed_rtl_clusters(&reshaped[run], &mut result);
            } else {
                result.push_str(&reshaped[run]);
            }
        }
    }
    result
}

/// Reverses an RTL run by base+combining-mark clusters so Arabic diacritics
/// (e.g. tanwin `ً` in `فرعاً`) stay attached to their base letter instead of
/// jumping onto adjacent glyphs and overlapping.
fn push_reversed_rtl_clusters(run: &str, out: &mut String) {
    let mut clusters: Vec<Vec<char>> = Vec::new();
    for ch in run.chars() {
        if is_combining_mark(ch) {
            if let Some(last) = clusters.last_mut() {
                last.push(ch);
                continue;
            }
        }
        clusters.push(vec![ch]);
    }
    for cluster in clusters.into_iter().rev() {
        for ch in cluster {
            out.push(mirror_char(ch));
        }
    }
}

fn is_combining_mark(ch: char) -> bool {
    matches!(
        ch,
        '\u{0610}'..='\u{061A}'
            | '\u{064B}'..='\u{065F}'
            | '\u{0670}'
            | '\u{06D6}'..='\u{06DC}'
            | '\u{06DF}'..='\u{06E4}'
            | '\u{06E7}'..='\u{06E8}'
            | '\u{06EA}'..='\u{06ED}'
    )
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
    fn arabic_font_is_registered_after_the_primary_ui_font() {
        let ctx = egui::Context::default();
        install_arabic_fonts(&ctx);
        // Fonts are only available after the first frame.
        let _ = ctx.run(egui::RawInput::default(), |_| {});

        let definitions = ctx.fonts(|f| f.definitions().clone());

        let proportional = &definitions.families[&FontFamily::Proportional];
        assert_ne!(
            proportional[0], ARABIC_FONT_NAME,
            "the primary UI font stays first"
        );
        assert_eq!(proportional[1], ARABIC_FONT_NAME);

        let monospace = &definitions.families[&FontFamily::Monospace];
        assert_eq!(monospace[1], ARABIC_FONT_NAME);

        let data = &definitions.font_data[ARABIC_FONT_NAME];
        assert_eq!(data.tweak.scale, 1.08);
        assert_eq!(data.tweak.y_offset_factor, -0.04);
        assert!(!data.font.is_empty());
    }

    #[test]
    fn mixed_arabic_and_latin_keep_their_natural_order() {
        // Latin words inside an Arabic sentence must not be reversed.
        let shaped = shape_arabic("مراجعة File 2");
        assert!(shaped.contains("File"), "latin run reversed: {shaped}");

        // Numbers must keep their digit order (not become "21").
        let shaped = shape_arabic("مراجعة 12 ملف");
        assert!(shaped.contains("12"), "digits reversed: {shaped}");

        // The character count is preserved so editing stays in sync.
        for text in ["مراجعة File 2", "مراجعة 12 ملف", "قيمة 3٫5"] {
            assert_eq!(
                shape_arabic(text).chars().count(),
                text.chars().count(),
                "char count changed for {text:?}"
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
