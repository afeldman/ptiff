//! PDS4-style XML label: encode/decode a flat per-image [`StorageModel`]
//! to/from the label text block.
//!
//! Mirrors `libptiff/src/io/backend/pds4/pds4_label.cpp`. Deliberately a
//! compact, self-describing subset of PDS4 (see ROADMAP M3 "no PDS4-archival
//! replacement"): a `Product_Observational` root holding one
//! `Array_2D_Image` element whose children are the format-neutral storage
//! fields (element name == storage key, element text == value). Keeping the
//! element name equal to the storage key round-trips losslessly.
//!
//! Unlike the ISIS3 label writer, [`write_label`] performs no field
//! validation (mirrors the C++ `writeLabel`, which just serializes whatever
//! fields are present); required-field / pixel-geometry validation happens
//! one layer up, in `memory::image_info_from_model`.

use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::reader::Reader;
use quick_xml::writer::Writer;

use crate::io::StorageModel;
use crate::{Error, Result};

const PRODUCT_ROOT: &str = "Product_Observational";
const ARRAY_NODE: &str = "Array_2D_Image";

/// Byte length of the seek header that precedes the XML label in a PDS4
/// document (mirrors `pds4::kHeaderSize`, `sizeof(std::uint64_t)`).
pub const HEADER_SIZE: u64 = std::mem::size_of::<u64>() as u64;

fn xml_write_err(e: std::io::Error) -> Error {
    Error::invalid_argument(format!("pds4: failed to write label: {e}"))
}

/// Serializes a flat per-image `model` into a PDS4-style XML label byte
/// block (not including the seek header).
///
/// # Errors
///
/// Currently infallible for any model shape (mirrors the C++ `writeLabel`,
/// which performs no field validation); the `Result` is kept for parity with
/// the C++ signature and to leave room for future encode failures (e.g. keys
/// that are not valid XML element names).
pub fn write_label(model: &StorageModel) -> Result<Vec<u8>> {
    let mut writer = Writer::new(Vec::new());

    writer
        .write_event(Event::Start(BytesStart::new(PRODUCT_ROOT)))
        .map_err(xml_write_err)?;
    writer
        .write_event(Event::Start(BytesStart::new(ARRAY_NODE)))
        .map_err(xml_write_err)?;

    let mut field_err: Option<Error> = None;
    model.for_each_field(|key, value| {
        if field_err.is_some() {
            return;
        }
        let result: std::result::Result<(), std::io::Error> = (|| {
            writer.write_event(Event::Start(BytesStart::new(key)))?;
            writer.write_event(Event::Text(BytesText::new(value)))?;
            writer.write_event(Event::End(BytesEnd::new(key)))?;
            Ok(())
        })();
        if let Err(e) = result {
            field_err = Some(xml_write_err(e));
        }
    });
    if let Some(e) = field_err {
        return Err(e);
    }

    writer
        .write_event(Event::End(BytesEnd::new(ARRAY_NODE)))
        .map_err(xml_write_err)?;
    writer
        .write_event(Event::End(BytesEnd::new(PRODUCT_ROOT)))
        .map_err(xml_write_err)?;

    Ok(writer.into_inner())
}

/// Parses a PDS4-style XML label into the flat per-image [`StorageModel`]
/// that produced it (round-trips with [`write_label`]).
///
/// `label_bytes` are the label bytes read from offset [`HEADER_SIZE`] in the
/// document.
///
/// # Errors
///
/// Returns [`crate::ErrorCode::InvalidArgument`] if the bytes are empty, not
/// valid UTF-8, not well-formed XML, or are missing the
/// `Product_Observational` root / `Array_2D_Image` element.
pub fn parse_label(label_bytes: &[u8]) -> Result<StorageModel> {
    if label_bytes.is_empty() {
        return Err(Error::invalid_argument("pds4: empty label"));
    }
    let text = std::str::from_utf8(label_bytes)
        .map_err(|_| Error::invalid_argument("pds4: label is not valid UTF-8"))?;

    let mut reader = Reader::from_str(text);
    // trim_text must stay disabled: quick-xml 0.41 emits entity references
    // (`&amp;` & co.) as separate `GeneralRef` events, and trimming the
    // individual text nodes around them would drop significant whitespace.
    // Leading/trailing outer whitespace is insignificant for the flat label
    // subset we parse (element names == storage keys).
    reader.config_mut().trim_text(false);

    let mut model = StorageModel::new();
    let mut seen_product = false;
    let mut in_product = false;
    let mut seen_array = false;
    let mut in_array = false;
    let mut current_key: Option<String> = None;
    let mut current_value = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                if in_array {
                    current_key = Some(name);
                    current_value.clear();
                } else if in_product && name == ARRAY_NODE {
                    in_array = true;
                    seen_array = true;
                } else if !in_product && name == PRODUCT_ROOT {
                    in_product = true;
                    seen_product = true;
                }
            }
            Ok(Event::Empty(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                if in_array {
                    model.set_field(name, "");
                } else if in_product && name == ARRAY_NODE {
                    seen_array = true;
                } else if !in_product && name == PRODUCT_ROOT {
                    seen_product = true;
                }
            }
            Ok(Event::Text(t)) => {
                if in_array && current_key.is_some() {
                    let text = t.xml10_content().map_err(|e| {
                        Error::invalid_argument(format!("pds4: malformed XML label: {e}"))
                    })?;
                    current_value.push_str(text.as_ref());
                }
            }
            Ok(Event::GeneralRef(r)) => {
                // quick-xml 0.41 emits named/numeric entity references
                // (e.g. `&amp;`) as `GeneralRef` events instead of folding
                // them into the surrounding `Text` event. Reconstruct the
                // `&name;` reference and resolve it with the escape module so
                // entity-escaped field values round-trip identically to the
                // pre-0.41 `BytesText::unescape()` behaviour.
                if in_array && current_key.is_some() {
                    let name = std::str::from_utf8(r.as_ref()).map_err(|_| {
                        Error::invalid_argument("pds4: label contains a non-UTF-8 entity reference")
                    })?;
                    let mut reference = String::with_capacity(name.len() + 2);
                    reference.push('&');
                    reference.push_str(name);
                    reference.push(';');
                    let resolved = quick_xml::escape::unescape(&reference).map_err(|e| {
                        Error::invalid_argument(format!("pds4: malformed XML label: {e}"))
                    })?;
                    current_value.push_str(resolved.as_ref());
                }
            }
            Ok(Event::End(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                if in_array && current_key.as_deref() == Some(name.as_str()) {
                    let key = current_key.take().expect("checked above");
                    model.set_field(key, std::mem::take(&mut current_value));
                } else if in_array && name == ARRAY_NODE {
                    in_array = false;
                } else if in_product && name == PRODUCT_ROOT {
                    in_product = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(Error::invalid_argument(format!(
                    "pds4: malformed XML label: {e}"
                )))
            }
            _ => {}
        }
    }

    if !seen_product {
        return Err(Error::invalid_argument(
            "pds4: label has no Product_Observational root",
        ));
    }
    if !seen_array {
        return Err(Error::invalid_argument(
            "pds4: label has no Array_2D_Image element",
        ));
    }

    Ok(model)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image_model() -> StorageModel {
        let mut m = StorageModel::new();
        m.set_field("imageWidth", "32");
        m.set_field("imageHeight", "32");
        m.set_field("tileWidth", "16");
        m.set_field("tileHeight", "16");
        m.set_field("samplesPerPixel", "1");
        m.set_field("pixelType", "UInt8");
        m.set_field("compression", "None");
        m
    }

    #[test]
    fn write_label_contains_expected_structure() {
        let label = write_label(&image_model()).expect("write_label");
        let text = String::from_utf8(label).expect("utf8");
        assert!(text.starts_with("<Product_Observational><Array_2D_Image>"));
        assert!(text.contains("<imageWidth>32</imageWidth>"));
        assert!(text.contains("<imageHeight>32</imageHeight>"));
        assert!(text.contains("<pixelType>UInt8</pixelType>"));
        assert!(text.ends_with("</Array_2D_Image></Product_Observational>"));
    }

    #[test]
    fn write_label_does_not_validate_fields() {
        // Mirrors the C++ writeLabel: no required-field validation at this layer.
        let m = StorageModel::new();
        let label = write_label(&m).expect("write_label must not fail on an empty model");
        let text = String::from_utf8(label).expect("utf8");
        assert_eq!(
            text,
            "<Product_Observational><Array_2D_Image></Array_2D_Image></Product_Observational>"
        );
    }

    #[test]
    fn label_round_trips_through_parse_and_write() {
        let model = image_model();
        let label = write_label(&model).expect("write_label");
        let parsed = parse_label(&label).expect("parse_label");
        assert_eq!(parsed.field("imageWidth").unwrap(), "32");
        assert_eq!(parsed.field("imageHeight").unwrap(), "32");
        assert_eq!(parsed.field("samplesPerPixel").unwrap(), "1");
        assert_eq!(parsed.field("pixelType").unwrap(), "UInt8");
        assert_eq!(parsed.field("tileWidth").unwrap(), "16");
        assert_eq!(parsed.field("tileHeight").unwrap(), "16");
        assert_eq!(parsed.field("compression").unwrap(), "None");
    }

    #[test]
    fn parse_label_rejects_empty_bytes() {
        let err = parse_label(&[]).expect_err("must fail");
        assert_eq!(err.code(), crate::ErrorCode::InvalidArgument);
    }

    #[test]
    fn parse_label_rejects_malformed_xml() {
        // Mismatched end-tag name: quick-xml's tokenizer rejects this outright
        // (unlike a merely-truncated document, which is caught by the missing
        // Array_2D_Image / Product_Observational checks below).
        let err = parse_label(
            b"<Product_Observational><Array_2D_Image></Product_Observational></Array_2D_Image>",
        )
        .expect_err("must fail");
        assert_eq!(err.code(), crate::ErrorCode::InvalidArgument);
    }

    #[test]
    fn parse_label_rejects_missing_product_root() {
        let err = parse_label(b"<NotAProduct/>").expect_err("must fail");
        assert_eq!(err.code(), crate::ErrorCode::InvalidArgument);
    }

    #[test]
    fn parse_label_rejects_missing_array_element() {
        let err =
            parse_label(b"<Product_Observational></Product_Observational>").expect_err("must fail");
        assert_eq!(err.code(), crate::ErrorCode::InvalidArgument);
    }

    #[test]
    fn parse_label_accepts_empty_field_value() {
        let bytes = b"<Product_Observational><Array_2D_Image><container/></Array_2D_Image></Product_Observational>";
        let parsed = parse_label(bytes).expect("parse_label");
        assert_eq!(parsed.field("container").unwrap(), "");
    }

    #[test]
    fn parse_unescapes_xml_entity_references_in_field_text() {
        // A label written by an external PDS4 producer may escape ampersands and
        // angle brackets as XML entity references. `parse_label` must resolve
        // them back to the literal characters (this is the role of
        // quick_xml's escape machinery, required by the 0.41 API).
        let bytes = b"<Product_Observational><Array_2D_Image><label>A &amp; B &lt;C&gt;</label></Array_2D_Image></Product_Observational>";
        let parsed = parse_label(bytes).expect("parse_label");
        assert_eq!(parsed.field("label").unwrap(), "A & B <C>");
    }

    #[test]
    fn parse_ignores_indentation_whitespace_between_fields() {
        // Real PDS4 labels are typically pretty-printed. With quick-xml 0.41
        // entity refs are separate `GeneralRef` events, so `trim_text` is
        // disabled; whitespace-only text nodes between elements must be
        // ignored while whitespace *inside* a field value is preserved.
        let bytes = br"<Product_Observational>
    <Array_2D_Image>
    <imageWidth>32</imageWidth>
    <imageHeight>32</imageHeight>
    <pixelType>UInt8</pixelType>
    </Array_2D_Image>
</Product_Observational>";
        let parsed = parse_label(bytes).expect("parse_label");
        assert_eq!(parsed.field("imageWidth").unwrap(), "32");
        assert_eq!(parsed.field("imageHeight").unwrap(), "32");
        assert_eq!(parsed.field("pixelType").unwrap(), "UInt8");
    }

    #[test]
    fn parse_preserves_inner_whitespace_inside_field_value() {
        // A human-edited label may pad a value with meaningless whitespace;
        // unlike the inter-element indentation, that leading/trailing space
        // inside the value is preserved as-is.
        let bytes = b"<Product_Observational><Array_2D_Image><comment> hello world </comment></Array_2D_Image></Product_Observational>";
        let parsed = parse_label(bytes).expect("parse_label");
        assert_eq!(parsed.field("comment").unwrap(), " hello world ");
    }
}
