use docli_core::DocliError;
use quick_xml::{
    events::{BytesStart, Event},
    Reader, Writer,
};

pub fn ensure_xml_space_preserve(xml: &[u8]) -> Result<Vec<u8>, DocliError> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);
    let mut writer = Writer::new(Vec::new());
    let mut buf = Vec::new();
    let mut pending_text = None::<BytesStart<'static>>;

    loop {
        match reader
            .read_event_into(&mut buf)
            .map_err(|error| DocliError::InvalidDocx {
                message: error.to_string(),
            })? {
            Event::Start(start) if start.local_name().as_ref() == b"t" => {
                pending_text = Some(start.into_owned());
            }
            Event::Text(text) => {
                if let Some(start) = pending_text.take() {
                    let content = text.unescape().map_err(|error| DocliError::InvalidDocx {
                        message: error.to_string(),
                    })?;
                    let needs_preserve = content.starts_with(char::is_whitespace)
                        || content.ends_with(char::is_whitespace);
                    let start = if needs_preserve && !has_xml_space(&start) {
                        with_attribute(start, ("xml:space", "preserve"))
                    } else {
                        start
                    };
                    writer.write_event(Event::Start(start)).map_err(|error| {
                        DocliError::InvalidDocx {
                            message: error.to_string(),
                        }
                    })?;
                }
                writer
                    .write_event(Event::Text(text))
                    .map_err(|error| DocliError::InvalidDocx {
                        message: error.to_string(),
                    })?;
            }
            Event::End(end) => {
                if let Some(start) = pending_text.take() {
                    writer.write_event(Event::Start(start)).map_err(|error| {
                        DocliError::InvalidDocx {
                            message: error.to_string(),
                        }
                    })?;
                }
                writer
                    .write_event(Event::End(end))
                    .map_err(|error| DocliError::InvalidDocx {
                        message: error.to_string(),
                    })?;
            }
            Event::Empty(start) => {
                let repaired = repair_id_attributes(start.into_owned());
                writer
                    .write_event(Event::Empty(repaired))
                    .map_err(|error| DocliError::InvalidDocx {
                        message: error.to_string(),
                    })?;
            }
            Event::Start(start) => {
                let repaired = repair_id_attributes(start.into_owned());
                writer
                    .write_event(Event::Start(repaired))
                    .map_err(|error| DocliError::InvalidDocx {
                        message: error.to_string(),
                    })?;
            }
            Event::Eof => break,
            event => {
                writer
                    .write_event(event)
                    .map_err(|error| DocliError::InvalidDocx {
                        message: error.to_string(),
                    })?;
            }
        }
        buf.clear();
    }

    Ok(writer.into_inner())
}

pub fn repair_durable_id_overflow(xml: &[u8]) -> Result<Vec<u8>, DocliError> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);
    let mut writer = Writer::new(Vec::new());
    let mut buf = Vec::new();

    loop {
        match reader
            .read_event_into(&mut buf)
            .map_err(|error| DocliError::InvalidDocx {
                message: error.to_string(),
            })? {
            Event::Start(start) => writer
                .write_event(Event::Start(repair_id_attributes(start.into_owned())))
                .map_err(|error| DocliError::InvalidDocx {
                    message: error.to_string(),
                })?,
            Event::Empty(start) => writer
                .write_event(Event::Empty(repair_id_attributes(start.into_owned())))
                .map_err(|error| DocliError::InvalidDocx {
                    message: error.to_string(),
                })?,
            Event::Eof => break,
            event => writer
                .write_event(event)
                .map_err(|error| DocliError::InvalidDocx {
                    message: error.to_string(),
                })?,
        }
        buf.clear();
    }

    Ok(writer.into_inner())
}

fn has_xml_space(start: &BytesStart<'_>) -> bool {
    start
        .attributes()
        .flatten()
        .any(|attribute| attribute.key.as_ref() == b"xml:space")
}

fn with_attribute(mut start: BytesStart<'static>, attr: (&str, &str)) -> BytesStart<'static> {
    start.push_attribute(attr);
    start
}

fn repair_id_attributes(start: BytesStart<'static>) -> BytesStart<'static> {
    let name = start.name().as_ref().to_vec();
    let mut rebuilt = BytesStart::new(String::from_utf8_lossy(&name).into_owned());
    let mut next_generated = 0x1000_0000_u32;

    for attribute in start.attributes().flatten() {
        let key = attribute.key.as_ref().to_vec();
        let value = String::from_utf8_lossy(attribute.value.as_ref()).into_owned();
        let repaired = if key.ends_with(b"paraId") || key.ends_with(b"durableId") {
            repair_id_value(&value, &mut next_generated)
        } else {
            value
        };
        let key = String::from_utf8_lossy(&key).into_owned();
        rebuilt.push_attribute((key.as_str(), repaired.as_str()));
    }

    rebuilt
}

fn repair_id_value(value: &str, next_generated: &mut u32) -> String {
    let parsed = u32::from_str_radix(value, 16).unwrap_or(0);
    if parsed < 0x7FFF_FFFF {
        return value.to_string();
    }

    let repaired = format!("{:08X}", *next_generated);
    *next_generated += 1;
    repaired
}

#[cfg(test)]
mod tests {
    use super::{ensure_xml_space_preserve, repair_durable_id_overflow};

    #[test]
    fn adds_xml_space_preserve_for_whitespace_text_nodes() {
        let input = br#"<w:document xmlns:w="x"><w:body><w:p><w:r><w:t> leading</w:t></w:r></w:p></w:body></w:document>"#;
        let repaired = String::from_utf8(ensure_xml_space_preserve(input).unwrap()).unwrap();

        assert!(repaired.contains(r#"xml:space="preserve""#));
    }

    #[test]
    fn repairs_overflowing_para_ids() {
        let input = br#"<w:document xmlns:w14="x"><w:p w14:paraId="FFFFFFFF"/></w:document>"#;
        let repaired = String::from_utf8(repair_durable_id_overflow(input).unwrap()).unwrap();

        assert!(repaired.contains(r#"w14:paraId="10000000""#));
    }
}
