use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use std::collections::HashMap;
use std::io::Cursor;
use xli_core::XliError;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SharedStringTable {
    strings: Vec<String>,
    index: HashMap<String, usize>,
}

impl SharedStringTable {
    pub fn parse(xml: &[u8]) -> Result<Self, XliError> {
        let mut reader = Reader::from_reader(Cursor::new(xml));
        reader.config_mut().trim_text(true);
        let mut buffer = Vec::new();
        let mut strings = Vec::new();
        let mut in_text = false;

        loop {
            buffer.clear();
            match reader.read_event_into(&mut buffer) {
                Ok(Event::Start(event)) if event.local_name().as_ref() == b"t" => {
                    in_text = true;
                }
                Ok(Event::Text(text)) if in_text => {
                    strings.push(String::from_utf8_lossy(text.as_ref()).into_owned());
                }
                Ok(Event::End(event)) if event.local_name().as_ref() == b"t" => {
                    in_text = false;
                }
                Ok(Event::Eof) => break,
                Ok(_) => {}
                Err(error) => return Err(xml_error(error)),
            }
        }

        let index = strings
            .iter()
            .enumerate()
            .map(|(idx, value): (usize, &String)| (value.clone(), idx))
            .collect();

        Ok(Self { strings, index })
    }

    pub fn get_or_append(&mut self, s: &str) -> usize {
        if let Some(index) = self.index.get(s) {
            return *index;
        }

        let index = self.strings.len();
        self.strings.push(s.to_string());
        self.index.insert(s.to_string(), index);
        index
    }

    pub fn serialize(&self) -> Result<Vec<u8>, XliError> {
        let mut writer = Writer::new(Vec::new());
        writer
            .write_event(Event::Decl(BytesDecl::new(
                "1.0",
                Some("UTF-8"),
                Some("yes"),
            )))
            .map_err(xml_error)?;

        let mut sst = BytesStart::new("sst");
        sst.push_attribute((
            "xmlns",
            "http://schemas.openxmlformats.org/spreadsheetml/2006/main",
        ));
        let count = self.strings.len().to_string();
        sst.push_attribute(("count", count.as_str()));
        sst.push_attribute(("uniqueCount", count.as_str()));
        writer.write_event(Event::Start(sst)).map_err(xml_error)?;

        for value in &self.strings {
            writer
                .write_event(Event::Start(BytesStart::new("si")))
                .map_err(xml_error)?;
            writer
                .write_event(Event::Start(BytesStart::new("t")))
                .map_err(xml_error)?;
            writer
                .write_event(Event::Text(BytesText::new(value)))
                .map_err(xml_error)?;
            writer
                .write_event(Event::End(BytesEnd::new("t")))
                .map_err(xml_error)?;
            writer
                .write_event(Event::End(BytesEnd::new("si")))
                .map_err(xml_error)?;
        }

        writer
            .write_event(Event::End(BytesEnd::new("sst")))
            .map_err(xml_error)?;
        Ok(writer.into_inner())
    }

    pub fn len(&self) -> usize {
        self.strings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }
}

fn xml_error<E: std::fmt::Display>(error: E) -> XliError {
    XliError::OoxmlCorrupt {
        details: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::SharedStringTable;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="2" uniqueCount="2"><si><t>Hello</t></si><si><t>World</t></si></sst>"#;

    #[test]
    fn parse_and_roundtrip() {
        let table = SharedStringTable::parse(XML.as_bytes()).expect("parse");
        assert_eq!(table.len(), 2);
        let serialized = table.serialize().expect("serialize");
        let serialized = String::from_utf8(serialized).expect("utf8");
        assert!(serialized.contains("uniqueCount=\"2\""));
        assert!(serialized.contains("<t>Hello</t>"));
        assert!(serialized.contains("<t>World</t>"));
    }

    #[test]
    fn get_or_append_deduplicates_existing_strings() {
        let mut table = SharedStringTable::parse(XML.as_bytes()).expect("parse");
        let index = table.get_or_append("Hello");
        assert_eq!(index, 0);
        assert_eq!(table.len(), 2);
    }

    #[test]
    fn get_or_append_appends_new_strings_at_end() {
        let mut table = SharedStringTable::parse(XML.as_bytes()).expect("parse");
        let index = table.get_or_append("Later");
        assert_eq!(index, 2);
        assert_eq!(table.get_or_append("Hello"), 0);
        let serialized = String::from_utf8(table.serialize().expect("serialize")).expect("utf8");
        assert!(serialized.contains("uniqueCount=\"3\""));
        assert!(serialized.contains("<t>Later</t>"));
    }
}
