use quick_xml::{Reader, Writer};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufReader, BufWriter, Cursor, Read, Write};
use std::path::Path;
use xli_core::XliError;
use zip::read::ZipArchive;
use zip::result::ZipError;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

pub type XmlReader<'a> = Reader<Cursor<&'a [u8]>>;
pub type XmlWriter = Writer<Vec<u8>>;

pub struct WorkbookPatcher {
    reader: ZipArchive<BufReader<File>>,
    writer: ZipWriter<BufWriter<File>>,
    patched: HashMap<String, Vec<u8>>,
    touched: HashSet<String>,
}

impl WorkbookPatcher {
    pub fn open(src: &Path, dst: &Path) -> Result<Self, XliError> {
        let src_file = BufReader::new(File::open(src).map_err(io_error)?);
        let dst_file = BufWriter::new(File::create(dst).map_err(io_error)?);
        let reader = ZipArchive::new(src_file).map_err(zip_error)?;
        let writer = ZipWriter::new(dst_file);

        Ok(Self {
            reader,
            writer,
            patched: HashMap::new(),
            touched: HashSet::new(),
        })
    }

    pub fn patch_part<F>(&mut self, part: &str, f: F) -> Result<(), XliError>
    where
        F: FnOnce(XmlReader<'_>, &mut XmlWriter) -> Result<(), XliError>,
    {
        let mut file = self.reader.by_name(part).map_err(zip_error)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).map_err(io_error)?;
        let reader = Reader::from_reader(Cursor::new(bytes.as_slice()));
        let mut writer = Writer::new(Vec::new());
        f(reader, &mut writer)?;
        self.patched.insert(part.to_string(), writer.into_inner());
        self.touched.insert(part.to_string());
        Ok(())
    }

    pub fn finalize(mut self) -> Result<(), XliError> {
        for index in 0..self.reader.len() {
            let file = self.reader.by_index(index).map_err(zip_error)?;
            let name = file.name().to_string();
            if self.touched.contains(&name) {
                let options = SimpleFileOptions::default()
                    .compression_method(file.compression())
                    .unix_permissions(file.unix_mode().unwrap_or(0o644));
                self.writer.start_file(&name, options).map_err(zip_error)?;
                if let Some(bytes) = self.patched.get(&name) {
                    self.writer.write_all(bytes).map_err(io_error)?;
                }
            } else {
                self.writer.raw_copy_file(file).map_err(zip_error)?;
            }
        }

        self.writer.finish().map_err(zip_error)?;
        Ok(())
    }
}

fn io_error(error: std::io::Error) -> XliError {
    XliError::OoxmlCorrupt {
        details: error.to_string(),
    }
}

fn zip_error(error: ZipError) -> XliError {
    XliError::OoxmlCorrupt {
        details: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::WorkbookPatcher;
    use quick_xml::events::{BytesText, Event};
    use rust_xlsxwriter::Workbook;
    use std::fs::File;
    use std::io::Read;
    use tempfile::tempdir;
    use zip::ZipArchive;

    #[test]
    fn passthrough_finalize_produces_valid_workbook() {
        let dir = tempdir().expect("tempdir");
        let src = dir.path().join("input.xlsx");
        let dst = dir.path().join("output.xlsx");
        let mut workbook = Workbook::new();
        workbook
            .add_worksheet()
            .write_string(0, 0, "hello")
            .expect("write");
        workbook.save(&src).expect("save");

        let patcher = WorkbookPatcher::open(&src, &dst).expect("open");
        patcher.finalize().expect("finalize");

        let file = File::open(&dst).expect("open");
        let mut archive = ZipArchive::new(file).expect("zip");
        archive.by_name("xl/workbook.xml").expect("workbook");
    }

    #[test]
    fn patch_part_rewrites_only_named_part() {
        let dir = tempdir().expect("tempdir");
        let src = dir.path().join("input.xlsx");
        let dst = dir.path().join("output.xlsx");
        let mut workbook = Workbook::new();
        workbook
            .add_worksheet()
            .write_string(0, 0, "hello")
            .expect("write");
        workbook.save(&src).expect("save");

        let mut patcher = WorkbookPatcher::open(&src, &dst).expect("open");
        patcher
            .patch_part("docProps/app.xml", |mut reader, writer| {
                let mut buffer = Vec::new();
                loop {
                    buffer.clear();
                    match reader.read_event_into(&mut buffer) {
                        Ok(Event::Text(text)) if text.as_ref() == b"Microsoft Excel" => {
                            writer
                                .write_event(Event::Text(BytesText::new("XLI Test")))
                                .map_err(|error| xli_core::XliError::OoxmlCorrupt {
                                    details: error.to_string(),
                                })?;
                        }
                        Ok(Event::Eof) => break,
                        Ok(event) => {
                            writer.write_event(event).map_err(|error| {
                                xli_core::XliError::OoxmlCorrupt {
                                    details: error.to_string(),
                                }
                            })?;
                        }
                        Err(error) => {
                            return Err(xli_core::XliError::OoxmlCorrupt {
                                details: error.to_string(),
                            })
                        }
                    }
                }
                Ok(())
            })
            .expect("patch");
        patcher.finalize().expect("finalize");

        let mut archive = ZipArchive::new(File::open(&dst).expect("open")).expect("zip");
        let mut app = String::new();
        archive
            .by_name("docProps/app.xml")
            .expect("app")
            .read_to_string(&mut app)
            .expect("read");
        assert!(app.contains("XLI Test"));
    }

    #[test]
    fn missing_part_returns_error() {
        let dir = tempdir().expect("tempdir");
        let src = dir.path().join("input.xlsx");
        let dst = dir.path().join("output.xlsx");
        let mut workbook = Workbook::new();
        workbook.save(&src).expect("save");

        let mut patcher = WorkbookPatcher::open(&src, &dst).expect("open");
        let error = patcher
            .patch_part("missing.xml", |_, _| Ok(()))
            .expect_err("missing");
        assert!(matches!(error, xli_core::XliError::OoxmlCorrupt { .. }));
    }
}
