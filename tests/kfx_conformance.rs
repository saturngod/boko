//! KFX conformance checks against the reference content model.
//!
//! These tests encode invariants learned from validating boko's output with
//! jhowell's kfxlib (`tools/kfxcheck.py`) against Kindle Previewer gold
//! masters:
//!
//! - Scale-fit page-template images must reference an empty style — the
//!   template carries all positioning, and readers flag any leftover image
//!   properties as an unexpected image style.
//! - Styles must never declare `font-size` in percent. Reference KFX only
//!   uses em/rem, and KFX consumers prune inherited percentage values, which
//!   breaks font-size resolution for descendants.
//! - The full kfxcheck validation (structural + position maps + trial EPUB
//!   conversion) must report zero errors for a real conversion.

mod common;

use boko::Format;
use boko::kfx::container::{
    extract_doc_symbols, parse_container_header, parse_container_info, parse_index_table,
    skip_enty_header,
};
use boko::kfx::ion::{IonParser, IonValue};
use boko::kfx::symbols::{KFX_SYMBOL_TABLE, KfxSymbol};

/// Parse every entity of the given fragment type into Ion values.
fn parse_entities(kfx: &[u8], type_id: u32) -> Vec<IonValue> {
    let header = parse_container_header(&kfx[..18]).expect("container header");
    let info = parse_container_info(
        &kfx[header.container_info_offset
            ..header.container_info_offset + header.container_info_length],
    )
    .expect("container info");
    let (index_offset, index_length) = info.index.expect("index table");
    let entities = parse_index_table(
        &kfx[index_offset..index_offset + index_length],
        header.header_len,
    );

    entities
        .iter()
        .filter(|loc| loc.type_id == type_id)
        .filter_map(|loc| {
            let entity = &kfx[loc.offset..loc.offset + loc.length];
            IonParser::new(skip_enty_header(entity)).parse().ok()
        })
        .collect()
}

/// Document symbols (local symbol table) of the container.
fn doc_symbols(kfx: &[u8]) -> Vec<String> {
    let header = parse_container_header(&kfx[..18]).expect("container header");
    let info = parse_container_info(
        &kfx[header.container_info_offset
            ..header.container_info_offset + header.container_info_length],
    )
    .expect("container info");
    match info.doc_symbols {
        Some((off, len)) if len > 0 => extract_doc_symbols(&kfx[off..off + len]),
        _ => Vec::new(),
    }
}

fn resolve_symbol(doc_symbols: &[String], id: u64) -> String {
    let base = KFX_SYMBOL_TABLE.len() as u64;
    if id < base {
        KFX_SYMBOL_TABLE[id as usize].to_string()
    } else {
        doc_symbols
            .get((id - base) as usize)
            .cloned()
            .unwrap_or_default()
    }
}

fn get_field(fields: &[(u64, IonValue)], sym: KfxSymbol) -> Option<&IonValue> {
    fields
        .iter()
        .find_map(|(k, v)| (*k == sym as u64).then_some(v))
}

/// A one-page PNG "cover" chapter plus a text chapter, with CSS that would
/// previously leak image styling and percentage font sizes into the KFX.
fn build_test_book() -> Vec<u8> {
    use common::{Doc, EpubBuilder, Nav};
    EpubBuilder::new("Conformance Book")
        .css("img{max-width:95%;border:0;padding:0} body{font-size:100%} p{font-size:100%} .note{font-size:80%}")
        .image("images/plate.png", common::tiny_png())
        .doc(Doc::new(
            "text/plate.xhtml",
            "Plate",
            "<img src=\"../images/plate.png\" alt=\"plate\"/>",
        ))
        .doc(Doc::new(
            "text/ch1.xhtml",
            "One",
            "<p>Plain paragraph <span class=\"note\">with a smaller note</span>.</p>",
        ))
        .nav(vec![
            Nav::new("Plate", "text/plate.xhtml"),
            Nav::new("One", "text/ch1.xhtml"),
        ])
        .build()
}

/// Scale-fit page templates carry all positioning (fixed dims, scale_fit,
/// float center); the image node inside their storyline must reference a
/// style with no properties, exactly like Kindle Previewer output.
#[test]
fn scale_fit_images_reference_empty_style() {
    let epub = build_test_book();
    let mut book = boko::Book::from_bytes(&epub, Format::Epub).expect("import epub");
    let kfx = common::export_to_bytes(&mut book, Format::Kfx);
    let symbols = doc_symbols(&kfx);

    // Collect story names of scale-fit sections and the style of each
    // storyline image.
    let mut scale_fit_stories = std::collections::BTreeSet::new();
    for section in parse_entities(&kfx, KfxSymbol::Section as u32) {
        let IonValue::Struct(fields) = &section else {
            continue;
        };
        let Some(IonValue::List(templates)) = get_field(fields, KfxSymbol::PageTemplates) else {
            continue;
        };
        for template in templates {
            let IonValue::Struct(tf) = template else {
                continue;
            };
            let is_scale_fit = get_field(tf, KfxSymbol::Layout)
                .and_then(|v| v.as_symbol())
                .is_some_and(|s| s == KfxSymbol::ScaleFit as u64);
            if is_scale_fit
                && let Some(story) = get_field(tf, KfxSymbol::StoryName).and_then(|v| v.as_symbol())
            {
                scale_fit_stories.insert(resolve_symbol(&symbols, story));
            }
        }
    }
    assert!(
        !scale_fit_stories.is_empty(),
        "image-only chapter should produce a scale-fit section"
    );

    // Style names referenced by scale-fit storyline images.
    let mut image_styles = std::collections::BTreeSet::new();
    for storyline in parse_entities(&kfx, KfxSymbol::Storyline as u32) {
        let IonValue::Struct(fields) = &storyline else {
            continue;
        };
        let story = get_field(fields, KfxSymbol::StoryName)
            .and_then(|v| v.as_symbol())
            .map(|s| resolve_symbol(&symbols, s))
            .unwrap_or_default();
        if !scale_fit_stories.contains(&story) {
            continue;
        }
        let Some(IonValue::List(content)) = get_field(fields, KfxSymbol::ContentList) else {
            continue;
        };
        for node in content {
            let IonValue::Struct(nf) = node else { continue };
            let is_image = get_field(nf, KfxSymbol::Type)
                .and_then(|v| v.as_symbol())
                .is_some_and(|s| s == KfxSymbol::Image as u64);
            if is_image
                && let Some(style) = get_field(nf, KfxSymbol::Style).and_then(|v| v.as_symbol())
            {
                image_styles.insert(resolve_symbol(&symbols, style));
            }
        }
    }
    assert!(
        !image_styles.is_empty(),
        "scale-fit storyline should contain an image node with a style"
    );

    // Each referenced style must have no properties besides its name.
    for style in parse_entities(&kfx, KfxSymbol::Style as u32) {
        let IonValue::Struct(fields) = &style else {
            continue;
        };
        let name = get_field(fields, KfxSymbol::StyleName)
            .and_then(|v| v.as_symbol())
            .map(|s| resolve_symbol(&symbols, s))
            .unwrap_or_default();
        if image_styles.contains(&name) {
            assert_eq!(
                fields.len(),
                1,
                "scale-fit image style {name} must be empty (style_name only), got: {fields:?}"
            );
        }
    }
}

/// Styles must never carry `font-size` in percent: reference KFX uses only
/// em/rem, and consumers prune inherited percentage values, breaking
/// font-size resolution for descendants. 100% folds to 1em, 80% to 0.8em.
#[test]
fn font_size_is_never_percent() {
    let epub = build_test_book();
    let mut book = boko::Book::from_bytes(&epub, Format::Epub).expect("import epub");
    let kfx = common::export_to_bytes(&mut book, Format::Kfx);

    let mut saw_font_size = false;
    for style in parse_entities(&kfx, KfxSymbol::Style as u32) {
        let IonValue::Struct(fields) = &style else {
            continue;
        };
        let Some(IonValue::Struct(dim)) = get_field(fields, KfxSymbol::FontSize) else {
            continue;
        };
        saw_font_size = true;
        let unit = get_field(dim, KfxSymbol::Unit).and_then(|v| v.as_symbol());
        assert_ne!(
            unit,
            Some(KfxSymbol::Percent as u64),
            "font-size must not use percent units: {dim:?}"
        );
    }
    assert!(
        saw_font_size,
        "test book declares font sizes; none reached the KFX styles"
    );
}

/// End-to-end: the full kfxcheck validation (structural checks, position and
/// location map verification, trial EPUB conversion via kfxlib) must report
/// zero errors for a real EPUB conversion. Skipped when `uv` or the kfxlib
/// plugin source is unavailable.
#[test]
fn kfxcheck_reports_no_errors() {
    let uv = std::process::Command::new("uv").arg("--version").output();
    if uv.is_err() {
        eprintln!("Skipping test - uv not installed");
        return;
    }

    let mut book = common::open_fixture("epictetus.epub");
    let kfx = common::export_to_bytes(&mut book, Format::Kfx);
    let tmp = tempfile::Builder::new()
        .suffix(".kfx")
        .tempfile()
        .expect("temp file");
    std::fs::write(tmp.path(), &kfx).expect("write kfx");

    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/tools/kfxcheck.py");
    let output = std::process::Command::new("uv")
        .args(["run", "--script", script, "-q"])
        .arg(tmp.path())
        .output()
        .expect("run kfxcheck");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Exit code 3 = kfxlib source unavailable (offline environment): skip.
    if output.status.code() == Some(3) {
        eprintln!("Skipping test - kfxlib unavailable: {stderr}");
        return;
    }
    assert!(
        output.status.success(),
        "kfxcheck reported problems:\n{stdout}\n{stderr}"
    );
    assert!(
        stdout.contains("0 errors"),
        "kfxcheck reported errors:\n{stdout}"
    );
}
