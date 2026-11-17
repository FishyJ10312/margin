//! Real Apple-encoded synthetic HEIC matrix; no user photos, accounts or requests.
use libheif_rs::{HeifContext, LibHeif};
use liubai_lib::{imaging, model::RenderSettings};
use std::{fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if liubai_lib::source_io::worker_entry_if_requested() { return Ok(()); }
    let a: Vec<_> = std::env::args().collect();
    let input = PathBuf::from(a.get(1).ok_or("fixture directory")?);
    let output = PathBuf::from(a.get(2).ok_or("new output directory")?);
    if output.exists() {
        return Err("Preserve previous evidence; choose a new output directory".into());
    }
    fs::create_dir_all(&output)?;
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(input.join("manifest.json"))?)?;
    let _lib = LibHeif::new();
    let mut settings = RenderSettings::default();
    settings.border_percent = 0.0;
    settings.export.format = "png".into();
    let mut rows = Vec::new();
    let mut failures = Vec::new();
    let mut base: Option<image::DynamicImage> = None;
    for item in manifest["cases"].as_array().ok_or("cases")? {
        let name = item["file"].as_str().ok_or("filename")?;
        if item["generated"] != true {
            failures.push(format!("{name}: encoder failed"));
            continue;
        }
        let source = input.join(name);
        let before = fs::read(&source)?;
        let ctx = HeifContext::read_from_bytes(&before)?;
        let h = ctx.primary_image_handle()?;
        let nclx = h.color_profile_nclx();
        let target = output.join(format!("{name}.png"));
        let result = imaging::export_image(&source, &settings, &target);
        let mut row = serde_json::json!({"file":name,"bitDepth":h.luma_bits_per_pixel(),"hasAlpha":h.has_alpha_channel(),"iccBytes":h.color_profile_raw().map(|p|p.data.len()),"nclx":nclx.map(|p|format!("{:?}/{:?}",p.color_primaries(),p.transfer_characteristics())),"sourceUnchanged":fs::read(&source)?==before});
        if matches!(
            item["expect"].as_str(),
            Some("reject-hdr" | "reject-ambiguous")
        ) {
            row["expected"] = item["expect"].clone();
            let error = result.err().unwrap_or_default();
            row["error"] = error.clone().into();
            let marker = if item["expect"] == "reject-hdr" {
                "HDR"
            } else {
                "颜色信息"
            };
            if !error.contains(marker)
                || target.exists()
                || imaging::preview(&source, &settings, 320).is_ok()
            {
                failures.push(format!(
                    "{name}: unsupported color rejection failed ({error})"
                ));
            }
        } else {
            row["expected"] = "accept".into();
            match result {
                Ok((w, h)) => {
                    let expected: Vec<u32> =
                        serde_json::from_value(item["expectedDimensions"].clone())?;
                    row["dimensions"] = serde_json::json!([w, h]);
                    if [w, h] != expected.as_slice() {
                        failures.push(format!("{name}: dimensions {w}x{h} != {expected:?}"));
                    }
                    let rendered = image::open(&target)?;
                    if name == "apple-orientation-1.heic" {
                        base = Some(rendered.clone());
                    }
                    let orientation = item["orientation"].as_u64().unwrap_or(1);
                    if name.starts_with("apple-orientation-") {
                        let mut reference = base.as_ref().ok_or("baseline first")?.clone();
                        reference.apply_orientation(
                            image::metadata::Orientation::from_exif(orientation as u8)
                                .ok_or("orientation")?,
                        );
                        let actual = rendered.to_rgb8();
                        let expected = reference.to_rgb8();
                        if actual.dimensions() == expected.dimensions() {
                            let mad = actual
                                .as_raw()
                                .iter()
                                .zip(expected.as_raw())
                                .map(|(a, b)| f64::from(a.abs_diff(*b)))
                                .sum::<f64>()
                                / actual.as_raw().len() as f64;
                            row["orientationMeanAbsoluteError"] = mad.into();
                            if mad > 2.0 {
                                failures.push(format!("{name}: orientation pixel mismatch {mad}"));
                            }
                        }
                    }
                    let bytes = fs::read(&target)?;
                    let reader = png::Decoder::new(std::io::Cursor::new(bytes)).read_info()?;
                    if reader.info().icc_profile.is_none() {
                        failures.push(format!("{name}: output lacks sRGB ICC"));
                    }
                }
                Err(error) => {
                    row["error"] = error.clone().into();
                    failures.push(format!("{name}: {error}"));
                }
            }
        }
        if row["sourceUnchanged"] != true {
            failures.push(format!("{name}: source changed"));
        }
        rows.push(row);
        imaging::clear_image_cache();
    }
    let report = serde_json::json!({"cases":rows,"failures":failures,"inputs":"Apple ImageIO generated synthetic quadrants, not iPhone-captured photos"});
    fs::write(
        output.join("report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{report}");
    if !failures.is_empty() {
        return Err("Format matrix failed; see report.json".into());
    }
    Ok(())
}
