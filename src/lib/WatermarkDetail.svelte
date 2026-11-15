<script lang="ts">
  import {t} from './i18n';
 import {onDestroy} from 'svelte';
 import {invoke} from '@tauri-apps/api/core';
 import {ZoomIn,X} from '@lucide/svelte';
 import FitImage from './FitImage.svelte';
 import type {Photo,RenderSettings,WatermarkDetail,OffsetLimits,PreviewResult} from './types';
 export let photo:Photo|null,settings:RenderSettings,overview:PreviewResult|null=null;
 export let onlimits:(limits:OffsetLimits|null)=>void;
 let dialog:HTMLDialogElement,detail:WatermarkDetail|null=null,error='',busy=false,zoom=1,view='detail';
 let shownPhoto='';
 let revision=0,shown=0,disposed=false,inFlight=false,queued:{revision:number;settings:RenderSettings;id:string}|null=null;
 let timer:ReturnType<typeof setTimeout>|undefined;
 $: key=JSON.stringify({id:photo?.id,settings});
 $: refresh(key);
 $: image=view==='placement'&&overview?{src:overview.dataUrl,width:overview.width,height:overview.height}:{src:detail?.dataUrl??'',width:detail?.viewWidth??1,height:detail?.viewHeight??1};
 function refresh(_key:string){
  revision++;if(!photo||settings.watermark.kind==='none')return;
  if(photo.id!==shownPhoto){detail=null;shown=0;shownPhoto=photo.id;}
  queued={revision,id:photo.id,settings:JSON.parse(JSON.stringify(settings))};
  if(!timer)timer=setTimeout(()=>{timer=undefined;void drain();},50);
 }
 async function drain(){if(inFlight||disposed)return;inFlight=true;
  try{while(queued&&!disposed){const next=queued;queued=null;busy=true;
   try{const result=await invoke<WatermarkDetail>('get_watermark_detail',{photoId:next.id,settings:next.settings});
    if(!disposed&&next.id===photo?.id&&next.revision>=shown){detail=result;shown=next.revision;error='';onlimits(result.limits);}
   }catch(e){if(!disposed&&next.revision===revision){error=String(e);}}
  }}finally{inFlight=false;if(!disposed)busy=false;}
 }
 onDestroy(()=>{disposed=true;dialog?.close();clearTimeout(timer);queued=null;});
</script>
<svelte:window on:keydown={(e)=>{if(e.key==='Escape'&&dialog?.open&&dialog.contains(document.activeElement)){dialog.close();}}}/>
<div class="watermark-detail">
 <div class="detail-heading"><strong><ZoomIn size={13}/>{$t("实际水印效果")}</strong><span>{busy?$t("同步中"):detail?`${detail.width} × ${detail.height} px`:''}</span></div>
 <div class="detail-controls"><select aria-label={$t("水印局部预览范围")} bind:value={view} on:change={()=>zoom=1}><option value="detail">{$t("局部细节")}</option><option value="placement">{$t("整图位置")}</option></select><select aria-label={$t("水印局部放大倍数")} bind:value={zoom}><option value={1}>{$t("完整显示 · 100%")}</option><option value={2}>200%</option><option value={4}>400%</option></select></div>
 <div class="detail-image">{#if image.src}<FitImage src={image.src} width={image.width} height={image.height} {zoom} alt={$t("带周围照片与边框的实际水印预览")}/>{:else}<span>{$t(error)||$t("正在准备实际合成预览")}</span>{/if}</div>
 <button class="text-button detail-expand" disabled={!detail} on:click={()=>{zoom=1;dialog.show();}}><ZoomIn size={12}/>{$t("放大查看水印")}</button>
 <p class="field-help">{$t("100% 完整显示；放大后可拖动查看。字体、大小、位置与主图同步。")}</p>
 {#if detail}<p class="detail-position">X {detail.position.x} · Y {detail.position.y} px</p>{/if}
 {#if error}<p class="field-error">{$t(error)}</p>{/if}
</div>
<dialog bind:this={dialog} class="detail-dialog" aria-labelledby="watermark-detail-title">
 <header><div><h2 id="watermark-detail-title">{$t("实际水印效果")}</h2><p>{$t("可在右侧继续调整，预览实时同步")}</p></div><button class="icon-button" aria-label={$t("关闭水印细节")} on:click={()=>dialog.close()}><X size={18}/></button></header>
 <div class="dialog-controls"><select aria-label={$t("放大窗口预览范围")} bind:value={view} on:change={()=>zoom=1}><option value="detail">{$t("局部细节")}</option><option value="placement">{$t("整图位置")}</option></select><select aria-label={$t("放大窗口倍数")} bind:value={zoom}><option value={1}>{$t("完整显示 · 100%")}</option><option value={2}>200%</option><option value={4}>400%</option></select><span>{busy?$t("同步中"):$t("导出字号保持不变")}</span></div>
 <div class="detail-image expanded">{#if image.src}<FitImage src={image.src} width={image.width} height={image.height} {zoom} alt={$t("实际合成后的水印及周围区域")}/>{/if}</div>
 <p class="field-help dialog-note">{$t("100% 以窗口大小完整显示整个区域；200% / 400% 用于检查字形，可拖动或用方向键平移。")}</p>
</dialog>
<style>
.watermark-detail{margin-top:14px;border:1px solid #dde4d5;border-radius:8px;padding:10px;background:#f8faf5;container-type:inline-size;}
.detail-heading{display:flex;justify-content:space-between;gap:4px;align-items:center;font-size:10px;color:#7d8975;}.detail-heading strong{display:flex;align-items:center;gap:5px;font-size:12px;font-weight:500;color:#4d6342;}
.detail-controls{display:flex;flex-wrap:wrap;gap:6px;margin:9px 0;}.detail-controls select{flex:1;padding:4px;font-size:10px;min-height:27px;min-width:0;}
.detail-image{height:156px;border:1px solid #dfe6d7;border-radius:6px;overflow:hidden;}.detail-image>span{display:block;padding:12px;font-size:11px;color:#849078;}
.detail-expand{font-size:11px;margin-top:9px;}.detail-position{font:10px ui-monospace,monospace;color:#819174;margin:8px 0 0;}
.detail-dialog{position:fixed;inset:auto;left:20px;top:14vh;z-index:70;margin:0;width:min(820px,calc(100vw - 365px));min-width:240px;max-height:80vh;border:1px solid #d7e0cd;border-radius:12px;padding:20px;background:#fafcf7;color:#43563a;box-shadow:0 16px 70px #10200d44;}
.detail-dialog::backdrop{background:#1b2d1a55;}.detail-dialog header{display:flex;justify-content:space-between;align-items:flex-start;gap:12px;}.detail-dialog h2{font-size:17px;font-weight:550;}.detail-dialog header p{font-size:12px;color:#7b8872;margin:7px 0 14px;}.dialog-controls{display:flex;flex-wrap:wrap;align-items:center;gap:8px;margin-bottom:12px;}.dialog-controls select{width:auto;font-size:12px;min-height:30px;}.dialog-controls span{font-size:11px;color:#7b8872;}.expanded{height:min(400px,53vh);}.dialog-note{margin:12px 0 0;}
@container (width < 220px){.detail-controls select{flex-basis:100%;}}
</style>
