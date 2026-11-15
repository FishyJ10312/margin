//! User-selected art direction is a constraint, not an invitation to always paint.
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
}
impl Default for FrameDesign {
    fn default() -> Self {
        Self {
            style: "graphic".into(),
            placement: "corners".into(),
            palette: "photo".into(),
            colors: vec!["#315D69".into(), "#C27B5E".into(), "#D6C3A0".into()],
            texture: false,
            notes: String::new(),
        }
    }
}
pub fn validate(d: &FrameDesign) -> Result<(), String> {
    if !matches!(
        d.style.as_str(),
        "graphic" | "editorial" | "film" | "linework" | "collage" | "illustration" | "custom"
    ) || !matches!(d.placement.as_str(), "corners" | "sides" | "bottom")
        || !matches!(d.palette.as_str(), "photo" | "mono" | "custom")
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
pub fn instructions(d: &FrameDesign) -> Result<String, String> {
    validate(d)?;
    if d.style == "custom" && d.notes.trim().is_empty() {
        return Err("请描述自定义边框风格".into());
    }
    let style=match d.style.as_str(){
 "graphic"=>"CLEAN GRAPHIC DESIGN. Crisp flat-color geometric forms and controlled interwoven graphic bands, strong spacing and grid alignment, 2-3 colors. Translate the photo into abstract rhythm and color, not literal scenery. No painterly rendering, watercolor, airbrushed gradients, impasto, ornate engraving, realistic beach scenes or faux vintage illustration.",
 "editorial"=>"PHOTOGRAPHIC EDITORIAL / GALLERY. Refined mat board, precise rules, restrained color panels and a deliberate editorial grid. The photograph is the main image. No invented lettering or captions, no painted scenery, no illustrative collage and no ornate floral frame.",
 "film"=>"ANALOG PHOTO PAPER / CONTACT SHEET. Quiet charcoal or ivory photographic borders, fine registration marks and understated film-edge geometry. Real photographic presentation, not a painting of a camera or a beach. No fake lettering, noisy scratches or distressed effects.",
 "linework"=>"PRECISE INK LINEWORK. Fine monochrome or two-color contour lines with a few recognisable, elegantly simplified motifs. Consistent stroke weights, calm spacing, designed negative space. No watercolor fill, painterly shadows, dense etching or realistic scenery.",
 "collage"=>"FLAT CUT-PAPER COLLAGE. A small number of clean paper shapes, tabs and strips, subtle layering and thoughtful placement. A contemporary graphic scrapbook, not a busy painted travel poster. No copied photo fragments, no fake lettering, no painterly texture.",
 "illustration"=>"LIGHT ILLUSTRATION. Deliberately simplified hand-drawn motifs, clean silhouettes, limited flat colors and readable shapes. A coherent illustration composition is allowed because the user explicitly selected it. Avoid highly rendered AI-painting effects or excessive tiny decorative objects.",
 _=>"CUSTOM DIRECTION. Follow the user's own visual direction closely. Avoid imposing a hand-painted or vintage-illustration treatment when it was not requested.",};
    let placement=match d.placement.as_str(){"sides"=>"Compose across all four sides with a clear hierarchy and breathing room.","bottom"=>"Put the main design in the lower border, with only a very small echo above. Keep the side margins largely clear.",_=>"Concentrate the design in two diagonal corners, leaving most of the other border quiet. Do not fill the whole border with scenery."};
    let palette=match d.palette.as_str(){"mono"=>"Use black/charcoal and ivory only; do not introduce colored accents.".into(),"custom"=>format!("Use only these user-picked accent colors: {}. A neutral white/ivory ground is allowed.",d.colors.join(", ")),_=>"Choose 2-3 restrained colors from the reference photograph, including at most one accent. Do not increase saturation indiscriminately.".into()};
    Ok(format!("USER-SELECTED VISUAL SYSTEM (hard constraint; extra notes may refine motifs but must not silently replace this selected style): {style}\nPLACEMENT: {placement}\nPALETTE: {palette}\nSURFACE: {}",if d.texture{"Very subtle paper grain only. No heavy distress or painted shading."}else{"Clean flat surface. NO paper grain, canvas texture, watercolor bleed, distressed print, noise or simulated brush texture."}))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn directions_enforce_user_choices_and_reject_unknown_options() {
        let mut d = FrameDesign::default();
        let p = instructions(&d).unwrap();
        assert!(
            p.contains("CLEAN GRAPHIC")
                && p.contains("NO paper grain")
                && p.contains("diagonal corners")
        );
        d.palette = "custom".into();
        d.colors[0] = "#123456".into();
        assert!(instructions(&d).unwrap().contains("#123456"));
        d.style = "illustration".into();
        assert!(instructions(&d).unwrap().contains("explicitly selected"));
        d.style = "not-a-style".into();
        assert!(instructions(&d).is_err());
    }
}
