use docli_core::{ContentBlock, DocliError, ParagraphContent};
use docx_rs::*;

use crate::{backend::CreateBackend, spec::CreateSpec};

/// Backend that uses the `docx-rs` crate to create DOCX files.
pub struct DocxRsBackend;

impl CreateBackend for DocxRsBackend {
    fn create(&self, spec: &CreateSpec) -> Result<Vec<u8>, DocliError> {
        let mut docx = Docx::new();

        // Build document from content blocks
        for block in &spec.content {
            docx = append_block(docx, block);
        }

        // Serialize to bytes
        let mut buf = Vec::new();
        docx.build()
            .pack(&mut std::io::Cursor::new(&mut buf))
            .map_err(|e| DocliError::CommitFailed {
                message: e.to_string(),
            })?;
        Ok(buf)
    }
}

fn append_block(mut docx: Docx, block: &ContentBlock) -> Docx {
    match block {
        ContentBlock::Heading1 { heading1 } => {
            docx = docx.add_paragraph(
                Paragraph::new()
                    .add_run(Run::new().add_text(heading1))
                    .style("Heading1"),
            );
        }
        ContentBlock::Heading2 { heading2 } => {
            docx = docx.add_paragraph(
                Paragraph::new()
                    .add_run(Run::new().add_text(heading2))
                    .style("Heading2"),
            );
        }
        ContentBlock::Heading3 { heading3 } => {
            docx = docx.add_paragraph(
                Paragraph::new()
                    .add_run(Run::new().add_text(heading3))
                    .style("Heading3"),
            );
        }
        ContentBlock::Paragraph { paragraph } => match paragraph {
            ParagraphContent::Text(text) => {
                docx = docx
                    .add_paragraph(Paragraph::new().add_run(Run::new().add_text(text)));
            }
            ParagraphContent::Block(block) => {
                let mut para = Paragraph::new();
                if let Some(ref style) = block.style {
                    para = para.style(style);
                }
                for run in &block.runs {
                    match run {
                        docli_core::InlineRun::Text(text_run) => {
                            let mut r = Run::new().add_text(&text_run.text);
                            if text_run.bold {
                                r = r.bold();
                            }
                            if text_run.italic {
                                r = r.italic();
                            }
                            para = para.add_run(r);
                        }
                        _ => {} // footnote, link — skip for now
                    }
                }
                docx = docx.add_paragraph(para);
            }
        },
        ContentBlock::Bullets { bullets } => {
            for item in bullets {
                docx = docx.add_paragraph(
                    Paragraph::new()
                        .add_run(Run::new().add_text(item))
                        .style("ListBullet"),
                );
            }
        }
        ContentBlock::Numbers { numbers } => {
            for item in numbers {
                docx = docx.add_paragraph(
                    Paragraph::new()
                        .add_run(Run::new().add_text(item))
                        .style("ListNumber"),
                );
            }
        }
        ContentBlock::Table { table } => {
            let mut rows = Vec::new();
            if !table.headers.is_empty() {
                let header_cells: Vec<TableCell> = table
                    .headers
                    .iter()
                    .map(|h| {
                        TableCell::new()
                            .add_paragraph(Paragraph::new().add_run(Run::new().add_text(h).bold()))
                    })
                    .collect();
                rows.push(TableRow::new(header_cells));
            }
            for row_data in &table.rows {
                let cells: Vec<TableCell> = row_data
                    .iter()
                    .map(|c| {
                        TableCell::new()
                            .add_paragraph(Paragraph::new().add_run(Run::new().add_text(c)))
                    })
                    .collect();
                rows.push(TableRow::new(cells));
            }
            docx = docx.add_table(Table::new(rows));
        }
        ContentBlock::PageBreak { page_break: true } => {
            docx = docx.add_paragraph(
                Paragraph::new().add_run(Run::new().add_break(BreakType::Page)),
            );
        }
        _ => {} // Image, Toc, Columns, Ref, PageBreak(false) — skip or stub
    }
    docx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_nonempty_docx_from_simple_spec() {
        let yaml = r#"
content:
  - heading1: "Title"
  - paragraph: "A paragraph."
  - bullets:
      - "First"
      - "Second"
  - table:
      headers: ["Name", "Value"]
      rows:
        - ["a", "1"]
        - ["b", "2"]
"#;
        let spec = crate::spec::CreateSpec::from_yaml(yaml).unwrap();
        let backend = DocxRsBackend;
        let bytes = backend.create(&spec).unwrap();
        assert!(!bytes.is_empty());
        // DOCX is a zip; check for PK magic bytes.
        assert_eq!(&bytes[0..2], b"PK");
    }
}
