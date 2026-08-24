//! ISIS3-style PDS3 text label: encode/decode a flat per-image
//! [`StorageModel`] to/from the label text block.
//!
//! Mirrors `libptiff/src/io/backend/isis/isis_label.cpp`. Deliberately a
//! compact, self-describing subset of ISIS3 (see ROADMAP M3) -- it mirrors
//! the structure (`Object=IsisCube` -> `Object=Core` -> `Group=Dimensions`/
//! `Pixels`) without implementing the full USGS data dictionary.

use std::collections::HashMap;

use crate::io::StorageModel;
use crate::{Error, Result};

/// ptiff `pixelType` -> ISIS3 PDS3 `Type` keyword value.
fn pixel_type_to_isis(pt: &str) -> Option<&'static str> {
    match pt {
        "UInt8" => Some("UnsignedByte"),
        "UInt16" => Some("UnsignedWord"),
        "UInt32" => Some("UnsignedInteger"),
        "Float32" => Some("Real"),
        "Float64" => Some("Double"),
        _ => None,
    }
}

/// ISIS3 `Type` keyword -> ptiff `pixelType`, or `None` if unrecognized.
fn isis_to_pixel_type(isis: &str) -> Option<&'static str> {
    match isis {
        "UnsignedByte" => Some("UInt8"),
        "UnsignedWord" => Some("UInt16"),
        "UnsignedInteger" => Some("UInt32"),
        "Real" => Some("Float32"),
        "Double" => Some("Float64"),
        _ => None,
    }
}

fn append_line(out: &mut String, text: &str) {
    out.push_str(text);
    out.push('\n');
}

/// The optional ptiff storage fields round-tripped as extra PDS3 keywords
/// inside `Core`. `compression` is represented in the `Pixels` group instead.
const EXTRA_FIELD_KEYS: [&str; 4] = ["tileWidth", "tileHeight", "predictor", "container"];

fn append_extra_fields(out: &mut String, model: &StorageModel) {
    for key in EXTRA_FIELD_KEYS {
        if let Ok(v) = model.field(key) {
            append_line(out, &format!("          {key} = \"{v}\""));
        }
    }
}

/// Parses every `key = value` assignment into a flat map, walking the label
/// line by line. A key is the trimmed text before the first `=` on the line;
/// the value is the trimmed text after it with surrounding double quotes
/// removed. Structural lines (`Object`/`Group`/`End`, which carry no `=`) are
/// naturally skipped.
fn collect_assignments(label: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for raw_line in label.split('\n') {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        let Some(eq) = line.find('=') else {
            continue;
        };
        let key = line[..eq].trim();
        let mut value = line[eq + 1..].trim();
        if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
            value = &value[1..value.len() - 1];
        }
        if !key.is_empty() {
            out.insert(key.to_string(), value.to_string());
        }
    }
    out
}

/// Serializes a flat per-image `model` into an ISIS3-style PDS3 text label.
///
/// # Errors
///
/// Returns [`crate::ErrorCode::InvalidArgument`] if a required storage field
/// is absent, or the `pixelType` is unsupported.
pub fn write_label(model: &StorageModel) -> Result<Vec<u8>> {
    let width = model
        .field("imageWidth")
        .map_err(|_| Error::invalid_argument("isis: label requires imageWidth"))?;
    let height = model
        .field("imageHeight")
        .map_err(|_| Error::invalid_argument("isis: label requires imageHeight"))?;
    let spp = model
        .field("samplesPerPixel")
        .map_err(|_| Error::invalid_argument("isis: label requires samplesPerPixel"))?;
    let ptype = model
        .field("pixelType")
        .map_err(|_| Error::invalid_argument("isis: label requires pixelType"))?;

    let isis_type = pixel_type_to_isis(ptype)
        .ok_or_else(|| Error::invalid_argument("isis: unsupported pixelType in label"))?;
    let compression = model.field("compression").unwrap_or("None");

    let mut out = String::new();
    append_line(&mut out, "Object = IsisCube");
    append_line(&mut out, "  Object = Core");
    append_line(&mut out, "    Group = Dimensions");
    append_line(&mut out, &format!("      Samples = {width}"));
    append_line(&mut out, &format!("      Lines = {height}"));
    append_line(&mut out, &format!("      Bands = {spp}"));
    append_line(&mut out, "    End_Group");
    append_line(&mut out, "    Group = Pixels");
    append_line(&mut out, &format!("      Type = {isis_type}"));
    append_line(&mut out, "      ByteOrder = Lsb");
    append_line(&mut out, &format!("      Compression = \"{compression}\""));
    append_line(&mut out, "    End_Group");
    append_extra_fields(&mut out, model);
    append_line(&mut out, "  End_Object");
    append_line(&mut out, "End_Object");
    append_line(&mut out, "End");

    Ok(out.into_bytes())
}

/// Parses an ISIS3-style label into the flat per-image [`StorageModel`] that
/// produced it (round-trips with [`write_label`]).
///
/// `label_text` is the label bytes as read from the start of the document,
/// including the trailing `End` marker (the scanner in
/// [`super::document::read_document`] determines its extent).
///
/// # Errors
///
/// Returns [`crate::ErrorCode::InvalidArgument`] if the bytes are not a
/// recognizable ISIS3 label / miss a required field.
pub fn parse_label(label_text: &[u8]) -> Result<StorageModel> {
    let label = std::str::from_utf8(label_text)
        .map_err(|_| Error::invalid_argument("isis: label is not valid UTF-8"))?;
    if !label.contains("Object = IsisCube") {
        return Err(Error::invalid_argument(
            "isis: not an ISIS3 label (missing IsisCube object)",
        ));
    }

    let kv = collect_assignments(label);
    let get = |key: &str| kv.get(key);

    let w = get("Samples")
        .ok_or_else(|| Error::invalid_argument("isis: label missing Dimensions/Pixels values"))?;
    let h = get("Lines")
        .ok_or_else(|| Error::invalid_argument("isis: label missing Dimensions/Pixels values"))?;
    let b = get("Bands")
        .ok_or_else(|| Error::invalid_argument("isis: label missing Dimensions/Pixels values"))?;
    let t = get("Type")
        .ok_or_else(|| Error::invalid_argument("isis: label missing Dimensions/Pixels values"))?;
    let iso_type = isis_to_pixel_type(t)
        .ok_or_else(|| Error::invalid_argument("isis: unrecognized pixel Type in label"))?;

    let mut model = StorageModel::new();
    model.set_field("imageWidth", w.clone());
    model.set_field("imageHeight", h.clone());
    model.set_field("samplesPerPixel", b.clone());
    model.set_field("pixelType", iso_type);
    match get("Compression") {
        Some(c) => model.set_field("compression", c.clone()),
        None => model.set_field("compression", "None"),
    }
    // Optional ptiff fields (round-trip with write_label).
    for key in EXTRA_FIELD_KEYS {
        if let Some(v) = get(key) {
            model.set_field(key, v.clone());
        }
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
        assert!(text.starts_with("Object = IsisCube\n"));
        assert!(text.contains("Samples = 32"));
        assert!(text.contains("Lines = 32"));
        assert!(text.contains("Bands = 1"));
        assert!(text.contains("Type = UnsignedByte"));
        assert!(text.contains("Compression = \"None\""));
        assert!(text.ends_with("End_Object\nEnd_Object\nEnd\n"));
    }

    #[test]
    fn write_label_rejects_missing_required_field() {
        let m = StorageModel::new();
        let err = write_label(&m).expect_err("must fail");
        assert_eq!(err.code(), crate::ErrorCode::InvalidArgument);
    }

    #[test]
    fn write_label_rejects_unsupported_pixel_type() {
        let mut m = image_model();
        m.set_field("pixelType", "Bogus");
        let err = write_label(&m).expect_err("must fail");
        assert_eq!(err.code(), crate::ErrorCode::InvalidArgument);
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
    fn parse_label_rejects_non_isis_text() {
        let err = parse_label(b"not a label").expect_err("must fail");
        assert_eq!(err.code(), crate::ErrorCode::InvalidArgument);
    }

    #[test]
    fn parse_label_rejects_unrecognized_pixel_type() {
        let mut model = image_model();
        model.set_field("pixelType", "Float32");
        let label = write_label(&model).expect("write_label");
        let mut text = String::from_utf8(label).unwrap();
        text = text.replace("Type = Real", "Type = Weird");
        let err = parse_label(text.as_bytes()).expect_err("must fail");
        assert_eq!(err.code(), crate::ErrorCode::InvalidArgument);
    }

    #[test]
    fn compression_defaults_to_none_when_absent_from_label() {
        let text = "Object = IsisCube\n  Object = Core\n    Group = Dimensions\n      Samples = 4\n      Lines = 4\n      Bands = 1\n    End_Group\n    Group = Pixels\n      Type = UnsignedByte\n      ByteOrder = Lsb\n    End_Group\n  End_Object\nEnd_Object\nEnd\n";
        let parsed = parse_label(text.as_bytes()).expect("parse_label");
        assert_eq!(parsed.field("compression").unwrap(), "None");
    }
}
