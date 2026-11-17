<script lang="ts">
  import {t,language} from './i18n';
 import { onMount,onDestroy,tick } from 'svelte';
 import { invoke } from '@tauri-apps/api/core';
 import ArtDirection from './ArtDirection.svelte';
 import {defaultImageConfig,defaultFrameDesign,piProviders} from './types';
 import { X, LoaderCircle, WandSparkles, RotateCcw } from '@lucide/svelte';

 import type { AiConfig, AiPhoto, AiState, FrameSuggestion, Photo, RenderSettings, PreviewResult,GeneratedArt,ChatGptDiagnostic,ChatGptState,ChatGptModel,PiCatalog } from './types';
 export let photo: Photo;
 export let initialConfig:AiConfig|null=null;
 export let onconfigure:(config:AiConfig)=>void;
 export let initialMode:'design'|'refine'|'image'='image';
 let mode=initialMode;
 export let imageTarget:'frame'|'sticker'='frame';
 $: imageTask=mode==='image'||mode==='refine'&&!!baseSettings.artwork?.frameImage;
 $: completeFrame=imageTask&&(mode==='refine'||imageTarget==='frame');
 let reference=false,refined=false;
 export let count: number;
 export let baseSettings:RenderSettings;
 export let apiKey = '';
 export let apiOrigin='';
 export let imageApiKey='';
 export let imageApiOrigin='';
 export let onapply: (suggestion: FrameSuggestion) => void;
 export let onsavetemplate:(suggestion:FrameSuggestion)=>void;
 export let onclose: () => void;
 let dialog: HTMLDialogElement;
 let config: AiConfig = {frameDesign:{...defaultFrameDesign,colors:[...defaultFrameDesign.colors]},image:{...defaultImageConfig},provider:'codex',baseUrl:'',model:'',codexPath:null,styleDirection:'',designDensity:'auto'};
 const origin=(url:string)=>{try{return new URL(url).origin;}catch{return '';}};
 $: if(!loading&&apiKey&&apiOrigin!==origin(config.baseUrl)){apiKey='';apiOrigin='';}
 $: if(!loading&&imageApiKey&&imageApiOrigin!==origin(config.image.baseUrl)){imageApiKey='';imageApiOrigin='';}
 let prepared: AiPhoto | null = null;
 let busy = false, loading = true, consent = false,alive=true;
 let cancelRequested=false,needsPreparation=false;
 let chatgptReady=false,chatgptLabel='';
 let connectionProblem='';
 let piReady=false;
 $: connectionName=config.provider==='pi'?'Pi · '+(piProviders.find(p=>p.id===(config.piProvider||'openai'))?.name||config.piProvider):config.provider==='chatgpt'?'ChatGPT':config.provider==='api'?$t('兼容视觉 API'):'Codex';
 let chatgptDiagnostic:ChatGptDiagnostic|null=null;
 $: previewAspect=prepared?.geometry?`${prepared.geometry.canvasWidth} / ${prepared.geometry.canvasHeight}`:baseSettings.ratio?`${baseSettings.ratio.width} / ${baseSettings.ratio.height}`:`${photo.width} / ${photo.height}`;
 let problem = '';
 let suggestions: FrameSuggestion[] = [];
 let previews:Record<number,string>={};
 let refinementComparison:HTMLDivElement|undefined;
 let resultContainer:HTMLDivElement|undefined;
 let creativeBrief='',hasInitialized=false;
 $: generationKind=mode==='refine'?'refine':mode==='design'?'design':imageTarget==='frame'?'image':'sticker';
 onMount(() => { dialog.showModal(); void initialize(); });
 async function initialize(draft:AiConfig|null=null) {
  loading=true;problem='';connectionProblem='';chatgptReady=false;piReady=false;
  try {
   const [s,p]=await Promise.all([invoke<AiState>('get_ai_state'),invoke<AiPhoto>(mode==='refine'?'prepare_art_refinement':'prepare_ai_photo',{photoId:photo.id,settings:baseSettings})]);
   if(!alive)return;
   const base=draft||initialConfig||s.config;
   config={...base,frameDesign:{...defaultFrameDesign,...base.frameDesign,colors:[...(base.frameDesign?.colors||defaultFrameDesign.colors)]},image:{...defaultImageConfig,...base.image}};
   // Older style cards must not silently constrain a new photo-led request.
   if(config.frameDesign.style!=='auto'){config.frameDesign.style='auto';config.frameDesign.placement='auto';config.designDensity='auto';}
   prepared=p;
   creativeBrief=completeFrame?config.frameDesign.notes:config.styleDirection;
   if(mode==='refine'&&!hasInitialized&&!initialConfig)creativeBrief=$t('保留当前主题，优化图案形状、大小、位置与布局，让照片成为视觉中心');
   hasInitialized=true;
   if(!imageTask&&config.provider==='pi'){
    const c=await invoke<PiCatalog>('list_pi_models',{path:config.piPath||null,provider:config.piProvider||'openai',refresh:false});
    if(alive)piReady=c.connected&&c.models.some(m=>m.id===config.model);
   }else if(!imageTask&&config.provider==='chatgpt'){
    const c=await invoke<ChatGptState>('get_chatgpt_state');if(!alive)return;
    const account=c.accounts.find(a=>a.id===(config.chatgptAccountId||c.activeId));
    chatgptLabel=account?.label||'';
    if(account?.connected&&account.planEnabled){const models=await invoke<ChatGptModel[]>('list_chatgpt_models',{accountId:account.id});if(alive)chatgptReady=models.some(m=>m.slug===config.model);}
    if(!chatgptReady)connectionProblem=$t('请在模型配置中连接账户并选择可用模型');
   }
  }catch(error){if(alive)problem=String(error);}finally{if(alive)loading=false;}
 }
 function syncBrief(){config.frameDesign.notes=creativeBrief;config.styleDirection=creativeBrief.trim()||$t('根据照片的内容、构图与色彩自主设计，保持清晰的视觉层次与适当留白');}
 function configure(){syncBrief();onconfigure(config);}
 async function changeGeneration(next:string){
  if(busy||loading)return;syncBrief();mode=next==='design'?'design':'image';initialMode=mode;imageTarget=next==='sticker'?'sticker':'frame';
  consent=false;reference=false;refined=false;suggestions=[];previews={};await initialize(config);await tick();if(alive)document.getElementById('style-direction')?.focus();
 }
 onDestroy(()=>{alive=false;if(busy)void invoke('cancel_art_frames');});
 async function cancelGeneration(){
  cancelRequested=true;needsPreparation=true;consent=false;
  await invoke('cancel_art_frames');
 }
 async function generate() {
  if (!prepared || !consent || busy || !imageTask&&(config.provider==='chatgpt'&&!chatgptReady||config.provider==='pi'&&!piReady)) return;
  busy=true;cancelRequested=false; problem='';chatgptDiagnostic=null; suggestions=[];previews={};
  try {
   if(needsPreparation){
    const next=await invoke<AiPhoto>(mode==='refine'?'prepare_art_refinement':'prepare_ai_photo',{photoId:photo.id,settings:baseSettings});
    if(!alive||cancelRequested)return;
    prepared=next;needsPreparation=false;
   }
   syncBrief();config={...config,language:$language};
   await invoke('save_ai_config',{config});
   if(!alive||cancelRequested)return;
   if(imageTask){
    const result=await invoke<GeneratedArt>(completeFrame?'generate_complete_frame':'generate_handdrawn',{token:prepared.token,config,reference:completeFrame||reference,apiKey:config.image.provider==='openai'?imageApiKey:''});
    if(!alive||cancelRequested)return;
    if(completeFrame){
     const g=prepared.geometry;if(!g?.photoBounds)throw $t('画布信息已更新，请重新打开智能配框');const b=g.photoBounds;
     suggestions=[{title:$t(mode==='refine'?'艺术边框精修':'整幅艺术边框'),rationale:result.summary.slice(0,200),frameColor:baseSettings.frameColor,borderPercent:baseSettings.borderPercent,decoration:{...baseSettings.decoration,kind:'none'},artwork:{elements:[],frameImage:{imagePath:result.imagePath,opacity:1,reference:{width:1000,height:1000,x:b.x,y:b.y,photoWidth:b.width,photoHeight:b.height}}}}];
    }else{
     suggestions=[{title:$t('手绘艺术边框'),rationale:result.summary.slice(0,200),frameColor:baseSettings.frameColor,borderPercent:baseSettings.borderPercent,decoration:baseSettings.decoration,artwork:{...baseSettings.artwork,elements:baseSettings.artwork?.elements||[],stickers:[{imagePath:result.imagePath,anchor:'bottom-left',scale:0.9,opacity:1,flipX:false}]}}];
    }
    if(mode==='refine')refined=true;
   }else{const result=await invoke<FrameSuggestion[]>('generate_art_frames',{token:prepared.token,config,apiKey:config.provider==='api'?apiKey:''});if(!alive||cancelRequested)return;suggestions=result;if(mode==='refine')refined=true;}
   if(!alive||cancelRequested)return;
   for(let i=0;i<suggestions.length;i++){const s=suggestions[i];const result=await invoke<PreviewResult>('get_preview',{photoId:photo.id,settings:{...baseSettings,...s,watermark:{...baseSettings.watermark,kind:'none'}}});if(!alive||cancelRequested)return;previews={...previews,[i]:result.dataUrl};}
   await tick();if(alive&&!cancelRequested&&(mode==='refine'||completeFrame))(mode==='refine'?refinementComparison:resultContainer)?.scrollIntoView({block:'start',behavior:window.matchMedia('(prefers-reduced-motion: reduce)').matches?'auto':'smooth'});
  } catch(error) {if(!alive||cancelRequested)return;problem=String(error);if(!imageTask&&config.provider==='chatgpt'){try{chatgptDiagnostic=(await invoke<ChatGptState>('get_chatgpt_state')).diagnostic??null;}catch{/* Keep the request failure. */}}} finally {if(alive){if(cancelRequested){suggestions=[];previews={};}busy=false;}}
 }
 async function close() { if(busy) {await cancelGeneration();return;}onclose(); }
</script>
<dialog bind:this={dialog} class="model-dialog" on:cancel|preventDefault={close} aria-labelledby="art-title">
 <header><div><h2 id="art-title">{$t(mode==='refine'?'精修边框':'AI 设计')}</h2><p>{$t(mode==='refine'?'描述想调整的地方，先看效果再应用':'从照片出发，设计一个属于它的边框')}</p></div><button class="icon-button" aria-label={busy?$t("取消模型请求"):$t('关闭 AI 设计')} on:click={close}><X size={18} /></button></header>
 <div class="model-body">
  {#if loading}<p class="model-loading"><LoaderCircle size={17} class="spin" />{$t("准备照片与本机连接…")}</p>{:else}
  <div class="analysis-photo">{#if prepared}<img src={prepared.dataUrl} width="80" height="64" alt={$t("将交给模型分析的照片缩略图")}/><div><strong>{photo.name}</strong><p>{$t(mode==='refine'?'参考原照片和当前边框，调整后先预览':'保留照片与画布比例，设计只出现在留白区域')}</p></div>{/if}</div>
  {#if mode==='refine'&&prepared?.before&&!suggestions.length}<div class="refine-before"><img src={prepared.before} alt={$t('发送给模型的当前配框预览，不含水印')}/><p>{$t(imageTask?'根据当前边框重新绘制图案形状、大小与布局；本轮会使用一次生图额度。':'可调整线条形状、图案大小与位置，重新安排布局。PNG 素材可移动、缩放和翻转。')}</p></div>{/if}
  <form aria-label={$t("艺术边框生成设置")} on:submit|preventDefault={generate}>
   <fieldset disabled={busy||refined}><legend class="sr-only">{$t("艺术边框生成设置")}</legend>
    <label class="field-label" for="style-direction">{$t(mode==='refine'?'这次想怎么调整？':'你的想法（可选）')}</label>
    <textarea id="style-direction" class="full-width" name="design-brief" rows="3" maxlength="1500" bind:value={creativeBrief} on:input={()=>consent=false} placeholder={$t(mode==='refine'?'例如：把主装饰移到左下，缩小图案，让交织线更舒展':'可以留空，让 AI 根据照片设计；也可以写下元素、气氛或不想要的内容')} aria-describedby="brief-help"></textarea>
    <p id="brief-help" class="field-help">{$t('无需先选风格。你的描述优先，未指定的部分交给 AI。')}</p>
    <details class="generation-options"><summary>{$t('更多选项')}</summary><div class="generation-fields">
     {#if mode!=='refine'}<label class="field-label" for="generation-method">{$t('生成方式')}</label><select id="generation-method" value={generationKind} on:change={event=>changeGeneration(event.currentTarget.value)}><option value="image">{$t('完整边框')}</option><option value="design">{$t('可编辑线条与图形')}</option><option value="sticker">{$t('独立装饰素材')}</option></select>{/if}
     {#if completeFrame}<ArtDirection bind:value={config.frameDesign} oninvalidate={()=>consent=false}/>{/if}
     {#if mode==='design'||completeFrame}<label class="field-label" for="design-density">{$t('装饰密度')}</label><select id="design-density" bind:value={config.designDensity} on:change={()=>consent=false}><option value="auto">{$t('由 AI 根据照片决定')}</option><option value="simple">{$t('简约 · 少量点缀')}</option><option value="balanced">{$t('适中 · 线条与图案')}</option><option value="rich">{$t('丰富 · 更多交织与插画')}</option></select>{/if}
     {#if imageTask&&!completeFrame}<label class="model-consent"><input type="checkbox" bind:checked={reference} on:change={()=>consent=false}/><span>{$t('将上方照片作为风格参考（可选，默认不发送）')}</span></label>{/if}
    </div></details>
    <div class="connection-summary"><div><strong>{imageTask?(config.image.provider==='codex'?'Codex':config.image.model):connectionName}</strong><small>{$t(imageTask?'图像生成':config.model||'使用默认模型')}</small></div><button type="button" class="text-button" disabled={busy} on:click={configure}>{$t('更换模型')}</button></div>
    {#if !imageTask&&(config.provider==='pi'&&!piReady||config.provider==='chatgpt'&&!chatgptReady)}<p class="field-help">{$t('请在模型配置中连接账户并选择可用模型')}</p>{/if}
    <label class="model-consent"><input type="checkbox" bind:checked={consent}/><span>{completeFrame?$t(mode==='refine'?'同意发送照片与当前边框缩略图，生成一幅精修边框并使用 {0} 的生图额度':'同意发送这张照片的缩略图，生成一幅艺术边框并使用 {0} 的生图额度',{0:config.image.provider==='codex'?'Codex':config.image.model+' · '+config.image.baseUrl}):imageTask&&config.image.provider==='openai'?$t('同意向 {0} 发送设计方向{1}，使用 {2} 生成一张透明图片并消耗该 API 额度',{0:config.image.baseUrl,1:reference?$t('与参考缩略图'):'',2:config.image.model}):imageTask?$t(reference?'同意发送方向与参考缩略图，生成一张图片并使用 Codex 额度':'同意只发送设计方向，生成一张图片并使用 Codex 额度'):config.provider==='pi'?$t(mode==='refine'?'同意通过 {0} 发送原图与配框缩略图，使用 {1} 调用 {2} 精修一轮':'同意通过 {0} 发送此缩略图，使用 {1} 调用 {2} 分析配框',{0:connectionName,1:$t(piProviders.find(p=>p.id===(config.piProvider||'openai'))?.billing||'订阅用量'),2:config.model}):config.provider==='chatgpt'?$t(mode==='refine'?'同意向 OpenAI 发送原图与配框缩略图，使用 {0} 的 ChatGPT 授权用量调用 {1} 精修一轮':'同意向 OpenAI 发送此缩略图，使用 {0} 的 ChatGPT 授权用量调用 {1} 分析配框',{0:chatgptLabel||$t('当前账户'),1:config.model||$t('所选模型')}):mode==='refine'?$t('同意发送原图缩略图与当前配框预览，调用模型一轮并使用所选入口的额度'):config.provider==='codex'?$t("同意将这张缩略图交给当前登录的 Codex 处理，并使用其额度"):$t("同意向 {0} 发送这张缩略图，并使用我的 API 额度", {0:config.baseUrl || '所填接口'})}</span></label>
    <button class="button primary full-width" type="submit" disabled={!consent || !prepared || busy||refined||!imageTask&&(config.provider==='chatgpt'&&!chatgptReady||config.provider==='pi'&&!piReady)}><WandSparkles size={15} />{$t(mode==='refine'?'精修一次':completeFrame?'生成边框':imageTask?'生成装饰素材':'生成三套边框建议')}</button>
   </fieldset>
  </form>
  {/if}
  {#if refined}<p class="creative-note" role="status">{$t('本轮精修完成，选择应用后可撤销。')}</p>{/if}
  {#if mode==='refine'&&suggestions.length&&prepared?.before}<div class="refine-comparison" bind:this={refinementComparison} aria-label={$t('精修前后对比')}><figure><figcaption>{$t('精修前')}</figcaption><img src={prepared.before} alt={$t('精修前的完整配框')} style:aspect-ratio={previewAspect} /></figure><figure><figcaption>{$t('精修后')}</figcaption>{#if previews[0]}<img src={previews[0]} alt={$t('精修后的完整配框')} style:aspect-ratio={previewAspect} />{:else}<div class="comparison-loading" role="status"><LoaderCircle size={18} class="spin"/>{$t('准备精修效果…')}</div>{/if}</figure></div>{/if}
  {#if busy}<progress aria-label={$t('模型正在处理')}></progress><div class="model-loading" role="status"><LoaderCircle size={17} class="spin" /><span>{$t(imageTask?'正在绘制艺术边框，通常需要一至三分钟…':'正在分析照片，完成后可预览和选择…')}</span><button class="text-button" on:click={cancelGeneration}>{$t("取消")}</button></div>{/if}
  {#if problem}<p class="model-problem" role="alert">{$t(problem)}</p>{#if !prepared && !loading}<button class="text-button" on:click={()=>initialize()}><RotateCcw size={13} />{$t("重新准备")}</button>{/if}{/if}
  {#if !imageTask&&config.provider==='chatgpt'&&connectionProblem}<p class="model-problem" role="alert">{$t(connectionProblem)}</p><button class="text-button" on:click={configure}>{$t('在模型配置中检查连接')}</button>{/if}
  {#if problem&&!imageTask&&config.provider==='chatgpt'}<button class="text-button" on:click={configure}>{$t('在模型配置中检查连接')}</button>{/if}
  {#if problem&&chatgptDiagnostic}<details class="advanced-connection"><summary>{$t('连接诊断')}</summary><code>HTTP {chatgptDiagnostic.status} {chatgptDiagnostic.bodyShape} {chatgptDiagnostic.code||''} {chatgptDiagnostic.param||''} {chatgptDiagnostic.requestId||''}</code></details>{/if}
  {#if suggestions.length}<div class="art-suggestions" class:refine-result={mode==='refine'} class:complete-result={completeFrame} bind:this={resultContainer}>{#each suggestions as suggestion,i}<article>{#if mode!=='refine'}<div class="art-image">{#if previews[i]}<img src={previews[i]} alt={$t("{0} 的实际边框预览（不含水印）", {0:suggestion.title})}/>{:else}<LoaderCircle size={17} class="spin" />{/if}</div>{/if}<div><h3>{suggestion.title}</h3><p>{suggestion.rationale}</p><button class="button secondary compact" on:click={()=>onapply(suggestion)}>{$t("应用到")} {count} {$t("张照片",{count})}</button><button class="text-button" on:click={()=>onsavetemplate(suggestion)}>{$t("保存为边框模板")}</button></div></article>{/each}</div>{/if}
 </div>
 <footer>{$t(imageTask&&config.image.provider==='openai'?'使用所填图像 API 的额度；取消后仍可能计费。没有自动重试或切换服务。':imageTask?'使用 Codex 生图额度；取消后已发出的任务仍可能计费。没有自动重试。':'模型提供设计参数，边框在本机绘制；生成后仍可手动调整')}</footer>
</dialog>

<style>
.sr-only{position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip-path:inset(50%);white-space:nowrap;border:0;}
.model-dialog[open]{display:grid;grid-template-rows:auto minmax(0,1fr) auto;overflow:hidden;}.model-dialog>.model-body{min-height:0;max-height:none;overscroll-behavior:contain;}
.model-body{container-type:inline-size;}.art-suggestions.complete-result article{grid-template-columns:1fr;}.art-suggestions.complete-result .art-image img{max-height:440px;}.art-suggestions.refine-result article{grid-template-columns:1fr;}.refine-comparison{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:12px;margin-block:18px;}.refine-comparison figure{margin:0;min-width:0;}.refine-comparison figcaption{font-size:12px;color:#617454;margin-block-end:8px;}.refine-comparison img{display:block;width:100%;max-height:360px;object-fit:contain;background:#eef1e9;border-radius:5px;}.comparison-loading{display:grid;place-content:center;gap:8px;min-height:150px;font-size:12px;color:#617454;}@container(max-width:420px){.refine-comparison{grid-template-columns:minmax(0,1fr);}}
.connection-summary{display:flex;align-items:center;justify-content:space-between;gap:12px;margin-block:18px 8px;padding:12px;background:#edf2e7;border:1px solid #dce5d3;border-radius:7px;font-size:12px;}.connection-summary small{display:block;color:#748369;margin-block-start:5px;overflow-wrap:anywhere;}.advanced-connection{font-size:11px;color:#7d8974;margin-block:10px;}.advanced-connection>summary{cursor:pointer;}
.creative-note{font-size:12px;line-height:1.8;background:#edf2e7;border-radius:7px;padding:12px;margin:10px 0;}.refine-before{display:grid;grid-template-columns:130px 1fr;gap:15px;align-items:center;margin:15px 0;}.refine-before img{width:130px;aspect-ratio:1;object-fit:contain;}.refine-before p{font-size:12px;line-height:1.8;}.model-body>progress{display:block;width:100%;height:5px;margin-top:18px;accent-color:#7c9369;}

.art-image{display:grid;place-items:center;min-height:90px;background:#f2f4ed;}.art-image img{display:block;width:100%;max-height:150px;object-fit:contain;}
textarea{font:12px/1.8 inherit;resize:vertical;min-height:72px;border:1px solid #d5dfcc;border-radius:6px;padding:9px;background:#fff;color:#45563b;}
.generation-options{border-block:1px solid var(--border);margin-block:12px;font-size:12px;}.generation-options>summary{cursor:pointer;padding-block:12px;color:var(--muted);}.generation-fields{display:grid;gap:8px;padding-block:4px 16px;}.analysis-photo{margin-block-end:16px;gap:12px;}.analysis-photo img{width:64px;height:64px;}.analysis-photo strong{font-size:13px;}.model-dialog textarea{font-family:inherit;font-size:13px;line-height:1.8;min-height:88px;}.model-dialog .connection-summary{padding:0;border:0;background:transparent;margin-block:12px;}.model-consent{margin-block:12px;}.model-dialog :is(input,textarea,select):focus-visible{outline:2px solid var(--green);outline-offset:2px;}
</style>
