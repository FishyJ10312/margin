import {currentMonitor,getCurrentWindow,PhysicalPosition,PhysicalSize} from '@tauri-apps/api/window';

let expanding:Promise<void>|null=null;

// Grow only when the preview is opened, never while choosing a font or resizing
// by hand. All measurements stay in physical pixels, including Retina displays.
export function ensureWatermarkWorkspace():Promise<void>{
 if(expanding)return expanding;
 expanding=expand().finally(()=>expanding=null);
 return expanding;
}

async function expand(){
 try{
  const app=getCurrentWindow();
  const [fullscreen,maximized]=await Promise.all([app.isFullscreen(),app.isMaximized()]);
  if(fullscreen||maximized)return;
  const [inner,outer,position,scale,monitor]=await Promise.all([app.innerSize(),app.outerSize(),app.outerPosition(),app.scaleFactor(),currentMonitor()]);
  if(!monitor||!(scale>0))return;
  const area=monitor.workArea,padding=8*scale;
  const frameWidth=Math.max(0,outer.width-inner.width),frameHeight=Math.max(0,outer.height-inner.height);
  const width=Math.round(Math.max(inner.width,Math.min(1280*scale,area.size.width-frameWidth-padding*2)));
  const height=Math.round(Math.max(inner.height,Math.min(900*scale,area.size.height-frameHeight-padding*2)));
  if(width<=inner.width&&height<=inner.height)return;
  await app.setSize(new PhysicalSize(width,height));
  const x=Math.round(Math.max(area.position.x+padding,Math.min(position.x,area.position.x+area.size.width-width-frameWidth-padding)));
  const y=Math.round(Math.max(area.position.y+padding,Math.min(position.y,area.position.y+area.size.height-height-frameHeight-padding)));
  if(x!==position.x||y!==position.y)await app.setPosition(new PhysicalPosition(x,y));
 }catch{
  // A window manager can decline resizing. The compact layout still keeps the
  // photo, watermark preview and font controls available without a dialog.
 }
}
