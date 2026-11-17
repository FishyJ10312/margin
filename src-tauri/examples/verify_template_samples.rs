//! Offline catalogue preview generation; never imports photos or invokes a model.
use base64::{engine::general_purpose::STANDARD, Engine};
use liubai_lib::{designs, imaging, model::RenderSettings};
use serde::Deserialize;
#[derive(Deserialize)]
struct FrameTemplate { id: String, settings: RenderSettings }
use std::{collections::HashMap, fs, path::PathBuf, time::Instant};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if liubai_lib::source_io::worker_entry_if_requested() { return Ok(()); }
    let args:Vec<_>=std::env::args().collect();
    let output=PathBuf::from(args.get(1).ok_or("output directory required")?);
    fs::create_dir_all(&output)?;
    let start=Instant::now();
    let catalogue=designs::library()?;
    for item in &catalogue {
        fs::write(output.join(format!("{}.jpg",item.id)), STANDARD.decode(item.thumbnail.split_once(',').ok_or("invalid thumbnail")?.1)?)?;
    }
    fs::write(output.join("library.json"),serde_json::to_vec(&catalogue)?)?;
    let mut personal=HashMap::new();
    if let Some(file)=args.get(2) {
        let state:serde_json::Value=serde_json::from_slice(&fs::read(file)?)?;
        let templates:Vec<FrameTemplate>=serde_json::from_value(state["frameTemplates"].clone())?;
        for item in templates {
            let thumbnail=imaging::frame_thumbnail(&item.settings)?;
            fs::write(output.join(format!("personal-{}.jpg",item.id)), STANDARD.decode(thumbnail.split_once(',').ok_or("invalid thumbnail")?.1)?)?;
            personal.insert(item.id,thumbnail);
        }
    }
    fs::write(output.join("personal-previews.json"),serde_json::to_vec(&personal)?)?;
    println!("{}",serde_json::json!({"builtins":catalogue.len(),"personal":personal.len(),"elapsedMillis":start.elapsed().as_millis()}));
    Ok(())
}
