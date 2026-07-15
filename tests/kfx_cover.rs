//! Regression coverage for EPUB cover metadata and Kindle-compatible KFX media.

mod common;

use boko::Book;
use boko::model::Format;
use common::{Doc, EpubBuilder, export_to_bytes};

#[test]
fn epub_png_cover_becomes_jfif_kfx_cover_resource() {
    let mut epub = EpubBuilder::new("KFX Cover Test")
        .cover_png()
        .doc(Doc::new(
            "text/chapter.xhtml",
            "Chapter",
            "<h1>Chapter</h1><p>Body text.</p>",
        ))
        .book();

    let kfx_bytes = export_to_bytes(&mut epub, Format::Kfx);
    let mut kfx = Book::from_bytes(&kfx_bytes, Format::Kfx).expect("re-import exported KFX");

    let cover_name = kfx
        .metadata()
        .cover_image
        .clone()
        .expect("KFX cover_image metadata");
    let cover = kfx
        .load_asset(&cover_name)
        .expect("load cover resource named by KFX metadata");

    assert!(cover.starts_with(&[0xff, 0xd8]), "cover must be JPEG");
    assert!(
        cover.windows(5).any(|window| window == b"JFIF\0"),
        "cover must contain a JFIF header"
    );
}
