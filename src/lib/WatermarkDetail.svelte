<script lang="ts">
  import {t} from './i18n';
 import {onDestroy} from 'svelte';
 import {invoke} from '@tauri-apps/api/core';
 import {ZoomIn} from '@lucide/svelte';
 import FitImage from './FitImage.svelte';
 import type {Photo,RenderSettings,WatermarkDetail,OffsetLimits,PreviewResult} from './types';
 export let photo:Photo|null,settings:RenderSettings,overview:PreviewResult|null=null;
 export let onlimits:(limits:OffsetLimits|null)=>void;
 let detail:WatermarkDetail|null=null,error='',busy=false,zoom=1,view='detail';
 let shownPhoto='';
 let revision=0,disposed=false,inFlight=false,queued:{revision:number;settings:RenderSettings;id:string}|null=null;
 let timer:ReturnType<typeof setTimeout>|undefined;
 $: key=JSON.stringify({id:photo?.id,settings});
 $: refresh(key);
 $: image=view==='placement'&&overview?{src:overview.dataUrl,width:overview.width,height:overview.height}:{src:detail?.dataUrl??'',width:detail?.viewWidth??1,height:detail?.viewHeight??1};
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
<section class="watermark-detail" aria-label={$t("实际水印效果")} aria-busy={busy}>
 <header class="detail-heading">
  <div class="detail-title"><strong><ZoomIn size={13}/>{$t("实际水印效果")}</strong><span class="detail-font" title={settings.watermark.fontFamily??''}>{settings.watermark.kind==='text'?(settings.watermark.fontFamily||$t("内置思源黑体 · 支持中文")):'PNG'}</span></div>
  <div class="detail-controls"><select aria-label={$t("水印局部预览范围")} bind:value={view} on:change={()=>zoom=1}><option value="detail">{$t("局部细节")}</option><option value="placement">{$t("整图位置")}</option></select><select aria-label={$t("水印局部放大倍数")} bind:value={zoom}><option value={1}>{$t("完整显示 · 100%")}</option><option value={2}>200%</option><option value={4}>400%</option></select></div>
 </header>
 <div class="detail-image">{#if image.src}<FitImage src={image.src} width={image.width} height={image.height} {zoom} alt={$t("带周围照片与边框的实际水印预览")}/>{:else}<span>{$t(error)||$t("正在准备实际合成预览")}</span>{/if}</div>
 <footer><span>{$t("选字时预览保持可见；放大后可拖动查看")}</span><span class="detail-position">{busy?$t("同步中"):detail?`${detail.width} × ${detail.height} px · X ${detail.position.x} · Y ${detail.position.y}`:''}</span></footer>
 {#if error}<p class="field-error">{$t(error)}</p>{/if}
</section>
<style>
.watermark-detail{flex:0 0 auto;block-size:clamp(204px,27vh,250px);min-inline-size:0;display:flex;flex-direction:column;gap:7px;margin:0 16px 12px;border:1px solid #d8e0d1;border-radius:8px;padding:10px 12px;background:#f8faf5;}
.detail-heading,.detail-title,.detail-controls{display:flex;align-items:center;gap:10px;min-inline-size:0;}
.detail-heading{justify-content:space-between;flex-wrap:wrap;row-gap:6px;}
.detail-title{flex:1 1 180px;}.detail-heading strong{display:flex;align-items:center;gap:5px;flex-shrink:0;font-size:12px;font-weight:500;color:#4d6342;}
.detail-font{font-size:11px;color:#7d8975;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;}
.detail-controls{flex-wrap:wrap;gap:6px;}.detail-controls select{inline-size:auto;max-inline-size:150px;padding:4px 6px;font-size:11px;min-block-size:27px;}
.detail-image{flex:1;min-block-size:80px;border:1px solid #dfe6d7;border-radius:6px;}.detail-image>span{display:block;padding:12px;font-size:11px;color:#849078;}
footer{display:flex;justify-content:space-between;flex-wrap:wrap;gap:3px 10px;font-size:10px;color:#819174;}.detail-position{font-family:ui-monospace,monospace;font-variant-numeric:tabular-nums;}
.field-error{margin:0;}
@media(max-height:760px){.watermark-detail{block-size:190px;margin-block-end:8px;gap:5px;padding-block:8px;}}
</style>
