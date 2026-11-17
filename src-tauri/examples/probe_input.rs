//! One isolated local input probe for the bounded malformed-image corpus.
use liubai_lib::{imaging,model::RenderSettings};
fn main()->Result<(),Box<dyn std::error::Error>> {
    if liubai_lib::source_io::worker_entry_if_requested() { return Ok(()); }
    let path=std::path::PathBuf::from(std::env::args().nth(1).ok_or("input")?);
    let bytes=std::fs::read(&path)?;let before=ring::digest::digest(&ring::digest::SHA256,&bytes);
    let mut settings=RenderSettings::default();settings.export.long_edge=Some(512);
    let started=std::time::Instant::now();let result=imaging::inspect_image(&path).and_then(|_|imaging::preview(&path,&settings,320));
    let mut row=serde_json::json!({"accepted":result.is_ok(),"seconds":started.elapsed().as_secs_f64(),"sourceUnchanged":before.as_ref()==ring::digest::digest(&ring::digest::SHA256,&std::fs::read(path)?).as_ref()});
    match result{Ok(v)=>row["dimensions"]=serde_json::json!([v.width,v.height]),Err(e)=>row["error"]=e.into()};
    #[cfg(unix)] {
        let mut usage:libc::rusage=unsafe{std::mem::zeroed()};
        if unsafe{libc::getrusage(libc::RUSAGE_SELF,&mut usage)}==0 {row["peakRssKiB"]=(if cfg!(target_os="macos"){usage.ru_maxrss as u64/1024}else{usage.ru_maxrss as u64}).into();}
    }
    println!("{row}");Ok(())
}
