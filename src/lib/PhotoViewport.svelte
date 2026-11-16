<script lang="ts">
  import {t} from './i18n';
  import {onMount,onDestroy} from 'svelte';
  import {Minus,Plus,Scan,Move} from '@lucide/svelte';
  import ColorSampler from './ColorSampler.svelte';
  import type {Photo,PreviewResult} from './types';
  export let photo:Photo, preview:PreviewResult|null=null,picking=false,loading=false;
  export let oncolor:(hex:string)=>void,onclose:()=>void;
  let host:HTMLDivElement,image:HTMLImageElement,panSurface:HTMLButtonElement;
  let width=600,height=500,zoom=.1,panX=0,panY=0,fit=true;
  let dragging=false,drag:{x:number;y:number;px:number;py:number}|null=null;
  let frame=0,disposed=false,observer:ResizeObserver;
  $: fullW=preview?.width??photo.width;
  $: fullH=preview?.height??photo.height;
  $: fitZoom=Math.min((width-48)/fullW,(height-48)/fullH,1);
  $: if(fit)zoom=Math.max(.005,fitZoom);
  $: percent=Math.round(zoom*1000)/10;
  $: if(image&&fullW&&fullH&&zoom)paint();
  function clampPan(){const mx=Math.max(0,(fullW*zoom-width)/2+80),my=Math.max(0,(fullH*zoom-height)/2+80);panX=Math.max(-mx,Math.min(mx,panX));panY=Math.max(-my,Math.min(my,panY));}
  function paint(){if(!frame&&!disposed)frame=requestAnimationFrame(()=>{frame=0;const target=image;if(!target)return;clampPan();target.style.width='1400px';target.style.height=`${1400*fullH/fullW}px`;target.style.transform=`translate(-50%,-50%) translate3d(${panX}px,${panY}px,0) scale(${fullW*zoom/1400})`;});}
  function setZoom(value:number,ax=0,ay=0){if(!Number.isFinite(value))return;const next=Math.max(.005,Math.min(4,value)),factor=next/zoom;panX=ax-(ax-panX)*factor;panY=ay-(ay-panY)*factor;fit=false;zoom=next;paint();}
  function fitView(){fit=true;panX=0;panY=0;paint();}
  function wheel(e:WheelEvent){if(picking)return;e.preventDefault();const r=host.getBoundingClientRect();setZoom(zoom*Math.exp(-e.deltaY*(e.deltaMode===1?.04:.002)),e.clientX-r.left-width/2,e.clientY-r.top-height/2);}
  function down(e:PointerEvent){if(!e.isPrimary||e.button!==0)return;e.preventDefault();panSurface.setPointerCapture(e.pointerId);drag={x:e.clientX,y:e.clientY,px:panX,py:panY};dragging=true;}
  function move(e:PointerEvent){if(!drag)return;panX=drag.px+e.clientX-drag.x;panY=drag.py+e.clientY-drag.y;paint();}
  function key(e:KeyboardEvent){const delta=e.shiftKey?80:20;const steps:Record<string,[number,number]>={ArrowLeft:[delta,0],ArrowRight:[-delta,0],ArrowUp:[0,delta],ArrowDown:[0,-delta]};if(steps[e.key]){e.preventDefault();panX+=steps[e.key][0];panY+=steps[e.key][1];paint();}else if(['+','=','-','0','1'].includes(e.key)){e.preventDefault();if(e.key==='0')fitView();else setZoom(e.key==='1'?1:zoom*(e.key==='-'?1/1.15:1.15));}}
  onMount(()=>{observer=new ResizeObserver(entries=>{const r=entries[0].contentRect;width=r.width;height=r.height;paint();});observer.observe(host);host.addEventListener('wheel',wheel,{passive:false});});
  onDestroy(()=>{disposed=true;cancelAnimationFrame(frame);observer?.disconnect();host?.removeEventListener('wheel',wheel);});
</script>
<div class="zoom-toolbar" aria-label={$t("照片缩放")}>
  <button class="icon-button" aria-label={$t("缩小照片")} on:click={()=>setZoom(zoom/1.15)}><Minus size={14}/></button>
  <input aria-label={$t("照片缩放百分比")} type="number" min="0.5" max="400" step="0.1" value={percent} on:change={(e)=>setZoom(e.currentTarget.valueAsNumber/100)}/><span>%</span>
  <button class="icon-button" aria-label={$t("放大照片")} on:click={()=>setZoom(zoom*1.15)}><Plus size={14}/></button>
  <input class="zoom-range" aria-label={$t("连续缩放照片")} type="range" min="-5.2983" max="1.3863" step="0.005" value={Math.log(zoom)} on:input={(e)=>setZoom(Math.exp(e.currentTarget.valueAsNumber))}/>
  <button class="text-button" on:click={fitView} aria-pressed={fit}><Scan size={13}/>{$t("适应")}</button>
  <button class="text-button" on:click={()=>setZoom(1)}>100%</button>
</div>
<div bind:this={host} class="canvas photo-viewport" class:dragging>
  <img bind:this={image} class="rendered-photo zoom-photo" src={preview?.dataUrl||photo.thumbnail} alt={$t("{0} 的加框预览", {0:photo.name})} on:load={paint}/>
  <button bind:this={panSurface} class="pan-surface" aria-label={$t("照片视图，拖动平移，滚轮缩放，0 适应，1 原始大小")} on:pointerdown={down} on:pointermove={move} on:lostpointercapture={()=>{drag=null;dragging=false;}} on:keydown={key}></button>
  <slot />
  {#if picking&&preview}<ColorSampler {photo} {preview} {image} {oncolor} {onclose}/>{/if}
  {#if loading}<span class="preview-pending">{$t("更新预览")}</span>{/if}
  {#if !picking}<span class="pan-hint"><Move size={11}/> {$t("滚轮缩放 · 拖动平移")}</span>{/if}
</div>
<style>
.zoom-toolbar{display:flex;align-items:center;justify-content:center;gap:5px;padding:0 16px 7px;color:#76836e;font-size:11px;}
.zoom-toolbar input[type=number]{width:61px;text-align:right;height:26px;padding:3px;border:1px solid #dfe5d7;background:#fff9;}
.zoom-range{max-width:120px;margin:0 8px;}
.photo-viewport{display:block;position:relative;overflow:hidden;padding:0;isolation:isolate;min-height:100px;touch-action:none;}
.zoom-photo{position:absolute;top:50%;left:50%;max-width:none;max-height:none;object-fit:fill;pointer-events:none;transform-origin:center;will-change:transform;filter:none;box-shadow:none;transition:none;-webkit-user-select:none;user-select:none;}
.pan-surface{position:absolute;inset:0;width:100%;height:100%;border:0;background:transparent;cursor:grab;touch-action:none;-webkit-user-select:none;user-select:none;}
.dragging .pan-surface{cursor:grabbing;}.pan-hint{position:absolute;bottom:9px;right:12px;font-size:10px;color:#8c9983;display:flex;align-items:center;gap:5px;pointer-events:none;}
</style>
