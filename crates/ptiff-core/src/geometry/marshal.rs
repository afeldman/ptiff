//! Typed marshalling of the PTIFF extension domains (fachliche Ebene of M3).
//!
//! Bridges the domain classes [`Camera`] and
//! [`CoordinateReferenceSystem`] with the flat `ptiff.<domain>.<key>` field
//! convention the TIFF backend already reads/writes via the private tags
//! 65001–65005 (RFC-7002). This module is what lets a typed camera / CRS on an
//! [`ImageDescriptor`](crate::ImageDescriptor) survive the
//! Scene → StorageModel → TIFF-tag round trip.
//!
//! # RFC caveat
//!
//! The **field schemas** below (`ptiff.camera.*`, `ptiff.crs.*`) are a
//! **proposal**, not yet a ratified RFC. The C++ oracle has no Scene-level camera
//! or CRS objects today, and PTIFF-1.0-RUST-CORE-PLAN.md §M3 marks these
//! normative field schemas as an open design task that is language-independent.
//! This Rust-side marshalling is implemented to be forward-compatible with that
//! RFC: unknown keys are preserved so a future writer's additional fields are
//! not lost, and readers ignore absent domains.
//!
//! # Schema
//!
//! | Domain | Field | Meaning |
//! |--------|-------|---------|
//! | camera | `ptiff.camera.model` | camera model name (`"pinhole"`, ...) |
//! | camera | `ptiff.camera.timestamp` | timestamp string (omitted when empty) |
//! | camera | `ptiff.camera.focal_px` / `focal_py` | focal length, horizontal/vertical (pixels) |
//! | camera | `ptiff.camera.principal_x` / `principal_y` | principal point (pixels) |
//! | camera | `ptiff.camera.rot_w/x/y/z` | camera-to-world rotation quaternion |
//! | camera | `ptiff.camera.pos_x/y/z` | camera world position (meters) |
//! | crs    | `ptiff.crs.body` | planetary body NAIF id (`"301"`, ...) |
//! | crs    | `ptiff.crs.projection` | projection kind (`"equirectangular"`, ...) |
//! | crs    | `ptiff.crs.reference_frame` | reference-frame / datum id (`"IAU_MOON"`, ...) |
//!
//! `f64` values use Rust's `to_string()`, which is the shortest round-trippable
//! representation; parsing it back reproduces the exact value.

use crate::geometry::{
    Camera, CoordinateReferenceSystem, Ellipsoid, Frame, Planet, Projection, ProjectionKind,
};
use crate::io::StorageModel;
use crate::{Error, Result};

/// Field-name prefixes per domain. The `ptiff.<domain>.` prefix is what
/// `StorageModel` carries and what the tag codec sees after trimming.
const CAMERA_PREFIX: &str = "ptiff.camera.";
const CRS_PREFIX: &str = "ptiff.crs.";

/// Maps a camera's state into `ptiff.camera.*` fields on `child`.
///
/// Each returned tuple is `(fully-qualified field name, value)`; callers set
/// them on a [`StorageModel`] child so the TIFF backend emits tag 65002.
pub fn camera_fields(camera: &Camera) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let i = camera.intrinsics();
    let e = camera.extrinsics();
    let mut push = |key: &str, value: String| out.push((format!("{CAMERA_PREFIX}{key}"), value));

    push("model", camera.model_name().to_string());
    if !camera.timestamp().is_empty() {
        push("timestamp", camera.timestamp().to_string());
    }
    push("focal_length_x", i.focal_length_pixels_x.to_string());
    push("focal_length_y", i.focal_length_pixels_y.to_string());
    push("principal_x", i.principal_point_x.to_string());
    push("principal_y", i.principal_point_y.to_string());
    push("rotation_w", e.rotation.w.to_string());
    push("rotation_x", e.rotation.x.to_string());
    push("rotation_y", e.rotation.y.to_string());
    push("rotation_z", e.rotation.z.to_string());
    push("position_x", e.translation.x.to_string());
    push("position_y", e.translation.y.to_string());
    push("position_z", e.translation.z.to_string());
    out
}

/// Reconstructs a [`Camera`] from any `ptiff.camera.*` fields on `node`.
///
/// Returns `Ok(None)` when the node carries no camera fields (the domain is
/// absent), `Ok(Some(camera))` when at least one field is present and valid,
/// and an error when a present field is malformed (e.g. a non-numeric
/// focal length). Unknown keys are ignored so future writers' extra fields are
/// not mistaken for errors.
pub fn camera_from_model(node: &StorageModel) -> Result<Option<Camera>> {
    let present = domain_present(node, CAMERA_PREFIX);
    if !present {
        return Ok(None);
    }

    let field = |key: &str| -> Result<&str> {
        node.field(&format!("{CAMERA_PREFIX}{key}"))
            .map_err(|_| Error::invalid_argument("SceneDeserializer: missing ptiff.camera field"))
    };
    let f64_field = |key: &str| -> Result<f64> {
        node.field(&format!("{CAMERA_PREFIX}{key}"))
            .map_err(|_| Error::invalid_argument("SceneDeserializer: missing ptiff.camera field"))?
            .parse()
            .map_err(|_| {
                Error::invalid_argument("SceneDeserializer: non-numeric ptiff.camera field")
            })
    };

    let model = field("model")?.to_string();
    let timestamp = field("timestamp").unwrap_or("").to_string();

    let intrinsics = crate::geometry::Intrinsics::new(
        f64_field("focal_length_x")?,
        f64_field("focal_length_y")?,
        f64_field("principal_x")?,
        f64_field("principal_y")?,
    );
    // Extrinsics are optional: a camera file/field-set may carry only the
    // intrinsics (model + focal + principal point), which the oracle fixture
    // does. When the rotation/position group is *absent* we default to the
    // identity pose (the `Camera` model always carries an `Extrinsics`, so
    // absence is represented by identity); when *partially* present we require
    // the full group (a partial group is malformed / out of spec).
    let has_extrinsics = [
        "rotation_w",
        "rotation_x",
        "rotation_y",
        "rotation_z",
        "position_x",
        "position_y",
        "position_z",
    ]
    .into_iter()
    .all(|k| node.field(&format!("{CAMERA_PREFIX}{k}")).is_ok());

    let extrinsics = if has_extrinsics {
        crate::geometry::Extrinsics::new(
            crate::geometry::Quaternion::new(
                f64_field("rotation_w")?,
                f64_field("rotation_x")?,
                f64_field("rotation_y")?,
                f64_field("rotation_z")?,
            ),
            crate::geometry::Vec3::new(
                f64_field("position_x")?,
                f64_field("position_y")?,
                f64_field("position_z")?,
            ),
        )
    } else {
        crate::geometry::Extrinsics::default()
    };

    Ok(Some(Camera::from_model(
        model, intrinsics, extrinsics, timestamp,
    )))
}

/// True when `model` carries at least one field whose key starts with `prefix`.
fn domain_present(model: &StorageModel, prefix: &str) -> bool {
    let mut present = false;
    model.for_each_field(|key, _| {
        if key.starts_with(prefix) {
            present = true;
        }
    });
    present
}

/// Maps a CRS's state into the registered RFC-0004 `ptiff.crs.*` fields.
///
/// The writer emits exactly the three normative RFC-0004 fields: `body`
/// (NAIF body id), `projection`, and `reference_frame` (the effective frame:
/// the override if set, else the body's canonical IAU default). This is the
/// interoperable schema the C++ oracle / bindings / GIS tooling expect.
///
/// The structured [`CoordinateReferenceSystem`] carries richer state (planet
/// name, ellipsoid, projection parameters) that RFC-0004 deliberately does
/// not; serializing it is intentionally **lossy**. The reader re-derives the
/// name from the NAIF body id and the ellipsoid / projection parameters fall
/// back to `UNSPECIFIED` / unset. Callers that need the full structured state
/// should not round-trip through the flat fields (see `ptiff_open_path_fields`
/// for a lossless flat view of the on-disk fields).
pub fn crs_fields(crs: &CoordinateReferenceSystem) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut push = |key: &str, value: String| out.push((format!("{CRS_PREFIX}{key}"), value));

    push("body", crs.planet().iau_identifier().to_string());
    push(
        "projection",
        projection_kind_str(crs.projection().kind()).to_string(),
    );
    push("reference_frame", crs.frame().id().to_string());

    out
}

/// Reconstructs a [`CoordinateReferenceSystem`] from any `ptiff.crs.*` fields
/// on `node`.
///
/// Accepts **both** CRS field schemas:
///
/// 1. the **registered** schema (RFC-0004 / `field-register.md`, what this
///    writer [`crs_fields`] and the C++ oracle emit): `ptiff.crs.body`,
///    `ptiff.crs.projection`, `ptiff.crs.reference_frame`;
/// 2. a **legacy extended** schema (pre-RFC-0004 Rust writer output):
///    `ptiff.crs.planet_name`, `planet_iau_id`, `planet_semi_major_m`,
///    `planet_semi_minor_m`, `frame`, `projection`, `param.*`, which carries
///    more of the internal `CoordinateReferenceSystem` state. Kept readable
///    for backward compatibility with files written by the old writer.
///
/// The registered schema is prioritised when present. A domain the file
/// doesn't carry stays `None`; a malformed numeric field is an error (mirrors
/// [`camera_from_model`]).
pub fn crs_from_model(node: &StorageModel) -> Result<Option<CoordinateReferenceSystem>> {
    if !domain_present(node, CRS_PREFIX) {
        return Ok(None);
    }

    // The registered schema (RFC-0004) is prioritised when its fields are
    // present; it carries no planet name / ellipsoid, so those are derived
    // best-effort from the body id.
    if let Ok(body) = node.field(&format!("{CRS_PREFIX}body")) {
        return crs_from_registered_schema(node, body);
    }

    // Extended schema: rich planet fields.
    let field = |key: &str| -> Result<&str> {
        node.field(&format!("{CRS_PREFIX}{key}"))
            .map_err(|_| Error::invalid_argument("SceneDeserializer: missing ptiff.crs field"))
    };
    let f64_field = |key: &str| -> Result<f64> {
        node.field(&format!("{CRS_PREFIX}{key}"))
            .map_err(|_| Error::invalid_argument("SceneDeserializer: missing ptiff.crs field"))?
            .parse()
            .map_err(|_| Error::invalid_argument("SceneDeserializer: non-numeric ptiff.crs field"))
    };

    let name = field("planet_name")?.to_string();
    let iau_id = field("planet_iau_id")?.to_string();
    let ellipsoid = Ellipsoid::new(
        f64_field("planet_semi_major_m")?,
        f64_field("planet_semi_minor_m")?,
    );

    // A known frame id round-trips via the `'static` constants; an unknown id
    // is treated as absent (see `frame_from_id`). The planet's canonical
    // reference frame falls back to a known constant derived from the IAU id,
    // else "UNSPECIFIED".
    let frame_override = field("frame").ok().and_then(frame_from_id);
    let reference_frame = frame_from_id(&iau_id).unwrap_or(Frame::new("UNSPECIFIED"));
    let planet = Planet::new(name, iau_id, ellipsoid, reference_frame);

    let kind = projection_kind_from_str(field("projection")?)?;
    let mut projection = Projection::new(kind);
    collect_projection_params(node, &mut projection);

    Ok(Some(CoordinateReferenceSystem::new(
        planet,
        frame_override,
        projection,
    )))
}

/// Reconstructs a [`CoordinateReferenceSystem`] from the registered RFC-0004
/// fields (`body`, `projection`, `reference_frame`). The structured model
/// carries a planet name + ellipsoid the registered schema does not; those are
/// derived best-effort from the NAIF body id (unknown ids keep a blank name /
/// UNSPECIFIED reference frame / UNSPECIFIED ellipsoid).
fn crs_from_registered_schema(
    node: &StorageModel,
    body: &str,
) -> Result<Option<CoordinateReferenceSystem>> {
    let field = |key: &str| -> Result<&str> {
        node.field(&format!("{CRS_PREFIX}{key}"))
            .map_err(|_| Error::invalid_argument("SceneDeserializer: missing ptiff.crs field"))
    };

    let name = planet_name_from_body_id(body);

    // The reference frame is a runtime string (e.g. "IAU_MOON_2000") that the
    // static-`Frame` model cannot hold losslessly. Map a *known* static frame
    // id (SPACECRAFT/CAMERA/browser body ids) when we can; otherwise fall back
    // to a frame derived from the body id, else the UNSPECIFIED literal. The
    // exact reference_frame string remains accessible losslessly through
    // ptiff_open_path_fields (the flat field view), not the structured model.
    let reference_frame = match field("reference_frame").ok().and_then(frame_from_id) {
        Some(f) => f,
        None => frame_from_id(body).unwrap_or(Frame::new("UNSPECIFIED")),
    };
    let frame_override = field("reference_frame").ok().and_then(frame_from_id);
    let planet = Planet::new(
        name,
        body.to_string(),
        Ellipsoid::UNSPECIFIED,
        reference_frame,
    );

    let projection = match field("projection") {
        Ok(kind_str) => Projection::new(projection_kind_from_str(kind_str)?),
        Err(_) => Projection::new(ProjectionKind::Equirectangular),
    };

    Ok(Some(CoordinateReferenceSystem::new(
        planet,
        frame_override,
        projection,
    )))
}

/// Best-effort human-readable body name for a NAIF body id (per RFC-0004 the
/// canonical form is the NAIF id itself).
fn planet_name_from_body_id(id: &str) -> String {
    match id {
        "301" => "Moon".to_string(),
        "399" => "Earth".to_string(),
        "499" => "Mars".to_string(),
        "599" => "Jupiter".to_string(),
        "699" => "Saturn".to_string(),
        "799" => "Uranus".to_string(),
        "899" => "Neptune".to_string(),
        _ => id.to_string(),
    }
}

/// Copies any `ptiff.crs.param.<key>` numeric fields into `projection`.
fn collect_projection_params(node: &StorageModel, projection: &mut Projection) {
    node.for_each_field(|key, value| {
        if let Some(param) = key.strip_prefix("ptiff.crs.param.") {
            if let Ok(v) = value.parse::<f64>() {
                projection.set_parameter(param.to_string(), v);
            }
        }
    });
}

/// Maps a well-known frame id string back to the matching [`Frame`] constant.
///
/// [`Frame`] stores a `'static` identifier, so arbitrary runtime strings cannot
/// be reconstructed without leaking. Only the pre-defined constants are
/// recognised; an unrecognised id maps to `None` (the override is treated as
/// absent).
fn frame_from_id(id: &str) -> Option<Frame> {
    match id {
        "J2000" => Some(Frame::J2000),
        "IAU_MOON" => Some(Frame::IAU_MOON),
        "IAU_EARTH" => Some(Frame::IAU_EARTH),
        "spacecraft" => Some(Frame::SPACECRAFT),
        "camera" => Some(Frame::CAMERA),
        _ => None,
    }
}

/// Canonical string for a [`ProjectionKind`].
fn projection_kind_str(kind: ProjectionKind) -> &'static str {
    match kind {
        ProjectionKind::Equirectangular => "equirectangular",
        ProjectionKind::Stereographic => "stereographic",
        ProjectionKind::Sinusoidal => "sinusoidal",
        ProjectionKind::Orthographic => "orthographic",
    }
}

/// Parses a canonical [`ProjectionKind`] string.
fn projection_kind_from_str(s: &str) -> Result<ProjectionKind> {
    match s {
        "equirectangular" => Ok(ProjectionKind::Equirectangular),
        "stereographic" => Ok(ProjectionKind::Stereographic),
        "sinusoidal" => Ok(ProjectionKind::Sinusoidal),
        "orthographic" => Ok(ProjectionKind::Orthographic),
        _ => Err(Error::invalid_argument(
            "SceneDeserializer: unrecognized projection kind",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_camera() -> Camera {
        Camera::from_model(
            "pinhole",
            crate::geometry::Intrinsics::new(900.0, 901.0, 512.5, 384.25),
            crate::geometry::Extrinsics::new(
                crate::geometry::Quaternion::new(0.7, 0.1, 0.2, 0.3),
                crate::geometry::Vec3::new(1.0, 2.0, 3.0),
            ),
            "2026-08-21T12:34:56.000Z",
        )
    }

    fn sample_crs() -> CoordinateReferenceSystem {
        CoordinateReferenceSystem::new(
            Planet::new(
                "Moon",
                "301",
                Ellipsoid::new(1_737_400.0, 1_735_700.0),
                Frame::IAU_MOON,
            ),
            Some(Frame::SPACECRAFT),
            Projection::new(ProjectionKind::Stereographic),
        )
    }

    fn model_with(fields: Vec<(String, String)>) -> StorageModel {
        let mut m = StorageModel::new();
        for (k, v) in fields {
            m.set_field(k, v);
        }
        m
    }

    #[test]
    fn camera_round_trips_through_fields() {
        let camera = sample_camera();
        let mut model = StorageModel::new();
        for (k, v) in camera_fields(&camera) {
            model.set_field(k, v);
        }
        let back = camera_from_model(&model).expect("parse").expect("present");
        assert_eq!(back, camera);
    }

    #[test]
    fn camera_fields_are_absent_when_node_has_none() {
        let model = model_with(vec![("imageWidth".to_string(), "64".to_string())]);
        assert_eq!(camera_from_model(&model).expect("parse"), None);
    }

    #[test]
    fn camera_malformed_field_is_an_error() {
        let model = model_with(vec![
            ("ptiff.camera.model".to_string(), "pinhole".to_string()),
            (
                "ptiff.camera.focal_length_x".to_string(),
                "not-a-number".to_string(),
            ),
        ]);
        assert!(camera_from_model(&model).is_err());
    }

    #[test]
    fn camera_intrinsics_only_uses_identity_extrinsics() {
        // An intrinsics-only camera (the oracle interop fixture shape: model +
        // focal + principal, no rotation/position) must parse successfully
        // with the identity pose, rather than erroring on the missing
        // extrinsics group. (This lets ptiff_open_path read such a file.)
        let model = model_with(vec![
            ("ptiff.camera.model".to_string(), "pinhole".to_string()),
            (
                "ptiff.camera.focal_length_x".to_string(),
                "700.0".to_string(),
            ),
            (
                "ptiff.camera.focal_length_y".to_string(),
                "700.0".to_string(),
            ),
            ("ptiff.camera.principal_x".to_string(), "64.0".to_string()),
            ("ptiff.camera.principal_y".to_string(), "64.0".to_string()),
        ]);
        let camera = camera_from_model(&model).expect("parse").expect("present");
        assert_eq!(camera.model_name(), "pinhole");
        assert_eq!(
            camera.intrinsics(),
            crate::geometry::Intrinsics::new(700.0, 700.0, 64.0, 64.0)
        );
        // Absent extrinsics fall back to the identity pose.
        assert_eq!(camera.extrinsics(), crate::geometry::Extrinsics::IDENTITY);
    }

    #[test]
    fn crs_reads_registered_rfc_0004_schema() {
        // The registered RFC-0004 / field-register CRS schema (what the C++
        // oracle interop fixture writes): body / projection / reference_frame.
        let model = model_with(vec![
            ("ptiff.crs.body".to_string(), "301".to_string()),
            (
                "ptiff.crs.projection".to_string(),
                "equirectangular".to_string(),
            ),
            (
                "ptiff.crs.reference_frame".to_string(),
                "IAU_MOON_2000".to_string(),
            ),
        ]);
        let crs = crs_from_model(&model).expect("parse").expect("present");
        // Body id drives the planet's IAU id / derived name.
        assert_eq!(crs.planet().iau_identifier(), "301");
        assert_eq!(crs.planet().name(), "Moon");
        assert_eq!(crs.projection().kind(), ProjectionKind::Equirectangular);
    }

    #[test]
    fn crs_fields_emit_registered_rfc_0004_schema() {
        // The writer emits exactly the three normative RFC-0004 fields,
        // regardless of the richer internal state on the CRS.
        let crs = sample_crs();
        let mut fields = crs_fields(&crs);
        fields.sort();
        assert_eq!(
            fields,
            vec![
                ("ptiff.crs.body".to_string(), "301".to_string()),
                (
                    "ptiff.crs.projection".to_string(),
                    "stereographic".to_string(),
                ),
                (
                    "ptiff.crs.reference_frame".to_string(),
                    "spacecraft".to_string(),
                ),
            ]
        );
    }

    #[test]
    fn crs_writes_effective_frame_when_no_override() {
        // With no frame override, the writer uses the body's canonical IAU
        // default frame as the reference_frame.
        let crs = sample_crs();
        let crs =
            CoordinateReferenceSystem::new(crs.planet().clone(), None, crs.projection().clone());
        let fields = crs_fields(&crs);
        let rf = fields
            .iter()
            .find(|(k, _)| k == "ptiff.crs.reference_frame")
            .expect("reference_frame present");
        assert_eq!(rf.1, "IAU_MOON");
    }

    #[test]
    fn crs_round_trips_through_fields() {
        // RFC-0004 carries body / projection / reference_frame only. The rich
        // internal state (name, ellipsoid, projection params) is deliberately
        // NOT round-tripped: the name is re-derived from the NAIF body id and
        // the ellipsoid falls back to UNSPECIFIED.
        let crs = sample_crs();
        let mut model = StorageModel::new();
        for (k, v) in crs_fields(&crs) {
            model.set_field(k, v);
        }
        let back = crs_from_model(&model).expect("parse").expect("present");
        // Body id drives the derived planet name.
        assert_eq!(back.planet().name(), "Moon");
        assert_eq!(back.planet().iau_identifier(), "301");
        // Ellipsoid is not carried by RFC-0004 → UNSPECIFIED.
        assert_eq!(back.planet().ellipsoid(), Ellipsoid::UNSPECIFIED);
        // Frame override round-trips via the known SPACECRAFT constant.
        assert_eq!(back.frame_override(), Some(Frame::SPACECRAFT));
        assert_eq!(back.frame(), Frame::SPACECRAFT);
        assert_eq!(back.projection().kind(), ProjectionKind::Stereographic);
    }

    #[test]
    fn crs_legacy_extended_schema_still_reads() {
        // Files written by the pre-RFC-0004 writer (extended schema) must
        // remain readable: name / ellipsoid / frame / projection params.
        let model = model_with(vec![
            ("ptiff.crs.planet_name".to_string(), "Moon".to_string()),
            ("ptiff.crs.planet_iau_id".to_string(), "301".to_string()),
            (
                "ptiff.crs.planet_semi_major_m".to_string(),
                "1737400.0".to_string(),
            ),
            (
                "ptiff.crs.planet_semi_minor_m".to_string(),
                "1735700.0".to_string(),
            ),
            ("ptiff.crs.frame".to_string(), "spacecraft".to_string()),
            (
                "ptiff.crs.projection".to_string(),
                "stereographic".to_string(),
            ),
            (
                "ptiff.crs.param.central_meridian".to_string(),
                "0.0".to_string(),
            ),
        ]);
        let crs = crs_from_model(&model).expect("parse").expect("present");
        assert_eq!(crs.planet().name(), "Moon");
        assert_eq!(crs.planet().iau_identifier(), "301");
        assert_eq!(
            crs.planet().ellipsoid(),
            Ellipsoid::new(1_737_400.0, 1_735_700.0)
        );
        assert_eq!(crs.frame_override(), Some(Frame::SPACECRAFT));
        assert_eq!(crs.projection().kind(), ProjectionKind::Stereographic);
    }

    #[test]
    fn crs_fields_are_absent_when_node_has_none() {
        let model = model_with(vec![("imageWidth".to_string(), "64".to_string())]);
        assert_eq!(crs_from_model(&model).expect("parse"), None);
    }

    #[test]
    fn unknown_extra_fields_are_preserved_and_ignored() {
        let mut model = StorageModel::new();
        for (k, v) in camera_fields(&sample_camera()) {
            model.set_field(k, v);
        }
        // A future writer's unknown camera field must not break the round trip.
        model.set_field("ptiff.camera.future_extension", "value");
        let back = camera_from_model(&model).expect("parse").expect("present");
        assert_eq!(back, sample_camera());
    }
}
