import type {RenderSettings} from './types';

/** Canvas geometry for an at-a-glance overview. Mirrors `layout::calculate` in
 * src-tauri/src/layout.rs (minimum border on the shorter edge, a fixed ratio only
 * adds space, the photo stays whole and centred). The real preview and export
 * are still rendered natively; this only sizes miniatures in the overview. */
export interface CanvasLayout {width:number;height:number;x:number;y:number;photoWidth:number;photoHeight:number}

export function canvasLayout(width:number,height:number,settings:Pick<RenderSettings,'borderPercent'|'ratio'>):CanvasLayout{
  const border=Math.round(Math.min(width,height)*settings.borderPercent/100);
  let canvasW=width+2*border,canvasH=height+2*border;
  const ratio=settings.ratio;
  if(ratio&&ratio.width>0&&ratio.height>0){
    const r=ratio.width/ratio.height;
    if(canvasW/canvasH<r)canvasW=Math.ceil(canvasH*r);
    else canvasH=Math.ceil(canvasW/r);
  }
  return {width:canvasW,height:canvasH,photoWidth:width,photoHeight:height,x:Math.floor((canvasW-width)/2),y:Math.floor((canvasH-height)/2)};
}
