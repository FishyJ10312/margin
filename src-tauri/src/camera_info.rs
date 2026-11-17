//! Camera settings read from a photo's EXIF block, and the `{token}` template
//! that turns them into a per-photo "camera info" watermark line.

use serde::Serialize;
use std::io::Cursor;

/// Shooting details, already formatted for display. Missing values are `None`.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraInfo {
    /// Display name of the body, see [`CameraInfo::body`].
    pub camera: Option<String>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub lens: Option<String>,
    /// "35mm"; the 35 mm equivalent for phone-sized sensors.
    pub focal: Option<String>,
    /// "52mm" (35 mm equivalent)
    pub focal35: Option<String>,
    /// "f/1.8"
    pub aperture: Option<String>,
    /// "1/250s" or "2s"
    pub shutter: Option<String>,
    /// "ISO 100"
    pub iso: Option<String>,
    /// "2026.11.17"
    pub date: Option<String>,
}

impl CameraInfo {
    pub fn is_empty(&self) -> bool {
        *self == CameraInfo::default()
    }

    /// Body name without a repeated brand: "Apple" + "iPhone 15 Pro" → "iPhone 15 Pro",
    /// "NIKON CORPORATION" + "NIKON Z 6" → "NIKON Z 6", "SONY" + "ILCE-7M4" → "SONY ILCE-7M4".
    pub fn body(&self) -> Option<String> {
        let model = self.model.as_deref().map(str::trim).filter(|m| !m.is_empty());
        let brand = self
            .make
            .as_deref()
            .and_then(|m| m.split_whitespace().next())
            .filter(|b| !b.is_empty());
        match (brand, model) {
            (Some(b), Some(m)) if m.to_lowercase().starts_with(&b.to_lowercase()) => Some(m.into()),
            (Some("Apple"), Some(m)) => Some(m.into()),
            (Some(b), Some(m)) => Some(format!("{b} {m}")),
            (None, Some(m)) => Some(m.into()),
            (Some(b), None) => Some(b.into()),
            (None, None) => None,
        }
    }
}

fn text(exif: &exif::Exif, tag: exif::Tag) -> Option<String> {
    let field = exif.get_field(tag, exif::In::PRIMARY)?;
    let value = match &field.value {
        exif::Value::Ascii(parts) => parts
            .iter()
            .map(|p| String::from_utf8_lossy(p).trim_matches(char::from(0)).trim().to_string())
            .find(|s| !s.is_empty())?,
        _ => return None,
    };
    // Guard the watermark against absurd vendor strings.
    Some(value.chars().take(60).collect())
}

fn rational(exif: &exif::Exif, tag: exif::Tag) -> Option<(u32, u32)> {
    match &exif.get_field(tag, exif::In::PRIMARY)?.value {
        exif::Value::Rational(v) => v.first().filter(|r| r.denom != 0).map(|r| (r.num, r.denom)),
        _ => None,
    }
}

fn uint(exif: &exif::Exif, tag: exif::Tag) -> Option<u32> {
    exif.get_field(tag, exif::In::PRIMARY)?.value.get_uint(0)
}

fn trim_number(value: f64, digits: usize) -> String {
    let s = format!("{value:.digits$}");
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}

pub fn format_shutter(num: u32, denom: u32) -> Option<String> {
    if num == 0 || denom == 0 {
        return None;
    }
    let seconds = f64::from(num) / f64::from(denom);
    if seconds >= 0.3 {
        return Some(format!("{}s", trim_number(seconds, 1)));
    }
    Some(format!("1/{}s", (1.0 / seconds).round() as u64))
}

/// Phones and compacts record a tiny real focal length (6.8mm); photographers
/// quote their 35mm equivalent (24mm) instead. Larger sensors keep the real value.
fn focal_label(real: Option<f64>, equivalent: Option<&str>) -> Option<String> {
    match (real, equivalent) {
        (Some(r), Some(eq)) if r < 10.0 => Some(eq.to_string()),
        (Some(r), _) if r > 0.0 => Some(format!("{}mm", trim_number(r, 1))),
        (_, eq) => eq.map(str::to_string),
    }
}

/// Read camera details from JPEG, HEIF/HEIC or PNG bytes. Never fails: photos
/// without EXIF simply produce an empty `CameraInfo`.
pub fn read(bytes: &[u8]) -> CameraInfo {
    let Ok(exif) = exif::Reader::new().read_from_container(&mut Cursor::new(bytes)) else {
        return CameraInfo::default();
    };
    use exif::Tag;
    let real_focal = rational(&exif, Tag::FocalLength).map(|(n, d)| f64::from(n) / f64::from(d));
    let focal35 = uint(&exif, Tag::FocalLengthIn35mmFilm)
        .filter(|v| *v > 0)
        .map(|v| format!("{v}mm"));
    let focal = focal_label(real_focal, focal35.as_deref());
    let aperture = rational(&exif, Tag::FNumber)
        .filter(|(n, _)| *n > 0)
        .map(|(n, d)| format!("f/{}", trim_number(f64::from(n) / f64::from(d), 1)));
    let shutter = rational(&exif, Tag::ExposureTime).and_then(|(n, d)| format_shutter(n, d));
    let iso = uint(&exif, Tag::PhotographicSensitivity)
        .filter(|v| *v > 0)
        .map(|v| format!("ISO {v}"));
    let date = text(&exif, Tag::DateTimeOriginal)
        .or_else(|| text(&exif, Tag::DateTime))
        .and_then(|d| {
            // EXIF dates are "YYYY:MM:DD HH:MM:SS".
            let day = d.get(0..10)?;
            let parts: Vec<_> = day.split(':').collect();
            (parts.len() == 3 && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit())))
                .then(|| parts.join("."))
        });
    let mut info = CameraInfo {
        camera: None,
        make: text(&exif, Tag::Make),
        model: text(&exif, Tag::Model),
        lens: text(&exif, Tag::LensModel),
        focal,
        focal35,
        aperture,
        shutter,
        iso,
        date,
    };
    info.camera = info.body();
    info
}

const SEPARATORS: [char; 2] = ['·', '|'];

/// Fill `{camera}`, `{make}`, `{model}`, `{lens}`, `{focal}`, `{focal35}`,
/// `{aperture}`, `{shutter}`, `{iso}` and `{date}`. Separators (`·` or `|`) left
/// next to a missing value are dropped, so a phone without a lens name still
/// reads cleanly. Unknown tokens are kept as typed.
pub fn resolve(template: &str, info: &CameraInfo) -> String {
    let value = |name: &str| -> Option<Option<String>> {
        Some(match name {
            "camera" => info.camera.clone().or_else(|| info.body()),
            "make" => info.make.clone(),
            "model" => info.model.clone(),
            "lens" => info.lens.clone(),
            "focal" => info.focal.clone(),
            "focal35" => info.focal35.clone(),
            "aperture" => info.aperture.clone(),
            "shutter" => info.shutter.clone(),
            "iso" => info.iso.clone(),
            "date" => info.date.clone(),
            _ => return None,
        })
    };
    let mut filled = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        filled.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('}') {
            Some(end) => match value(&after[..end]) {
                Some(v) => {
                    filled.push_str(v.as_deref().unwrap_or(""));
                    rest = &after[end + 1..];
                }
                None => {
                    filled.push('{');
                    rest = after;
                }
            },
            None => {
                filled.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    filled.push_str(rest);
    filled
        .lines()
        .map(|line| {
            let Some(sep) = line.chars().find(|c| SEPARATORS.contains(c)) else {
                return line.split_whitespace().collect::<Vec<_>>().join(" ");
            };
            line.split(SEPARATORS)
                .map(|piece| piece.split_whitespace().collect::<Vec<_>>().join(" "))
                .filter(|piece| !piece.is_empty())
                .collect::<Vec<_>>()
                .join(&format!(" {sep} "))
        })
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sony() -> CameraInfo {
        CameraInfo {
            camera: None,
            make: Some("SONY".into()),
            model: Some("ILCE-7M4".into()),
            lens: Some("FE 35mm F1.4 GM".into()),
            focal: Some("35mm".into()),
            focal35: Some("35mm".into()),
            aperture: Some("f/1.8".into()),
            shutter: Some("1/250s".into()),
            iso: Some("ISO 100".into()),
            date: Some("2026.11.17".into()),
        }
    }

    #[test]
    fn camera_name_drops_repeated_brand() {
        let mut i = sony();
        assert_eq!(i.body().as_deref(), Some("SONY ILCE-7M4"));
        i.make = Some("NIKON CORPORATION".into());
        i.model = Some("NIKON Z 6".into());
        assert_eq!(i.body().as_deref(), Some("NIKON Z 6"));
        i.make = Some("Apple".into());
        i.model = Some("iPhone 15 Pro".into());
        assert_eq!(i.body().as_deref(), Some("iPhone 15 Pro"));
    }

    #[test]
    fn template_fills_tokens_and_lines() {
        let out = resolve("{camera}\n{focal} {aperture} {shutter} {iso}", &sony());
        assert_eq!(out, "SONY ILCE-7M4\n35mm f/1.8 1/250s ISO 100");
        assert_eq!(resolve("Shot on {camera}", &sony()), "Shot on SONY ILCE-7M4");
    }

    #[test]
    fn missing_values_drop_their_separators() {
        let mut i = sony();
        i.lens = None;
        i.iso = None;
        assert_eq!(
            resolve("{camera} · {lens} · {focal} · {iso}", &i),
            "SONY ILCE-7M4 · 35mm"
        );
        assert_eq!(resolve("{lens}\n{date}", &i), "2026.11.17");
        assert_eq!(resolve("{camera} · {iso}", &CameraInfo::default()), "");
    }

    #[test]
    fn unknown_tokens_and_braces_are_kept() {
        assert_eq!(resolve("{nope} {iso} {", &sony()), "{nope} ISO 100 {");
    }

    #[test]
    fn shutter_reads_as_photographers_write_it() {
        assert_eq!(format_shutter(1, 250).as_deref(), Some("1/250s"));
        assert_eq!(format_shutter(10, 3200).as_deref(), Some("1/320s"));
        assert_eq!(format_shutter(1, 2).as_deref(), Some("0.5s"));
        assert_eq!(format_shutter(2, 1).as_deref(), Some("2s"));
        assert_eq!(format_shutter(0, 1), None);
    }

    #[test]
    fn phones_quote_the_equivalent_focal_length() {
        assert_eq!(focal_label(Some(6.765), Some("24mm")).as_deref(), Some("24mm"));
        assert_eq!(focal_label(Some(23.0), Some("35mm")).as_deref(), Some("23mm"));
        assert_eq!(focal_label(Some(6.765), None).as_deref(), Some("6.8mm"));
        assert_eq!(focal_label(None, Some("50mm")).as_deref(), Some("50mm"));
        assert_eq!(focal_label(None, None), None);
    }

    #[test]
    fn photos_without_exif_read_as_empty() {
        assert!(read(b"not an image").is_empty());
    }
}
