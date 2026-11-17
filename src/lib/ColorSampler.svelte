<script lang="ts">
  import {t} from './i18n';
  import { onDestroy, onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import type { ColorSample, Photo, PreviewResult, SampleImage } from './types';
  export let photo: Photo;
  export let preview: PreviewResult;
  export let image: HTMLImageElement;
  export let oncolor: (hex: string) => void;
  export let onclose: () => void;
  type Tile = SampleImage & {canvas:HTMLCanvasElement;pixels:Uint8ClampedArray};
  let surface: HTMLButtonElement, fixed:HTMLCanvasElement, following:HTMLCanvasElement, follower:HTMLDivElement;
  let proxy:Tile|null=null, tiles=new Map<string,Tile>();
  let x=Math.floor(photo.width/2), y=Math.floor(photo.height/2), hex='';
  let inside=true;
  let precise=false, error='', disposed=false, committing=false, frame=0, tileBusy=false;
  let pointer:{x:number;y:number}|null=null, pendingPoint:{clientX:number;clientY:number}|null=null;
  let followX=180, followY=100;
  const tileKey=()=>`${Math.floor(x/512)},${Math.floor(y/512)}`;
  async function decode(result:SampleImage):Promise<Tile>{
    const img=new Image();img.src=result.dataUrl;await img.decode();
    const canvas=document.createElement('canvas');canvas.width=result.width;canvas.height=result.height;
    const ctx=canvas.getContext('2d',{willReadFrequently:true})!;ctx.drawImage(img,0,0);
    return {...result,canvas,pixels:ctx.getImageData(0,0,result.width,result.height).data};
  }
  onMount(()=>{
    surface.focus({preventScroll:true});
    void invoke<SampleImage>('get_sampler_image',{photoId:photo.id,x:null,y:null}).then(decode).then(t=>{if(!disposed){proxy=t;schedule();}}).catch(e=>{if(!disposed)error=String(e);});
  });
  onDestroy(()=>{disposed=true;cancelAnimationFrame(frame);for(const t of tiles.values()){t.canvas.width=0;t.canvas.height=0;}tiles.clear();if(proxy){proxy.canvas.width=0;proxy.canvas.height=0;}proxy=null;});
  function schedule(){if(!frame&&!disposed)frame=requestAnimationFrame(draw);}
  async function loadTile(){
    const key=tileKey();if(tileBusy||tiles.has(key)||disposed)return;
    tileBusy=true;const tx=x,ty=y;
    try{
      const tile=await decode(await invoke<SampleImage>('get_sampler_image',{photoId:photo.id,x:tx,y:ty}));
      if(disposed){tile.canvas.width=0;return;}
      while(tiles.size>=3){const k=tiles.keys().next().value!;const old=tiles.get(k)!;old.canvas.width=0;old.canvas.height=0;tiles.delete(k);}
      tiles.set(key,tile);schedule();
    }catch(e){if(!disposed)error=String(e);}finally{tileBusy=false;if(!disposed&&key!==tileKey())void loadTile();}
  }
  function draw(){
    frame=0;if(disposed)return;
    if(pendingPoint&&image){
      const r=image.getBoundingClientRect(),b=preview.photoBounds,p=pendingPoint;pendingPoint=null;
      const px=(p.clientX-r.left)/r.width*preview.width,py=(p.clientY-r.top)/r.height*preview.height;
      inside=px>=b.x&&py>=b.y&&px<b.x+b.width&&py<b.y+b.height;
      if(inside){
        x=Math.max(0,Math.min(photo.width-1,Math.floor((px-b.x)/b.width*photo.width)));
        y=Math.max(0,Math.min(photo.height-1,Math.floor((py-b.y)/b.height*photo.height)));
      }
      const sr=surface.getBoundingClientRect();
      followX=Math.max(6,Math.min(sr.width-148,p.clientX-sr.left+24));
      followY=Math.max(6,Math.min(sr.height-170,p.clientY-sr.top+24));
    }
    const tile=tiles.get(tileKey());const active=tile??proxy;precise=!!tile;
    if(active){
      const sx=tile?x-active.x:x/photo.width*active.width,sy=tile?y-active.y:y/photo.height*active.height;
      const zoomX=tile?1:active.width/photo.width,zoomY=tile?1:active.height/photo.height;
      const ix=Math.max(0,Math.min(active.width-1,Math.floor(sx))),iy=Math.max(0,Math.min(active.height-1,Math.floor(sy)));
      const i=(iy*active.width+ix)*4;
      hex='#'+Array.from(active.pixels.slice(i,i+3)).map(c=>c.toString(16).padStart(2,'0')).join('').toUpperCase();
      for(const c of [fixed,following]){
        const ctx=c?.getContext('2d');if(!ctx)continue;ctx.imageSmoothingEnabled=false;ctx.clearRect(0,0,126,126);
        ctx.drawImage(active.canvas,0,0,active.width,active.height,63-(sx+.5)*6/zoomX,63-(sy+.5)*6/zoomY,active.width*6/zoomX,active.height*6/zoomY);
      }
    }
    if(follower)follower.style.transform=`translate3d(${followX}px,${followY}px,0)`;
    void loadTile();
  }
  function move(e:MouseEvent){pointer={x:e.clientX,y:e.clientY};pendingPoint={clientX:e.clientX,clientY:e.clientY};schedule();}
  async function commit(e?:MouseEvent){
    if(committing)return;if(e){move(e);cancelAnimationFrame(frame);draw();if(!inside)return;}
    committing=true;
    try{const c=await invoke<ColorSample>('sample_color',{photoId:photo.id,x,y});if(!disposed){oncolor(c.hex);onclose();}}
    catch(e){if(!disposed){error=String(e);committing=false;}}
  }
  function key(e:KeyboardEvent){
    if(e.key==='Escape'){e.preventDefault();onclose();return;}
    const moves:Record<string,[number,number]>={ArrowLeft:[-1,0],ArrowRight:[1,0],ArrowUp:[0,-1],ArrowDown:[0,1]};
    if(moves[e.key]){e.preventDefault();const [dx,dy]=moves[e.key],step=e.shiftKey?10:1;x=Math.max(0,Math.min(photo.width-1,x+dx*step));y=Math.max(0,Math.min(photo.height-1,y+dy*step));pendingPoint=null;schedule();}
    else if(e.key==='Enter'||e.key===' '){e.preventDefault();void commit();}
  }
</script>
<button bind:this={surface} class="sampling-surface" aria-label={$t("照片取色器，方向键逐像素移动，回车取色，Escape 退出")} on:mousemove={move} on:mouseleave={()=>pointer=null} on:click={commit} on:keydown={key}></button>
<div class="sampling-loupe" aria-hidden="true">
  <div class="pixel-window"><canvas bind:this={fixed} width="126" height="126"></canvas><i></i></div>
  <div class="sample-value"><span style:background={hex||'#fff'}></span><strong>{hex||$t("准备取色…")}</strong></div>
  <small>{precise?$t("原图像素"):$t("快速预览")} {x+1}, {y+1} · 6×</small>
</div>
<div bind:this={follower} class="following-loupe" class:visible={!!pointer} aria-hidden="true">
  <div class="pixel-window"><canvas bind:this={following} width="126" height="126"></canvas><i></i></div>
  <strong>{hex}</strong>
</div>
<div class="sampling-help" role="status">{error||$t("移动查看双放大镜，点击精确取色 · 方向键微调 · Esc 退出")}</div>
<style>
  .following-loupe{position:absolute;top:0;left:0;pointer-events:none;opacity:0;background:#fff;border:2px solid #fff;border-radius:68px 68px 9px 9px;box-shadow:0 3px 18px #15141255;padding:4px;will-change:transform;z-index:4;}
  .following-loupe.visible{opacity:1;}
  .following-loupe .pixel-window{border-radius:50%;}
  .following-loupe strong{display:block;text-align:center;font:11px ui-monospace,monospace;padding:5px 0 3px;}
  canvas{display:block;width:126px;height:126px;image-rendering:pixelated;}
</style>
