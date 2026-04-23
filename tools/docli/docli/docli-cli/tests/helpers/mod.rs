//! Programmatic DOCX fixture builders for integration tests.
#![allow(dead_code)]

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use zip::write::SimpleFileOptions;
use zip::ZipWriter;

/// Build `simple.docx` — a document with headings, normal paragraphs, a table, and a bookmark.
pub fn build_simple_docx() -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/simple.docx");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }

    let file = File::create(&path).unwrap();
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default();

    // [Content_Types].xml
    zip.start_file("[Content_Types].xml", options).unwrap();
    zip.write_all(CONTENT_TYPES_XML.as_bytes()).unwrap();

    // _rels/.rels
    zip.start_file("_rels/.rels", options).unwrap();
    zip.write_all(RELS_XML.as_bytes()).unwrap();

    // word/document.xml
    zip.start_file("word/document.xml", options).unwrap();
    zip.write_all(SIMPLE_DOCUMENT_XML.as_bytes()).unwrap();

    // word/_rels/document.xml.rels
    zip.start_file("word/_rels/document.xml.rels", options)
        .unwrap();
    zip.write_all(DOC_RELS_XML.as_bytes()).unwrap();

    zip.finish().unwrap();
    path
}

/// Build `reviewed.docx` — a document with comments and tracked changes.
pub fn build_reviewed_docx() -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reviewed.docx");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }

    let file = File::create(&path).unwrap();
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default();

    // [Content_Types].xml
    zip.start_file("[Content_Types].xml", options).unwrap();
    zip.write_all(CONTENT_TYPES_XML.as_bytes()).unwrap();

    // _rels/.rels
    zip.start_file("_rels/.rels", options).unwrap();
    zip.write_all(RELS_XML.as_bytes()).unwrap();

    // word/document.xml
    zip.start_file("word/document.xml", options).unwrap();
    zip.write_all(REVIEWED_DOCUMENT_XML.as_bytes()).unwrap();

    // word/_rels/document.xml.rels
    zip.start_file("word/_rels/document.xml.rels", options)
        .unwrap();
    zip.write_all(DOC_RELS_XML.as_bytes()).unwrap();

    zip.finish().unwrap();
    path
}

// ---------------------------------------------------------------------------
// Raw OOXML templates
// ---------------------------------------------------------------------------

const CONTENT_TYPES_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#;

const RELS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#;

const DOC_RELS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
</Relationships>"#;

const SIMPLE_DOCUMENT_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body>
    <w:p>
      <w:pPr><w:pStyle w:val="Heading1"/></w:pPr>
      <w:r><w:t>Introduction</w:t></w:r>
    </w:p>
    <w:p>
      <w:r><w:t>This is the first paragraph of the document.</w:t></w:r>
    </w:p>
    <w:p>
      <w:pPr><w:pStyle w:val="Heading2"/></w:pPr>
      <w:r><w:t>Details</w:t></w:r>
    </w:p>
    <w:p>
      <w:bookmarkStart w:id="0" w:name="important_section"/>
      <w:r><w:t>This paragraph contains a bookmark.</w:t></w:r>
      <w:bookmarkEnd w:id="0"/>
    </w:p>
    <w:tbl>
      <w:tr>
        <w:tc><w:p><w:r><w:t>A1</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>B1</w:t></w:r></w:p></w:tc>
      </w:tr>
      <w:tr>
        <w:tc><w:p><w:r><w:t>A2</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>B2</w:t></w:r></w:p></w:tc>
      </w:tr>
    </w:tbl>
  </w:body>
</w:document>"#;

const REVIEWED_DOCUMENT_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
            xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml">
  <w:body>
    <w:p>
      <w:pPr><w:pStyle w:val="Heading1"/></w:pPr>
      <w:r><w:t>Document Under Review</w:t></w:r>
    </w:p>
    <w:p>
      <w:commentRangeStart w:id="1"/>
      <w:r><w:t>This text has a comment attached to it.</w:t></w:r>
      <w:commentRangeEnd w:id="1"/>
      <w:r>
        <w:rPr><w:rStyle w:val="CommentReference"/></w:rPr>
        <w:commentReference w:id="1"/>
      </w:r>
    </w:p>
    <w:p>
      <w:ins w:id="2" w:author="Alice" w:date="2025-01-15T10:00:00Z">
        <w:r><w:t>This sentence was inserted by Alice.</w:t></w:r>
      </w:ins>
    </w:p>
    <w:p>
      <w:del w:id="3" w:author="Bob" w:date="2025-01-16T14:30:00Z">
        <w:r><w:delText>This sentence was deleted by Bob.</w:delText></w:r>
      </w:del>
    </w:p>
    <w:p>
      <w:r><w:t>Final unchanged paragraph.</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
