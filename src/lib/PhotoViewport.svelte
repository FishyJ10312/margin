<script lang="ts">
  import {t} from './i18n';
  import {onMount,onDestroy} from 'svelte';
  import {Minus,Plus,Scan} from '@lucide/svelte';
  import ColorSampler from './ColorSampler.svelte';
  import type {Photo,PreviewResult} from './types';
  export let photo:Photo, preview:PreviewResult|null=null,picking=false,loading=false;
  export let oncolor:(hex:string)=>void,onclose:()=>void;
  /** Watermark rectangle on the full-resolution canvas; null hides the handle. */
  export let markBox:{x:number;y:number;width:number;height:number}|null=null;
  /** Drag deltas in full-resolution canvas pixels, relative to where the drag began. */
  export let onmarkdrag:(dx:number,dy:number,phase:'start'|'move'|'end')=>void=()=>{};
  let markDrag: { x: number; y: number; box: NonNullable<typeof markBox>; zoom: number } | null = null;
  let markShift = { x: 0, y: 0 };
  $: handleBox = markDrag?.box ?? markBox;
  $: handle = handleBox ? {
    left: width / 2 + panX + (handleBox.x + markShift.x - fullW / 2) * zoom,
    top: height / 2 + panY + (handleBox.y + markShift.y - fullH / 2) * zoom,
    w: Math.max(12, handleBox.width * zoom), h: Math.max(12, handleBox.height * zoom),
  } : null;
  function markDown(event: PointerEvent) {
    if (!event.isPrimary || event.button !== 0 || !markBox) return;
    event.preventDefault();
    event.stopPropagation();
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    markDrag = { x: event.clientX, y: event.clientY, box: markBox, zoom };
    onmarkdrag(0, 0, 'start');
  }
  function markMove(event: PointerEvent) {
    if (!markDrag) return;
    markShift = { x: (event.clientX - markDrag.x) / markDrag.zoom, y: (event.clientY - markDrag.y) / markDrag.zoom };
    onmarkdrag(markShift.x, markShift.y, 'move');
  }
  function markUp() {
    if (!markDrag) return;
    onmarkdrag(markShift.x, markShift.y, 'end');
    markDrag = null;
    markShift = { x: 0, y: 0 };
  }
  function markKey(e:KeyboardEvent){const step=(e.shiftKey?10:1)/Math.max(zoom,.01);const d:Record<string,[number,number]>={ArrowLeft:[-step,0],ArrowRight:[step,0],ArrowUp:[0,-step],ArrowDown:[0,step]};if(!d[e.key])return;e.preventDefault();e.stopPropagation();onmarkdrag(0,0,'start');onmarkdrag(d[e.key][0],d[e.key][1],'end');}
  let host:HTMLDivElement,image:HTMLImageElement,panSurface:HTMLButtonElement;
  let width=600,height=500,zoom=.1,panX=0,panY=0,fit=true;
  let dragging=false,drag:{x:number;y:number;px:number;py:number}|null=null;
  let frame=0,disposed=false,observer:ResizeObserver;
  $: fullW=preview?.width??photo.width;
  $: fullH=preview?.height??photo.height;
  $: fitZoom=Math.min((width-64)/fullW,(height-120)/fullH,1);
  $: if(fit)zoom=Math.max(.005,fitZoom);
  $: percent=Math.round(zoom*1000)/10;
  $: if(image&&fullW&&fullH&&zoom)paint();
  function clampPan(){const mx=Math.max(0,(fullW*zoom-width)/2+80),my=Math.max(0,(fullH*zoom-height)/2+80);panX=Math.max(-mx,Math.min(mx,panX));panY=Math.max(-my,Math.min(my,panY));}
  function paint(){if(!frame&&!disposed)frame=requestAnimationFrame(()=>{frame=0;const target=image;if(!target)return;clampPan();target.style.width='1400px';target.style.height=`${1400*fullH/fullW}px`;target.style.transform=`translate(-50%,-50%) translate3d(${panX}px,${panY}px,0) scale(${fullW*zoom/1400})`;});}
  function setZoom(value:number,ax=0,ay=0){if(!Number.isFinite(value))return;const next=Math.max(.005,Math.min(4,value)),factor=next/zoom;panX=ax-(ax-panX)*factor;panY=ay-(ay-panY)*factor;fit=false;zoom=next;paint();}
  function fitView(){fit=true;panX=0;panY=0;paint();}
  function wheel(e:WheelEvent){if(picking)return;e.preventDefault();if(markDrag)return;const r=host.getBoundingClientRect();setZoom(zoom*Math.exp(-e.deltaY*(e.deltaMode===1?.04:.002)),e.clientX-r.left-width/2,e.clientY-r.top-height/2);}
  function down(e:PointerEvent){if(!e.isPrimary||e.button!==0)return;e.preventDefault();panSurface.setPointerCapture(e.pointerId);drag={x:e.clientX,y:e.clientY,px:panX,py:panY};dragging=true;}
  function move(e:PointerEvent){if(!drag)return;panX=drag.px+e.clientX-drag.x;panY=drag.py+e.clientY-drag.y;paint();}
  function key(e:KeyboardEvent){const delta=e.shiftKey?80:20;const steps:Record<string,[number,number]>={ArrowLeft:[delta,0],ArrowRight:[-delta,0],ArrowUp:[0,delta],ArrowDown:[0,-delta]};if(steps[e.key]){e.preventDefault();panX+=steps[e.key][0];panY+=steps[e.key][1];paint();}else if(['+','=','-','0','1'].includes(e.key)){e.preventDefault();if(e.key==='0')fitView();else setZoom(e.key==='1'?1:zoom*(e.key==='-'?1/1.15:1.15));}}
  onMount(()=>{observer=new ResizeObserver(entries=>{const r=entries[0].contentRect;width=r.width;height=r.height;paint();});observer.observe(host);host.addEventListener('wheel',wheel,{passive:false});});
  onDestroy(()=>{disposed=true;cancelAnimationFrame(frame);observer?.disconnect();host?.removeEventListener('wheel',wheel);});
</script>
<div bind:this={host} class="canvas photo-viewport" class:dragging>
  <img bind:this={image} class="rendered-photo zoom-photo" src={preview?.dataUrl||photo.thumbnail} alt={$t("{0} 的加框预览", {0:photo.name})} on:load={paint}/>
  <button bind:this={panSurface} class="pan-surface" aria-label={$t("照片视图，拖动平移，滚轮缩放，0 适应，1 原始大小")} on:pointerdown={down} on:pointermove={move} on:lostpointercapture={()=>{drag=null;dragging=false;}} on:keydown={key}></button>
  {#if handle&&!picking}<button type="button" class="mark-handle" class:dragging={!!markDrag} style:left="{handle.left}px" style:top="{handle.top}px" style:width="{handle.w}px" style:height="{handle.h}px" aria-label={$t("拖动调整水印位置，方向键微调")} title={$t("拖动调整水印位置，方向键微调")} on:pointerdown={markDown} on:pointermove={markMove} on:pointerup={markUp} on:lostpointercapture={markUp} on:keydown={markKey}></button>{/if}
  <slot />
  {#if picking&&preview}<ColorSampler {photo} {preview} {image} {oncolor} {onclose}/>{/if}
  {#if loading}<span class="preview-pending">{$t("更新预览")}</span>{/if}
  {#if !picking}<div class="zoom-toolbar" role="toolbar" aria-label={$t("照片缩放")}>
    <button class="icon-button" aria-label={$t("缩小照片")} title={$t("缩小（-）")} on:click={()=>setZoom(zoom/1.15)}><Minus size={14}/></button>
    <label class="zoom-percent" title={$t("输入缩放百分比；按 1 查看原始大小")}><input aria-label={$t("照片缩放百分比")} type="number" min="0.5" max="400" step="0.1" value={percent} on:change={(e)=>setZoom(e.currentTarget.valueAsNumber/100)}/><span>%</span></label>
    <button class="icon-button" aria-label={$t("放大照片")} title={$t("放大（+）")} on:click={()=>setZoom(zoom*1.15)}><Plus size={14}/></button>
    <span class="zoom-divider" aria-hidden="true"></span>
    <button class="text-button zoom-fit" on:click={fitView} aria-pressed={fit} title={$t("适应窗口（0）")}><Scan size={13}/>{$t("适应")}</button>
    <slot name="tools" />
  </div>{/if}
</div>
<style>
.zoom-toolbar{position:absolute;z-index:3;left:50%;bottom:14px;transform:translateX(-50%);display:flex;align-items:center;gap:2px;padding:4px;border-radius:10px;font-size:12px;color:var(--muted);white-space:nowrap;}
.zoom-percent{display:flex;align-items:center;color:var(--ink);}
.zoom-toolbar input[type=number]{width:48px;text-align:right;height:28px;padding:0 2px;border:1px solid transparent;border-radius:6px;background:transparent;color:inherit;font:inherit;-moz-appearance:textfield;appearance:textfield;}
.zoom-toolbar input[type=number]::-webkit-inner-spin-button,.zoom-toolbar input[type=number]::-webkit-outer-spin-button{-webkit-appearance:none;margin:0;}
.zoom-toolbar input[type=number]:hover,.zoom-toolbar input[type=number]:focus{border-color:var(--border);}
.zoom-percent span{padding-inline-end:4px;color:var(--muted);}
.zoom-divider{width:1px;height:16px;margin:0 4px;background:var(--border-strong,#3a3a3a);}
.zoom-toolbar :global(.text-button){height:28px;padding:0 9px;border-radius:6px;font-size:12px;gap:6px;color:var(--muted);}
.zoom-toolbar :global(.text-button[aria-pressed=true]){color:var(--accent,#e3a446);}
.photo-viewport{display:block;position:relative;overflow:hidden;padding:0;isolation:isolate;min-height:100px;touch-action:none;}
.zoom-photo{position:absolute;top:50%;left:50%;max-width:none;max-height:none;object-fit:fill;pointer-events:none;transform-origin:center;will-change:transform;filter:none;box-shadow:none;transition:none;-webkit-user-select:none;user-select:none;}
.pan-surface{position:absolute;inset:0;width:100%;height:100%;border:0;background:transparent;cursor:grab;touch-action:none;-webkit-user-select:none;user-select:none;}
.dragging .pan-surface{cursor:grabbing;}
.mark-handle{position:absolute;z-index:1;padding:0;border:1px dashed transparent;border-radius:2px;background:transparent;cursor:move;touch-action:none;}
.mark-handle:hover,.mark-handle:focus-visible,.mark-handle.dragging{border-color:var(--primary,#f0a23b);background:rgb(240 162 59 / .08);outline:none;}
</style>
