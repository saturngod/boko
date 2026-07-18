//! KFX must preserve table cell spans and ordered-list start values.
//!
//! These live in the IR `SemanticMap` (`col_span`, `row_span`, `list_start`)
//! and are emitted as Ion integers on the storyline element. Before the
//! carriers existed, KFX export dropped them: spanned cells collapsed to 1x1
//! and `<ol start=N>` lost its numbering.

mod common;

use boko::model::{Format, Role};

#[test]
fn kfx_preserves_table_spans_and_ol_start() {
    use common::{Doc, EpubBuilder, Nav};

    let epub = EpubBuilder::new("Spans Book")
        .doc(Doc::new(
            "text/ch1.xhtml",
            "Spans",
            "<h1>Grid</h1>\
             <table><thead><tr><th>Head</th></tr></thead><tbody>\
             <tr><td colspan=\"2\">wide</td><td rowspan=\"3\">tall</td></tr>\
             <tr><td>a</td><td>b</td></tr>\
             </tbody></table>\
             <ol start=\"5\"><li>five</li><li>six</li></ol>",
        ))
        .nav(vec![Nav::new("Grid", "text/ch1.xhtml")])
        .build();

    // Round-trip EPUB → KFX → import.
    let mut src = boko::Book::from_bytes(&epub, Format::Epub).expect("import epub");
    let kfx = common::export_to_bytes(&mut src, Format::Kfx);
    let out = boko::Book::from_bytes(&kfx, Format::Kfx).expect("import kfx");

    // Table cell spans and the th/td distinction survive.
    let (mut saw_colspan, mut saw_rowspan, mut saw_header) = (false, false, false);
    let cell_ids: Vec<_> = {
        let ids: Vec<_> = out.spine().iter().map(|e| e.id).collect();
        let mut v = Vec::new();
        for id in ids {
            let ch = out.load_chapter(id).expect("load");
            for nid in ch.iter_dfs() {
                if ch.node(nid).map(|n| n.role) == Some(Role::TableCell) {
                    if ch.semantics.col_span(nid) == Some(2) {
                        saw_colspan = true;
                    }
                    if ch.semantics.row_span(nid) == Some(3) {
                        saw_rowspan = true;
                    }
                    if ch.semantics.is_header_cell(nid) {
                        saw_header = true;
                    }
                    v.push(nid);
                }
            }
        }
        v
    };
    assert!(!cell_ids.is_empty(), "KFX round-trip lost the table cells");
    assert!(saw_colspan, "colspan=2 did not survive the KFX round trip");
    assert!(saw_rowspan, "rowspan=3 did not survive the KFX round trip");
    assert!(
        saw_header,
        "th header cell did not survive the KFX round trip"
    );

    // Ordered-list start survives.
    let mut saw_start = false;
    let ids: Vec<_> = out.spine().iter().map(|e| e.id).collect();
    for id in ids {
        let ch = out.load_chapter(id).expect("load");
        for nid in ch.iter_dfs() {
            if ch.node(nid).map(|n| n.role) == Some(Role::OrderedList)
                && ch.semantics.list_start(nid) == Some(5)
            {
                saw_start = true;
            }
        }
    }
    assert!(saw_start, "ol start=5 did not survive the KFX round trip");
}

// ============================================================================
// Storyline encoding assertions (element types / classifications / markers)
// ============================================================================

/// Scan every storyline ($259) entity in a KFX container, returning the set of
/// element `type` symbol ids, `yj.classification` symbol ids, and resolved
/// `yj.semantics.type` marker names.
fn scan_storylines(
    kfx: &[u8],
) -> (
    std::collections::BTreeSet<u64>,
    std::collections::BTreeSet<u64>,
    std::collections::BTreeSet<String>,
) {
    use boko::kfx::container::{
        extract_doc_symbols, parse_container_header, parse_container_info, parse_index_table,
        skip_enty_header,
    };
    use boko::kfx::ion::{IonParser, IonValue};
    use boko::kfx::symbols::{KFX_SYMBOL_TABLE, KfxSymbol};

    let header = parse_container_header(&kfx[..18]).expect("container header");
    let info = parse_container_info(
        &kfx[header.container_info_offset
            ..header.container_info_offset + header.container_info_length],
    )
    .expect("container info");
    let doc_symbols = match info.doc_symbols {
        Some((off, len)) if len > 0 => extract_doc_symbols(&kfx[off..off + len]),
        _ => Vec::new(),
    };
    let (index_offset, index_length) = info.index.expect("index table");
    let entities = parse_index_table(
        &kfx[index_offset..index_offset + index_length],
        header.header_len,
    );

    let resolve = |id: u64| -> String {
        let base = KFX_SYMBOL_TABLE.len() as u64;
        if id < base {
            KFX_SYMBOL_TABLE[id as usize].to_string()
        } else {
            doc_symbols
                .get((id - base) as usize)
                .cloned()
                .unwrap_or_default()
        }
    };
    let semantics_field: Vec<u64> = doc_symbols
        .iter()
        .enumerate()
        .filter(|(_, s)| s.as_str() == "yj.semantics.type")
        .map(|(i, _)| i as u64 + KFX_SYMBOL_TABLE.len() as u64)
        .collect();

    let mut types = std::collections::BTreeSet::new();
    let mut classifications = std::collections::BTreeSet::new();
    let mut markers = std::collections::BTreeSet::new();

    fn walk(
        v: &IonValue,
        types: &mut std::collections::BTreeSet<u64>,
        classifications: &mut std::collections::BTreeSet<u64>,
        markers: &mut std::collections::BTreeSet<String>,
        semantics_field: &[u64],
        resolve: &dyn Fn(u64) -> String,
    ) {
        match v {
            IonValue::Struct(fields) => {
                for (k, val) in fields {
                    if *k == KfxSymbol::Type as u64
                        && let IonValue::Symbol(s) = val
                    {
                        types.insert(*s);
                    }
                    if *k == KfxSymbol::YjClassification as u64
                        && let IonValue::Symbol(s) = val
                    {
                        classifications.insert(*s);
                    }
                    if semantics_field.contains(k)
                        && let IonValue::Symbol(s) = val
                    {
                        markers.insert(resolve(*s));
                    }
                    walk(
                        val,
                        types,
                        classifications,
                        markers,
                        semantics_field,
                        resolve,
                    );
                }
            }
            IonValue::List(items) => {
                for item in items {
                    walk(
                        item,
                        types,
                        classifications,
                        markers,
                        semantics_field,
                        resolve,
                    );
                }
            }
            IonValue::Annotated(_, inner) => walk(
                inner,
                types,
                classifications,
                markers,
                semantics_field,
                resolve,
            ),
            _ => {}
        }
    }

    for loc in entities {
        if loc.type_id != KfxSymbol::Storyline as u32 {
            continue;
        }
        let entity = &kfx[loc.offset..loc.offset + loc.length];
        let payload = skip_enty_header(entity);
        if let Ok(value) = IonParser::new(payload).parse() {
            walk(
                &value,
                &mut types,
                &mut classifications,
                &mut markers,
                &semantics_field,
                &resolve,
            );
        }
    }
    (types, classifications, markers)
}

/// A bordered table must keep its table/table_row element types in KFX.
/// The border container-wrapper hack previously degraded the whole structure
/// to nested `container` elements (Kindle Previewer keeps table types and
/// renders borders from styles).
#[test]
fn bordered_table_keeps_table_structure_in_kfx() {
    use boko::kfx::symbols::KfxSymbol;
    use common::{Doc, EpubBuilder, Nav};

    let epub = EpubBuilder::new("Bordered Table")
        .css("table{border:1px solid black;border-collapse:collapse} td,th{border:1px solid #333} tr{border-bottom:1px solid #999}")
        .doc(Doc::new(
            "text/ch1.xhtml",
            "Table",
            "<p>before</p><table><thead><tr><th>H</th></tr></thead>\
             <tbody><tr><td>a</td><td>b</td></tr><tr><td>c</td><td>d</td></tr></tbody></table>",
        ))
        .nav(vec![Nav::new("Table", "text/ch1.xhtml")])
        .build();

    let mut src = boko::Book::from_bytes(&epub, Format::Epub).expect("import epub");
    let kfx = common::export_to_bytes(&mut src, Format::Kfx);

    let (types, _, _) = scan_storylines(&kfx);
    assert!(
        types.contains(&(KfxSymbol::Table as u64)),
        "bordered table lost its table element type: {types:?}"
    );
    assert!(
        types.contains(&(KfxSymbol::TableRow as u64)),
        "bordered table lost its table_row element types: {types:?}"
    );

    // And the structure survives a full round-trip.
    let out = boko::Book::from_bytes(&kfx, Format::Kfx).expect("import kfx");
    let ids: Vec<_> = out.spine().iter().map(|e| e.id).collect();
    let mut saw_row = false;
    for id in ids {
        let ch = out.load_chapter(id).expect("load");
        for nid in ch.iter_dfs() {
            if ch.node(nid).map(|n| n.role) == Some(Role::TableRow) {
                saw_row = true;
            }
        }
    }
    assert!(saw_row, "table rows lost through KFX round-trip");
}

/// Sidebars export as plain containers carrying a `yj.semantics.type: sidebar`
/// marker — never the $280 element type or $620 classification, which
/// Amazon-produced books don't use — while the role survives the round trip.
#[test]
fn sidebar_exports_as_container_with_marker() {
    use boko::kfx::symbols::KfxSymbol;
    use common::{Doc, EpubBuilder, Nav};

    let epub = EpubBuilder::new("Sidebar Book")
        .doc(Doc::new(
            "text/ch1.xhtml",
            "Aside",
            "<p>body text</p>\
             <aside epub:type=\"sidebar\"><p>note box</p></aside>",
        ))
        .nav(vec![Nav::new("Aside", "text/ch1.xhtml")])
        .build();

    let mut src = boko::Book::from_bytes(&epub, Format::Epub).expect("import epub");
    let kfx = common::export_to_bytes(&mut src, Format::Kfx);

    let (types, classifications, markers) = scan_storylines(&kfx);
    assert!(
        !types.contains(&(KfxSymbol::Sidebar as u64)),
        "sidebar must not use the $280 element type: {types:?}"
    );
    assert!(
        !classifications.contains(&(KfxSymbol::YjSidenote as u64)),
        "sidebar must not use the $620 classification: {classifications:?}"
    );
    assert!(
        markers.contains("sidebar"),
        "sidebar must carry the yj.semantics.type marker: {markers:?}"
    );

    // Role survives the round trip via the marker.
    let out = boko::Book::from_bytes(&kfx, Format::Kfx).expect("import kfx");
    let ids: Vec<_> = out.spine().iter().map(|e| e.id).collect();
    let mut saw_sidebar = false;
    for id in ids {
        let ch = out.load_chapter(id).expect("load");
        for nid in ch.iter_dfs() {
            if ch.node(nid).map(|n| n.role) == Some(Role::Sidebar) {
                saw_sidebar = true;
            }
        }
    }
    assert!(saw_sidebar, "sidebar role lost through KFX round-trip");
}
