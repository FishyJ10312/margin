<script lang="ts">
 import {t,language} from './i18n';
 import {onMount,onDestroy,tick} from 'svelte';
 import {X,Bookmark,Trash2,LoaderCircle,Plus} from '@lucide/svelte';
 import {invoke} from '@tauri-apps/api/core';
 import type {FrameTemplate,RenderSettings,BuiltinFrame,Photo,PreviewResult} from './types';
 export let currentSettings:RenderSettings;
 export let photo:Photo|null=null;
 export let currentPreview='';
 export let templates:FrameTemplate[]=[];
 export let draft:RenderSettings|null=null;
 export let initialName='';
 export let initialCategory='all';
 export let onsave:(name:string,settings:RenderSettings)=>Promise<void>;
 export let ondelete:(id:string)=>Promise<void>;
 export let onapply:(settings:RenderSettings)=>void;
 export let onclose:()=>void;
 type Choice={id:string;name:string;description:string;settings:RenderSettings;thumbnail:string};
 const clone=<T,>(v:T):T=>JSON.parse(JSON.stringify(v));
 let builtins:BuiltinFrame[]=[],tab='design',category=initialCategory,fetching=true;
 let savedPreviews:Record<string,string>={};
 let dialog:HTMLDialogElement,name=initialName||$t('我的艺术边框'),busy=false,problem='',saved=false;
 let chosen:Choice|null=null,candidate:RenderSettings|null=null,selectedId='',keepLayout=true;
 let previewData='',previewError='',previewPending=false,compare=false,alive=true,ready=false;
 let version=0,running=false,timer:ReturnType<typeof setTimeout>|undefined;
 let queued:{version:number;photoId:string;settings:RenderSettings}|null=null;
 const categories=[['all','全部'],['film','胶片'],['photo','摄影装帧'],['graphic','平面设计'],['lines','线条'],['botanical','植物'],['geometry','几何'],['illustration','插画']];
 $: visible=builtins.filter(v=>category==='all'||v.category===category);
 $: serialized=candidate?JSON.stringify(candidate):'';
 $: if(ready&&serialized)schedule(photo?.id||'',serialized);
 onMount(()=>{dialog.showModal();if(draft)tab='mine';void load();});
 onDestroy(()=>{alive=false;version++;queued=null;clearTimeout(timer);});
 async function load(){fetching=true;try{const [catalog,personal]=await Promise.all([invoke<BuiltinFrame[]>('list_builtin_frames'),templates.length?invoke<Record<string,string>>('get_frame_template_previews'):Promise.resolve({})]);if(!alive)return;builtins=catalog;savedPreviews=personal||{};if(chosen&&savedPreviews[chosen.id])chosen={...chosen,thumbnail:savedPreviews[chosen.id]};if(!chosen&&draft)choose({id:'draft',name:initialName||$t('待保存方案'),description:'',settings:draft,thumbnail:''});else if(!chosen&&builtins.length)pickBuiltin(builtins.find(v=>category==='all'||v.category===category)||builtins[0]);}catch(e){problem=String(e);}finally{if(alive){fetching=false;ready=true;}}}
 function choose(item:Choice){chosen=item;selectedId=item.id;compare=false;candidate={...clone(item.settings),ratio:clone(keepLayout?currentSettings.ratio:item.settings.ratio),borderPercent:keepLayout?currentSettings.borderPercent:item.settings.borderPercent,watermark:clone(currentSettings.watermark),export:clone(currentSettings.export)};}
 function pickBuiltin(v:BuiltinFrame){choose({id:v.id,name:$language==='en'?v.englishName:v.name,description:$t(v.description),settings:v.settings,thumbnail:v.thumbnail});}
 function setCategory(value:string){category=value;const first=builtins.find(v=>value==='all'||v.category===value);if(first)pickBuiltin(first);}
 function pickSaved(item:FrameTemplate){choose({...item,thumbnail:savedPreviews[item.id]||item.thumbnail,description:''});}
 function setTab(value:string){tab=value;if(value==='mine'){if(draft)choose({id:'draft',name:initialName||$t('待保存方案'),description:'',settings:draft,thumbnail:''});else if(templates[0])pickSaved(templates[0]);}else{const first=builtins.find(v=>category==='all'||v.category===category);if(first)pickBuiltin(first);}}
 function layoutChoice(){if(!candidate||!chosen)return;candidate.ratio=clone(keepLayout?currentSettings.ratio:chosen.settings.ratio);candidate.borderPercent=keepLayout?currentSettings.borderPercent:chosen.settings.borderPercent;}
 function schedule(photoId:string,value:string){const settings:RenderSettings=JSON.parse(value);version++;previewError='';if(!/^#[\dA-Fa-f]{6}$/.test(settings.frameColor)){queued=null;previewPending=false;previewError=$t('请输入六位 HEX 颜色');return;}if(!photoId){previewData=chosen?.thumbnail||'';previewPending=false;return;}queued={version,photoId,settings};previewPending=true;if(!timer)timer=setTimeout(()=>{timer=undefined;void drain();},70);}
 async function drain(){if(running)return;running=true;try{while(queued&&alive){const job=queued;queued=null;try{const result=await invoke<PreviewResult>('get_preview',{photoId:job.photoId,settings:job.settings});if(alive&&job.version===version){previewData=result.dataUrl;previewError='';previewPending=false;}}catch(e){if(alive&&job.version===version){previewError=String(e);previewPending=false;}}}}finally{running=false;}}
 async function save(){if(!draft||busy)return;busy=true;problem='';try{await onsave(name,selectedId==='draft'&&candidate?candidate:draft);if(!alive)return;saved=true;draft=null;await tick();const item=templates[templates.length-1];if(item)choose({...item,description:''});}catch(e){if(alive)problem=String(e);}finally{if(alive)busy=false;}}
 async function remove(id:string){if(busy)return;busy=true;problem='';try{await ondelete(id);if(chosen?.id===id){candidate=null;chosen=null;selectedId='';}}catch(e){problem=String(e);}finally{busy=false;}}
 function palette(colors:string[]){if(candidate?.artwork?.design)candidate.artwork.design.colors=[...colors];}
 function apply(){if(candidate&&!previewPending&&!previewError)onapply(clone(candidate));}
</script>
<dialog bind:this={dialog} class="templates-dialog" aria-labelledby="templates-title" on:cancel|preventDefault={onclose}>
 <header><div><h2 id="templates-title"><Bookmark size={18}/>{$t('边框模板')}</h2><p>{$t('胶片、卡纸与图案，直接预览选用，无需 AI 生成')}</p></div><button type="button" class="icon-button" aria-label={$t('关闭模板库')} on:click={onclose}><X size={18}/></button></header>
 <div class="templates-body">
  <div class="catalog">
   <div class="template-tabs" role="group" aria-label={$t('模板分类')}><button class:active={tab==='design'} on:click={()=>setTab('design')}>{$t('内置模板')}</button><button class:active={tab==='mine'} on:click={()=>setTab('mine')}>{$t('我的模板')} · {templates.length}</button></div>
   {#if tab==='design'}
    <div class="template-filters" role="group" aria-label={$t('设计类型')}>{#each categories as [value,label]}<button type="button" class:chosen={category===value} aria-pressed={category===value} on:click={()=>setCategory(value)}>{$t(label)}</button>{/each}</div>
    {#if fetching}<p class="loading"><LoaderCircle size={16} class="spin"/>{$t('准备设计模板')}</p>{:else}
    <fieldset class="template-grid"><legend class="sr-only">{$t('边框样式')}</legend>{#each visible as item(item.id)}<label class="style-card" class:selected={selectedId===item.id}><input type="radio" name="frame-choice" value={item.id} checked={selectedId===item.id} on:change={()=>pickBuiltin(item)}/><img src={item.thumbnail} alt="" loading="lazy" width="360" height="256"/><strong>{$language==='en'?item.englishName:item.name}</strong><span>{$t(item.description)}</span></label>{/each}</fieldset>{/if}
   {:else}
    {#if draft}<form on:submit|preventDefault={save}><label class="field-label" for="template-name">{$t('模板名称')}</label><div class="save-template"><input id="template-name" bind:value={name} maxlength="60" required placeholder={$t('为这个边框取个名字')}/><button class="button primary compact" disabled={busy||!name.trim()}>{#if busy}<LoaderCircle size={14} class="spin"/>{:else}<Plus size={14}/>{/if}{$t('保存模板')}</button></div><p class="field-help">{$t('只保存比例、边框与艺术图形，不包含照片、署名或模型凭据')}</p></form>{/if}
    {#if saved}<p class="template-saved" role="status">{$t('模板已保存，重新打开软件后仍可使用')}</p>{/if}
    {#if fetching}<p class="loading" role="status"><LoaderCircle size={16} class="spin"/>{$t('准备设计模板')}</p>{:else if templates.length}<fieldset class="template-grid"><legend class="sr-only">{$t('我的模板')}</legend>{#each templates as item(item.id)}<article class="saved-card"><label class="style-card" class:selected={selectedId===item.id}><input type="radio" name="frame-choice" value={item.id} checked={selectedId===item.id} on:change={()=>pickSaved(item)}/><img src={savedPreviews[item.id]||item.thumbnail} alt="" loading="lazy" width="360" height="256"/><strong>{item.name}</strong></label><button type="button" class="icon-button delete-template" disabled={busy} aria-label={$t('删除模板 {0}',{0:item.name})} on:click={()=>remove(item.id)}><Trash2 size={13}/></button></article>{/each}</fieldset>{:else}<p class="templates-empty">{$t('还没有边框模板')}</p>{/if}
   {/if}
   {#if problem}<p class="field-error" role="alert">{$t(problem)}</p>{/if}
  </div>
  <section class="style-preview" aria-label={$t('当前照片的样式预览')}>
   {#if candidate&&chosen}
    <div class="preview-heading"><h3>{chosen.name}</h3><button type="button" class="text-button" class:comparing={compare} aria-pressed={compare} disabled={!currentPreview} on:click={()=>compare=!compare}>{$t(compare?'返回新样式':'对比原效果')}</button></div>
    <div class="preview-image" aria-busy={previewPending}>{#if compare&&currentPreview}<img src={currentPreview} alt={$t('应用前的照片效果')}/>{:else if previewData}<img src={previewData} alt={$t('当前照片与所选边框的实际预览')}/>{:else}<LoaderCircle size={20} class="spin"/>{/if}{#if previewPending}<span class="preview-status" role="status">{$t('更新预览…')}</span>{/if}</div>
    {#if !photo}<p class="field-help">{$t('导入照片后可查看实际效果')}</p>{/if}
    <div class="style-controls">
     <label class="layout-check"><input type="checkbox" bind:checked={keepLayout} on:change={layoutChoice}/>{$t('保留当前比例和边框宽度')}</label>
     <details class="template-adjustments"><summary>{$t('调整模板细节')}</summary><div class="template-adjustment-fields">
     <label class="field-label" for="style-margin">{$t('边框比例')} · {candidate.borderPercent}%</label><input id="style-margin" type="range" min="0" max="30" step="0.5" bind:value={candidate.borderPercent}/>
     <div class="style-colors"><label for="style-base">{$t('底色')}</label><input id="style-base" type="color" bind:value={candidate.frameColor}/><input aria-label={$t('边框颜色')} class="hex-input" bind:value={candidate.frameColor} maxlength="7" pattern="#[0-9a-fA-F]{6}"/></div>
     {#if candidate.artwork?.design}
      {#if candidate.artwork.design.film}
       {#if candidate.artwork.design.id!=='scan-instant'}<label class="layout-check"><input type="checkbox" bind:checked={candidate.artwork.design.film.labels}/>{$t('片边标记')}</label>{/if}
       <label class="field-label" for="film-texture">{$t('扫描颗粒')} · {Math.round(candidate.artwork.design.film.texture*100)}%</label><input id="film-texture" type="range" min="0" max="1" step="0.05" bind:value={candidate.artwork.design.film.texture}/>
      {/if}
      <div class="style-colors"><span>{$t('图案配色')}</span>{#each candidate.artwork.design.colors as color,i}<input type="color" aria-label={$t('样式颜色 {0}',{0:i+1})} value={color} on:input={e=>{if(candidate?.artwork?.design)candidate.artwork.design.colors[i]=e.currentTarget.value;}}/>{/each}</div>
      {#if !candidate.artwork.design.film}<div class="palette-shortcuts"><button type="button" on:click={()=>palette(['#303B3E','#8B9291','#D5D7D1'])}>{$t('黑白')}</button><button type="button" on:click={()=>palette(['#315D69','#C27B5E','#D6C3A0'])}>{$t('海岸')}</button><button type="button" on:click={()=>palette(['#755D48','#B88D67','#D5C4A8'])}>{$t('暖调')}</button></div>{/if}
      <label class="field-label" for="style-intensity">{$t('装饰强度')} · {Math.round(candidate.artwork.design.intensity*100)}%</label><input id="style-intensity" type="range" min="0.2" max="1.5" step="0.05" bind:value={candidate.artwork.design.intensity}/>
      <label class="field-label" for="style-scale">{$t(candidate.artwork.design.film?'片边尺寸':'图案尺寸')} · {Math.round((candidate.artwork.design.scale??1)*100)}%</label><input id="style-scale" type="range" min="0.2" max="1" step="0.05" value={candidate.artwork.design.scale??1} on:input={e=>{if(candidate?.artwork?.design)candidate.artwork.design.scale=Number(e.currentTarget.value);}}/>
     {/if}
     {#if candidate.artwork?.frameImage}<p class="field-help">{$t('按实际透明开口贴合照片，原照片始终完整保留。')}</p>{/if}
     </div></details>
     {#if previewError}<p class="field-error" role="alert">{$t(previewError)}</p>{/if}
    </div>
   {:else}<p class="preview-empty">{$t('选择左侧样式，查看实际效果')}</p>{/if}
  </section>
 </div>
 <footer><span>{$t('切换样式只预览，点击应用后才修改当前边框')}</span><button type="button" class="button primary" disabled={!candidate||busy||previewPending||!!previewError} on:click={apply}>{$t('应用所选样式')}</button></footer>
</dialog>
<style>
.sr-only{position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip-path:inset(50%);white-space:nowrap;border:0;}.templates-dialog{width:min(1040px,94vw);max-height:90vh;padding:0;border:1px solid var(--border);border-radius:12px;background:var(--surface);color:var(--ink);box-shadow:0 16px 70px #14131144;}.templates-dialog::backdrop{background:#201e1a55;}header{display:flex;justify-content:space-between;gap:10px;padding:18px 20px;border-bottom:1px solid var(--border);}h2{font-size:17px;font-weight:550;display:flex;align-items:center;gap:8px;}header p{font-size:12px;color:var(--muted);margin-top:6px;}.templates-body{display:grid;grid-template-columns:minmax(280px,1.15fr) minmax(260px,1fr);max-height:calc(90vh - 160px);overflow:hidden;}.catalog{padding:18px;max-height:calc(90vh - 160px);overflow:auto;border-inline-end:1px solid var(--border);min-width:0;}.template-tabs{display:flex;gap:6px;padding:4px;border-radius:8px;background:var(--surface-soft);}.template-tabs button{flex:1;border:0;padding:9px;background:transparent;border-radius:5px;color:var(--muted);font-size:12px;}.template-tabs button.active{background:var(--paper);color:var(--ink);box-shadow:0 1px 3px #2d2a2616;}.template-filters{display:flex;flex-wrap:wrap;gap:6px;margin-block:14px;}.template-filters button,.palette-shortcuts button{border:1px solid var(--border);background:transparent;border-radius:15px;padding:6px 10px;color:var(--muted);font-size:11px;}.template-filters .chosen{background:var(--ink);color:var(--surface);border-color:var(--ink);}.template-grid{border:0;padding:0;display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:10px;margin:14px 0 0;}.style-card{display:flex;flex-direction:column;position:relative;gap:7px;min-width:0;padding:10px;background:var(--paper);border:1px solid var(--border);border-radius:8px;cursor:pointer;}.style-card.selected{border-color:var(--ink);background:var(--surface-soft);box-shadow:inset 0 0 0 1px var(--ink);}.style-card:focus-within{outline:2px solid var(--green);outline-offset:2px;}.style-card input{position:absolute;inset-block-start:12px;inset-inline-start:12px;accent-color:var(--green);}.style-card img{display:block;width:100%;aspect-ratio:1.4;object-fit:contain;background:var(--surface-soft);height:auto;}.style-card strong{font-size:12px;padding-inline-end:14px;}.style-card span{font-size:11px;line-height:1.6;color:var(--muted);}.saved-card{position:relative;}.delete-template{position:absolute;inset-inline-end:6px;inset-block-end:6px;}.save-template{display:flex;gap:8px;}.save-template input{min-width:0;flex:1;}.template-saved{padding:10px;background:var(--surface-soft);font-size:12px;margin-block:12px;}.style-preview{padding:18px;min-width:0;align-self:start;position:static;max-height:calc(90vh - 160px);overflow:auto;background:var(--surface);}.preview-heading{display:flex;align-items:center;justify-content:space-between;gap:10px;margin-bottom:10px;}h3{font-size:13px;font-weight:550;}.preview-image{display:grid;place-items:center;position:relative;min-height:180px;background:var(--surface-soft);border-radius:6px;overflow:hidden;}.preview-image img{display:block;max-width:100%;width:100%;height:auto;max-height:320px;object-fit:contain;}.preview-status{position:absolute;inset-block-end:8px;inset-inline-start:8px;background:var(--surface);border-radius:4px;padding:5px 7px;font-size:11px;color:var(--muted);}.style-controls{display:grid;gap:8px;margin-top:14px;}.layout-check{display:flex;gap:7px;align-items:center;font-size:11px;}.layout-check input{accent-color:var(--green);}.style-colors{display:flex;align-items:center;gap:7px;font-size:11px;}.style-colors input[type=color]{width:30px;height:28px;border:1px solid var(--border);border-radius:5px;padding:2px;background:transparent;}.hex-input{min-width:0;width:100px;}.palette-shortcuts{display:flex;gap:6px;}footer{display:flex;align-items:center;justify-content:space-between;gap:15px;padding:12px 20px;border-top:1px solid var(--border);}footer span{font-size:11px;color:var(--muted);}.loading{display:flex;gap:8px;align-items:center;font-size:12px;margin-top:20px;}.templates-empty,.preview-empty{padding:30px 0;text-align:center;font-size:12px;color:var(--muted);}.comparing{background:var(--surface-soft);border-radius:4px;}
@media(max-width:760px){.templates-body{grid-template-columns:minmax(230px,1fr) minmax(230px,1fr);}.catalog,.style-preview{padding:12px;}.template-grid{gap:8px;}.preview-image img{max-height:240px;}}@media(max-width:580px){.templates-body{grid-template-columns:1fr;overflow:auto;}.catalog,.style-preview{max-height:none;overflow:visible;}.catalog{border:0;}.style-preview{position:static;border-top:1px solid var(--border);}footer span{display:none;}.template-grid{grid-template-columns:repeat(2,minmax(0,1fr));}}
.template-adjustments{font-size:12px;border-top:1px solid var(--border);margin-top:4px;}.template-adjustments>summary{cursor:pointer;padding-block:12px;color:var(--muted);}.template-adjustment-fields{display:grid;gap:8px;padding-bottom:12px;}
</style>
