<script lang="ts">
  import {t} from './i18n';
 import {onDestroy} from 'svelte';
 import {invoke} from '@tauri-apps/api/core';
 import {ZoomIn,ChevronRight} from '@lucide/svelte';
 import FitImage from './FitImage.svelte';
 import type {Photo,RenderSettings,WatermarkDetail,OffsetLimits,PreviewResult} from './types';
 export let photo:Photo|null,settings:RenderSettings,overview:PreviewResult|null=null;
 export let onlimits:(limits:OffsetLimits|null)=>void;
 export let collapsed=false,oncollapse:(collapsed:boolean)=>void=()=>{};
 let detail:WatermarkDetail|null=null,error='',busy=false,zoom=1,view='detail';
 let shownPhoto='';
 let revision=0,disposed=false,inFlight=false,queued:{revision:number;settings:RenderSettings;id:string}|null=null;
 let timer:ReturnType<typeof setTimeout>|undefined;
 $: key=JSON.stringify({id:photo?.id,settings});
 $: refresh(key);
 $: image=view==='placement'&&overview?{src:overview.dataUrl,width:overview.width,height:overview.height}:{src:detail?.dataUrl??'',width:detail?.viewWidth??1,height:detail?.viewHeight??1};
 $: positionLabel=busy?$t("同步中"):detail?`${detail.width} × ${detail.height} px · X ${detail.position.x} · Y ${detail.position.y}`:'';
 function refresh(_key:string){
  revision++;if(!photo||settings.watermark.kind==='none')return;
  if(photo.id!==shownPhoto){detail=null;error='';shownPhoto=photo.id;}
  busy=true;
  queued={revision,id:photo.id,settings:JSON.parse(JSON.stringify(settings))};
  if(!timer)timer=setTimeout(()=>{timer=undefined;void drain();},50);
 }
 async function drain(){if(inFlight||disposed)return;inFlight=true;
  try{while(queued&&!disposed){const next=queued;queued=null;busy=true;
   try{const result=await invoke<WatermarkDetail>('get_watermark_detail',{photoId:next.id,settings:next.settings});
    if(!disposed&&next.id===photo?.id&&next.revision===revision){detail=result;error='';onlimits(result.limits);}
   }catch(e){if(!disposed&&next.revision===revision){error=String(e);}}
  }}finally{inFlight=false;if(!disposed)busy=false;}
 }
 onDestroy(()=>{disposed=true;clearTimeout(timer);queued=null;});
</script>
<details class="watermark-detail" class:collapsed open={!collapsed} on:toggle={(event)=>oncollapse(!event.currentTarget.open)} on:wheel|stopPropagation aria-label={$t("实际水印效果")} aria-busy={busy}>
 <summary class="detail-heading" aria-label={$t(collapsed?"展开水印预览":"收起水印预览")} title={$t(collapsed?"展开水印预览":"收起水印预览")}>
  <strong><ZoomIn size={12}/>{#if !collapsed}<span class="detail-font" title={settings.watermark.fontFamily??''}>{settings.watermark.kind==='text'?(settings.watermark.fontFamily||$t("内置思源黑体 · 支持中文")):'PNG'}</span>{/if}</strong>{#if !collapsed}<ChevronRight size={12}/>{/if}
 </summary>
 {#if !collapsed}
 <div class="detail-body">
 <div class="detail-image" title={positionLabel}>{#if image.src}<FitImage src={image.src} width={image.width} height={image.height} {zoom} alt={$t("带周围照片与边框的实际水印预览")}/>{:else}<span>{$t(error)||$t("正在准备实际合成预览")}</span>{/if}</div>
  <div class="detail-controls"><select aria-label={$t("水印局部预览范围")} bind:value={view} on:change={()=>zoom=1}><option value="detail">{$t("局部")}</option><option value="placement">{$t("整图")}</option></select><select aria-label={$t("水印局部放大倍数")} bind:value={zoom}><option value={1}>{$t("完整显示")}</option><option value={2}>200%</option><option value={4}>400%</option></select></div>
 {#if error}<p class="field-error">{$t(error)}</p>{/if}
 </div>
 {/if}
</details>
<style>
.watermark-detail{position:absolute;inset-block-start:8px;inset-inline-end:8px;z-index:2;inline-size:160px;max-inline-size:calc(100% - 16px);margin:0;border:1px solid #d8e0d1;border-radius:6px;padding:6px;background:#f8faf5f5;box-shadow:0 2px 7px #24321b0d;}
.detail-heading{display:flex;align-items:center;justify-content:space-between;gap:4px;min-block-size:20px;color:#4d6342;cursor:pointer;list-style:none;outline-offset:3px;}
.detail-heading::-webkit-details-marker{display:none;}.detail-heading strong{display:flex;align-items:center;gap:4px;min-inline-size:0;font-weight:500;}.detail-heading :global(svg){flex-shrink:0;}
.detail-body{display:flex;flex-direction:column;gap:4px;min-inline-size:0;margin-block-start:2px;}
.detail-font{font-size:10px;color:#6a7961;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;}
.detail-controls{display:flex;gap:4px;min-inline-size:0;}.detail-controls select{flex:1;inline-size:0;min-inline-size:0;height:22px;block-size:22px;padding:2px;font-size:9px;min-block-size:22px;}.detail-controls select:first-child{flex:0 0 56px;}
.detail-image{block-size:72px;min-inline-size:0;border:1px solid #dfe6d7;border-radius:5px;}.detail-image>span{display:block;padding:4px;font-size:10px;color:#849078;}
.field-error{margin:0;}
.watermark-detail.collapsed{inline-size:28px;padding:2px;}.collapsed .detail-heading{justify-content:center;min-block-size:22px;}
</style>
