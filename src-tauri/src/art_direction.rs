//! Photo-led design by default; only explicit optional choices constrain the model.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct FrameDesign {
    pub style: String,
    pub placement: String,
    pub palette: String,
    pub colors: Vec<String>,
    pub texture: bool,
    pub notes: String,
    /// What the image model paints. Anything but "none" asks for one opaque
    /// surface that Margin cuts the photograph opening into locally.
    pub material: String,
}
impl Default for FrameDesign {
    fn default() -> Self {
        Self {
            style: "auto".into(),
            placement: "auto".into(),
            palette: "photo".into(),
            colors: vec!["#315D69".into(), "#C27B5E".into(), "#D6C3A0".into()],
            texture: false,
            notes: String::new(),
            material: "auto".into(),
        }
    }
}
pub fn validate(d: &FrameDesign) -> Result<(), String> {
    if !matches!(
        d.style.as_str(),
        "auto"
            | "graphic"
            | "editorial"
            | "film"
            | "linework"
            | "collage"
            | "illustration"
            | "custom"
    ) || !matches!(
        d.placement.as_str(),
        "auto" | "corners" | "sides" | "bottom"
    ) || !matches!(d.palette.as_str(), "photo" | "mono" | "custom")
        || !MATERIALS.iter().any(|(id, _)| *id == d.material)
        || d.colors.len() != 3
        || d.notes.chars().count() > 1500
    {
        return Err("边框风格选项无效".into());
    }
    for c in &d.colors {
        crate::imaging::parse_color(c)?;
    }
    Ok(())
}
/// Surfaces the image model may paint, each described as a physical material
/// rather than a picture, so the result reads as a real mount around the print.
pub const MATERIALS: &[(&str, &str)] = &[
    ("auto", "Choose ONE physical surface that suits this photograph, such as handmade cotton paper, washi with long fibres, fine linen, a soft watercolour wash, suminagashi marbling, aged plaster, raw silk or matte gilded leaf. Pick what a gallery framer would choose for this print."),
    ("paper", "Heavy handmade cotton rag paper: soft deckle-like fibre clumps, gentle tooth, tiny natural inclusions, warm off-white or a pale tint taken from the photo."),
    ("washi", "Japanese washi paper: translucent layered kozo fibres, long visible strands, faint mottling in a pale tint from the photo."),
    ("linen", "Fine bookbinding linen: tight even weave, subtle slub threads, matte, dyed in a muted colour taken from the photo."),
    ("wash", "A loose watercolour wash on cold-pressed paper: two or three photo colours bleeding softly into each other, granulation and soft blooms, no recognisable objects."),
    ("marble", "Suminagashi or Turkish marbled paper: slow concentric ink currents in two or three restrained photo colours on an ivory ground, elegant and calm, not psychedelic."),
    ("plaster", "Hand-troweled lime plaster: soft cloudy tonal shifts, very fine trowel marks, chalky matte, in a quiet colour from the photo."),
    ("gilded", "Matte gilded leaf on a dark bole ground: slightly irregular square leaf seams, soft worn edges, restrained warm metal, never shiny chrome."),
    ("none", ""),
];
pub fn material(d: &FrameDesign) -> Option<&'static str> {
    MATERIALS
        .iter()
        .find(|(id, _)| *id == d.material && *id != "none")
        .map(|(_, text)| *text)
}
pub fn instructions(d: &FrameDesign) -> Result<String, String> {
    validate(d)?;
    if d.style == "custom" && d.notes.trim().is_empty() {
        return Err("请描述自定义边框风格".into());
    }
    let style=match d.style.as_str(){
 "auto"=>"PHOTO-LED ORIGINAL DESIGN. Independently choose a visual vocabulary and composition from this photograph's forms, color relationships, subject and mood. The user's brief, when present, is the primary creative direction. When it is empty, make your own considered design decision. Do not impose a preset style, default to film borders, or assume watercolor, vintage illustration or hand drawing. Develop a specific, coherent idea with purposeful detail and clear visual hierarchy; a generic stock frame is not the goal.",
 "graphic"=>"CLEAN GRAPHIC DESIGN. Crisp flat-color geometric forms and controlled interwoven graphic bands, strong spacing and grid alignment, 2-3 colors. Translate the photo into abstract rhythm and color, not literal scenery. No painterly rendering, watercolor, airbrushed gradients, impasto, ornate engraving, realistic beach scenes or faux vintage illustration.",
 "editorial"=>"PHOTOGRAPHIC EDITORIAL / GALLERY. Refined mat board, precise rules, restrained color panels and a deliberate editorial grid. The photograph is the main image. No invented lettering or captions, no painted scenery, no illustrative collage and no ornate floral frame.",
 "film"=>"ANALOG PHOTO PAPER / CONTACT SHEET. Quiet charcoal or ivory photographic borders, fine registration marks and understated film-edge geometry. Real photographic presentation, not a painting of a camera or a beach. No fake lettering, noisy scratches or distressed effects.",
 "linework"=>"PRECISE INK LINEWORK. Fine monochrome or two-color contour lines with a few recognisable, elegantly simplified motifs. Consistent stroke weights, calm spacing, designed negative space. No watercolor fill, painterly shadows, dense etching or realistic scenery.",
 "collage"=>"FLAT CUT-PAPER COLLAGE. A small number of clean paper shapes, tabs and strips, subtle layering and thoughtful placement. A contemporary graphic scrapbook, not a busy painted travel poster. No copied photo fragments, no fake lettering, no painterly texture.",
 "illustration"=>"LIGHT ILLUSTRATION. Deliberately simplified hand-drawn motifs, clean silhouettes, limited flat colors and readable shapes. A coherent illustration composition is allowed because the user explicitly selected it. Avoid highly rendered AI-painting effects or excessive tiny decorative objects.",
 _=>"CUSTOM DIRECTION. Follow the user's own visual direction closely. Avoid imposing a hand-painted or vintage-illustration treatment when it was not requested.",};
    let placement=match d.placement.as_str(){"auto"=>"Choose the placement, rhythm and balance to suit the actual photo opening and border widths. No fixed corner, side or bottom composition is prescribed. Keep the photograph visually dominant.","sides"=>"Compose across all four sides with a clear hierarchy and breathing room.","bottom"=>"Put the main design in the lower border, with only a very small echo above. Keep the side margins largely clear.",_=>"Concentrate the design in two diagonal corners, leaving most of the other border quiet. Do not fill the whole border with scenery."};
    let palette=match d.palette.as_str(){"mono"=>"Use black/charcoal and ivory only; do not introduce colored accents.".into(),"custom"=>format!("Use only these user-picked accent colors: {}. A neutral white/ivory ground is allowed.",d.colors.join(", ")),_=>"Choose 2-3 restrained colors from the reference photograph, including at most one accent. Do not increase saturation indiscriminately.".into()};
    let heading = if d.style == "auto" {
        "CREATIVE APPROACH"
    } else {
        "USER-SELECTED VISUAL SYSTEM (honor this explicit style choice)"
    };
    Ok(format!(
        "{heading}: {style}\nPLACEMENT: {placement}\nPALETTE: {palette}\nSURFACE: {}",
        if d.texture {
            "Very subtle paper grain only. No heavy distress or painted shading."
        } else {
            "Clean surface and legible forms. Avoid gratuitous texture, noise, artificial aging and glossy AI-painting effects. If the user explicitly requests a material or drawing technique, honor the brief while keeping the border refined."
        }
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn directions_enforce_user_choices_and_reject_unknown_options() {
        let mut d = FrameDesign::default();
        let p = instructions(&d).unwrap();
        assert!(p.contains("PHOTO-LED ORIGINAL DESIGN") && p.contains("No fixed corner"));
        assert!(!p.contains("hard constraint") && !p.contains("CLEAN GRAPHIC"));
        d.style = "graphic".into();
        d.placement = "corners".into();
        assert!(instructions(&d).unwrap().contains("CLEAN GRAPHIC"));
        d.palette = "custom".into();
        d.colors[0] = "#123456".into();
        assert!(instructions(&d).unwrap().contains("#123456"));
        d.style = "illustration".into();
        assert!(instructions(&d).unwrap().contains("explicitly selected"));
        d.style = "not-a-style".into();
        assert!(instructions(&d).is_err());
        d.style = "auto".into();
        assert!(material(&d).unwrap().contains("ONE physical surface"));
        d.material = "none".into();
        assert!(material(&d).is_none() && instructions(&d).is_ok());
        d.material = "velvet-glitter".into();
        assert!(instructions(&d).is_err());
    }
}
