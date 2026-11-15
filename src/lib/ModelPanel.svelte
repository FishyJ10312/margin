<script lang="ts">
  import {t,language} from './i18n';
 import { onMount,onDestroy,tick } from 'svelte';
 import { invoke } from '@tauri-apps/api/core';
 import { open } from '@tauri-apps/plugin-dialog';
 import ImageConnection from './ImageConnection.svelte';
 import ArtDirection from './ArtDirection.svelte';
 import ChatGPTConnection from './ChatGPTConnection.svelte';
 import {defaultImageConfig,defaultFrameDesign,piProviders} from './types';
 import { X, LoaderCircle, WandSparkles, RotateCcw } from '@lucide/svelte';

 import type { AiConfig, AiPhoto, AiState, FrameSuggestion, Photo, RenderSettings, PreviewResult,GeneratedArt,ChatGptDiagnostic,ChatGptState,PiCatalog } from './types';
 export let photo: Photo;
 export let initialConfig:AiConfig|null=null;
 export let onconfigure:(config:AiConfig)=>void;
 export let initialMode:'design'|'refine'|'image'='design';
 let mode=initialMode;
 let imageTarget:'frame'|'sticker'='frame';
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
 let config: AiConfig = {frameDesign:{...defaultFrameDesign,colors:[...defaultFrameDesign.colors]},image:{...defaultImageConfig},provider:'codex',baseUrl:'',model:'',codexPath:null,styleDirection:'交织线条，少量手绘元素，有层次、有细节、主题鲜明',designDensity:'balanced'};
 const origin=(url:string)=>{try{return new URL(url).origin;}catch{return '';}};
 $: if(!loading&&apiKey&&apiOrigin!==origin(config.baseUrl)){apiKey='';apiOrigin='';}
 let state: AiState | null = null;
 let prepared: AiPhoto | null = null;
 let busy = false, loading = true, consent = false,alive=true;
 let accountBusy=false,chatgptReady=false,chatgptLabel='';
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
 let baseError = '', modelError = '';
 let baseDirty = false, modelDirty = false;
 onMount(() => { dialog.showModal(); void initialize(); });
 async function initialize() {
  loading = true; problem = '';
  try { const [s,p] = await Promise.all([invoke<AiState>('get_ai_state'),invoke<AiPhoto>(mode==='refine'?'prepare_art_refinement':'prepare_ai_photo',{photoId:photo.id,settings:baseSettings})]);if(!alive)return; state=s;const base=initialConfig||s.config;config={...base,frameDesign:{...defaultFrameDesign,...base.frameDesign,colors:[...(base.frameDesign?.colors||defaultFrameDesign.colors)]},image:{...defaultImageConfig,...base.image}};prepared=p;if(mode==='refine'&&!initialConfig){if(imageTask)config.frameDesign.notes=$t('保留当前主题，按所选风格整理图案形状、大小与位置，让照片成为视觉中心');else config.styleDirection=$t('保留当前主题，优化线条弧度、图案大小与位置，整理布局和疏密；让照片成为视觉中心');}if(!imageTask&&config.provider==='pi'){const c=await invoke<PiCatalog>('list_pi_models',{path:config.piPath||null,provider:config.piProvider||'openai',refresh:false});if(!alive)return;piReady=c.connected&&c.models.some(m=>m.id===config.model);} }
  catch (error) { if(alive)problem=String(error); } finally { if(alive)loading=false; }
 }
 function validateField(field:'base'|'model',input:HTMLInputElement,force=false) {
  const dirty=field==='base'?baseDirty:modelDirty;
  const message=(force||dirty)&&!input.validity.valid?(field==='base'?$t("请输入完整的 HTTPS 或本机 HTTP 地址"):$t("请填写支持图片输入的模型名称")):'';
  if(field==='base')baseError=message;else modelError=message;
 }
 async function chooseCodex() {const path=await open({title:$t("选择 Codex 可执行文件"),multiple:false});if(typeof path==='string')config.codexPath=path;}
 async function changeMode(next:typeof mode){if(busy||loading||accountBusy)return;mode=next;consent=false;reference=false;refined=false;suggestions=[];previews={};await initialize();}
 onDestroy(()=>{alive=false;if(busy)void invoke('cancel_art_frames');});
 async function generate() {
  if (!prepared || !consent || busy || accountBusy || !imageTask&&(config.provider==='chatgpt'&&!chatgptReady||config.provider==='pi'&&!piReady)) return;
  busy=true; problem='';chatgptDiagnostic=null; suggestions=[];previews={};
  try {
   config={...config,language:$language};
   await invoke('save_ai_config',{config});
   if(!alive)return;
   if(imageTask){
    const result=await invoke<GeneratedArt>(completeFrame?'generate_complete_frame':'generate_handdrawn',{token:prepared.token,config,reference:completeFrame||reference,apiKey:config.image.provider==='openai'?imageApiKey:''});
    if(completeFrame){
     const g=prepared.geometry;if(!g?.photoBounds)throw $t('画布信息已更新，请重新打开智能配框');const b=g.photoBounds;
     suggestions=[{title:$t(mode==='refine'?'艺术边框精修':'整幅艺术边框'),rationale:result.summary.slice(0,200),frameColor:baseSettings.frameColor,borderPercent:baseSettings.borderPercent,decoration:{...baseSettings.decoration,kind:'none'},artwork:{elements:[],frameImage:{imagePath:result.imagePath,opacity:1,reference:{width:1000,height:1000,x:b.x,y:b.y,photoWidth:b.width,photoHeight:b.height}}}}];
    }else{
     suggestions=[{title:$t('手绘艺术边框'),rationale:result.summary.slice(0,200),frameColor:baseSettings.frameColor,borderPercent:baseSettings.borderPercent,decoration:baseSettings.decoration,artwork:{...baseSettings.artwork,elements:baseSettings.artwork?.elements||[],stickers:[{imagePath:result.imagePath,anchor:'bottom-left',scale:0.9,opacity:1,flipX:false}]}}];
    }
    if(mode==='refine')refined=true;
   }else{suggestions=await invoke<FrameSuggestion[]>('generate_art_frames',{token:prepared.token,config,apiKey:config.provider==='api'?apiKey:''});if(mode==='refine')refined=true;}
   if(!alive)return;
   for(let i=0;i<suggestions.length;i++){const s=suggestions[i];const result=await invoke<PreviewResult>('get_preview',{photoId:photo.id,settings:{...baseSettings,...s,watermark:{...baseSettings.watermark,kind:'none'}}});if(!alive)return;previews={...previews,[i]:result.dataUrl};}
   await tick();if(alive&&(mode==='refine'||completeFrame))(mode==='refine'?refinementComparison:resultContainer)?.scrollIntoView({block:'start',behavior:window.matchMedia('(prefers-reduced-motion: reduce)').matches?'auto':'smooth'});
  } catch(error) {if(!alive)return;problem=String(error);if(!imageTask&&config.provider==='chatgpt'){try{chatgptDiagnostic=(await invoke<ChatGptState>('get_chatgpt_state')).diagnostic??null;}catch{/* Keep the request failure. */}}} finally {if(alive)busy=false;}
 }
 async function close() { if(busy) {await invoke('cancel_art_frames');return;} if(accountBusy)await invoke('cancel_chatgpt_sign_in');onclose(); }
</script>
<dialog bind:this={dialog} class="model-dialog" on:cancel|preventDefault={close} aria-labelledby="art-title">
 <header><div><h2 id="art-title">{$t("智能配框")}</h2><p>{$t('由你选择风格、布局与配色')}</p></div><button class="icon-button" aria-label={busy?$t("取消模型请求"):$t("关闭智能配框")} on:click={close}><X size={18} /></button></header>
 <div class="model-body">
  <div class="creative-tabs" role="group" aria-label={$t('创作方式')}>
   <button class:active={mode==='image'} disabled={busy||loading||accountBusy} on:click={()=>changeMode('image')}>{$t('艺术边框')}</button>
   <button class:active={mode==='design'} disabled={busy||loading||accountBusy} on:click={()=>changeMode('design')}>{$t('线条与几何')}</button>
   <button class:active={mode==='refine'} disabled={busy||loading||accountBusy||!baseSettings.artwork} on:click={()=>changeMode('refine')}>{$t('精修一次')}</button>
  </div>
  {#if loading}<p class="model-loading"><LoaderCircle size={17} class="spin" />{$t("准备照片与本机连接…")}</p>{:else}
  <div class="analysis-photo">{#if prepared}<img src={prepared.dataUrl} alt={$t("将交给模型分析的照片缩略图")} /><div><strong>{photo.name}</strong><p>{prepared.width} × {prepared.height} {$t("px · 去除原始元数据")}</p><p>{$t(mode==='design'?"每次分析一张，生成后自行选择方案":mode==='refine'?"一轮精修只返回一套方案，先预览再应用":completeFrame?'分析这张照片，生成一幅完整艺术边框':'每次只生成一张素材，参考照片默认不发送')}</p><p>{$t("同时传入当前画布与留白信息，保留你选定的比例")}</p></div>{/if}</div>
  {#if mode==='refine'&&prepared?.before&&!suggestions.length}<div class="refine-before"><img src={prepared.before} alt={$t('发送给模型的当前配框预览，不含水印')}/><p>{$t(imageTask?'根据当前边框重新绘制图案形状、大小与布局；本轮会使用一次生图额度。':'可调整线条形状、图案大小与位置，重新安排布局。PNG 素材可移动、缩放和翻转。')}</p></div>{/if}
  {#if imageTask}<p class="creative-note">{$t(completeFrame?'按选定的设计语言、装饰位置和配色生成完整边框。先预览再应用，也可以保存为模板。':'生成一组独立手绘素材，叠加到现有边框；可调整大小、位置和透明度。')}</p>{/if}
  {#if mode==='image'}<fieldset class="image-target"><legend>{$t('生成内容')}</legend><label><input type="radio" name="image-target" value="frame" bind:group={imageTarget} disabled={busy} on:change={()=>{consent=false;suggestions=[];previews={};}}/><span><strong>{$t('整幅艺术边框')}</strong><small>{$t('匹配照片，四周形成完整设计')}</small></span></label><label><input type="radio" name="image-target" value="sticker" bind:group={imageTarget} disabled={busy} on:change={()=>{consent=false;suggestions=[];previews={};}}/><span><strong>{$t('独立手绘素材')}</strong><small>{$t('给现有边框添加一组装饰')}</small></span></label></fieldset>{/if}
  <form aria-label={$t("艺术边框生成设置")} on:submit|preventDefault={generate}>
   <fieldset disabled={busy||refined}><legend class="sr-only">{$t("艺术边框生成设置")}</legend>
    {#if completeFrame}<ArtDirection bind:value={config.frameDesign} oninvalidate={()=>consent=false}/>{/if}
    <label class="field-label" for="style-direction">{$t(mode==='refine'?'这次想怎么调整？':completeFrame?'补充要求（可选）':'想要什么设计风格？')}</label>
    {#if !completeFrame}<div class="style-directions">{#if mode==='refine'}{#each [['更简约','减少重复装饰，收拢主装饰，保留更多有意的空白'],['线条更流动','重新设计曲线弧度与交织节奏，线条舒展，主次清晰'],['缩小图案','缩小图案并调整间距，保留主题与精致细节'],['重排布局','重新安排主装饰与呼应区域，改变位置和疏密，保持简约']] as preset}<button type="button" on:click={()=>config.styleDirection=$t(preset[1])}>{$t(preset[0])}</button>{/each}{:else}{#each [$t("交织线条"),$t("童趣卡通"),$t("植物手绘"),$t("海洋小鱼"),$t("几何艺术")] as direction}<button type="button" on:click={()=>config.styleDirection=direction+$t('，呼应照片内容与构图，完整插画、交织线条和细节形成主次，装饰只在留白')}>{direction}</button>{/each}{/if}</div>{/if}
    {#if completeFrame}<textarea id="style-direction" class="full-width" rows="2" maxlength="1500" bind:value={config.frameDesign.notes} required={config.frameDesign.style==='custom'} placeholder={$t('例如：保留海水的蓝，少量橙色点缀，线条清晰，不画海滩风景')} aria-label={$t('自定义艺术边框方向')}></textarea>{:else}
    <textarea id="style-direction" class="full-width" rows="3" maxlength="1500" bind:value={config.styleDirection} placeholder={$t(mode==='refine'?'例如：把主装饰移到左下，缩小图案，让交织线更舒展':'如：蓝橙细线交织，角落有小猫、星星，像旅行手账')} aria-label={$t("自定义艺术边框方向")}></textarea>{/if}
    {#if mode==='design'||completeFrame}<div class="field-title"><label class="field-label" for="design-density">{$t("装饰密度")}</label></div><select id="design-density" bind:value={config.designDensity}><option value="simple">{$t("简约 · 少量点缀")}</option><option value="balanced">{$t("适中 · 线条与图案")}</option><option value="rich">{$t("丰富 · 更多交织与插画")}</option></select>{/if}
    {#if imageTask}
     <div class="connection-summary"><div><strong>{$t(completeFrame?'艺术边框生图':'手绘生图')} · {config.image.provider==='codex'?'Codex':'GPT Image API'}</strong></div><button type="button" class="text-button" disabled={busy||accountBusy} on:click={()=>onconfigure(config)}>{$t('模型配置')}</button></div>
     <details class="advanced-connection"><summary>{$t('临时高级连接')}</summary>
     <ImageConnection bind:value={config.image} bind:apiKey={imageApiKey} bind:keyOrigin={imageApiOrigin} recommendedModels={state?.imageModels||[defaultImageConfig.model]} codexPath={config.codexPath||state?.codexPath||null} loggedIn={state?.codexLoggedIn||false} onchoose={chooseCodex} onreset={()=>{config.codexPath=null;}} oninvalidate={()=>{consent=false;}}/>
     </details>
    {:else}
    <div class="connection-summary"><div><strong>{connectionName}</strong><small>{config.model||$t('使用默认模型')}</small></div><button type="button" class="text-button" disabled={busy||accountBusy} on:click={()=>onconfigure(config)}>{$t('模型配置')}</button></div>
    {#if config.provider==='pi'}<p class="field-help">{$t(piReady?'已保存 Pi 连接；确认后才会发送缩略图。':'请在模型配置中连接订阅并选择支持图片输入的模型。')}</p>
    {:else}<details class="advanced-connection"><summary>{$t('临时高级连接')}</summary>
    <label class="field-label model-field" for="model-provider">{$t("使用的模型入口")}</label>
    <select id="model-provider" name="model-provider" disabled={accountBusy} bind:value={config.provider} on:change={() => {consent=false;problem='';}}><option value="chatgpt">{$t('ChatGPT 账户 · 无需 API Key')}</option><option value="codex">{$t("本机 Codex · 使用现有登录")}</option><option value="api">{$t("OpenAI 兼容视觉 API")}</option></select>
    {#if config.provider==='chatgpt'}
     <ChatGPTConnection bind:accountId={config.chatgptAccountId} bind:model={config.model} oninvalidate={()=>consent=false} onready={ready=>chatgptReady=ready} onbusy={value=>accountBusy=value} onlabel={value=>chatgptLabel=value} onproblem={value=>connectionProblem=value}/>
    {:else if config.provider==='codex'}
     <p class="connection-status">{state?.codexPath?(state.codexLoggedIn?$t("检测到本机 Codex 登录"):$t("检测到 Codex，请先在 Codex 中登录")):$t("尚未找到 Codex，可在高级连接中选择程序")}</p>
     <p class="field-help">{$t("交由本机 Codex 调用当前账户处理，会使用 Codex 额度。")}</p>
     <details class="advanced-watermark"><summary>{$t("高级连接")}</summary><label class="field-label" for="codex-model">{$t("模型名称（留空使用 Codex 默认模型）")}</label><input id="codex-model" class="full-width" bind:value={config.model} maxlength="200" placeholder={$t("可选")} /><button type="button" class="file-button" on:click={chooseCodex}><span>{config.codexPath || state?.codexPath || $t("选择 Codex 可执行文件")}</span></button>{#if config.codexPath}<button type="button" class="text-button" on:click={() => config.codexPath=null}>{$t("恢复自动检测")}</button>{/if}</details>
    {:else}
     <label class="field-label model-field" for="api-base">Base URL</label>
     <p id="base-help" class="field-help">{$t("例如 https://你的服务商/v1；本机服务可使用 localhost。")}</p>
     <input id="api-base" class="full-width" type="url" required bind:value={config.baseUrl} aria-invalid={!!baseError} aria-describedby={baseError?'base-help base-error':'base-help'} on:input={(event)=>{baseDirty=true;if(baseError)validateField('base',event.currentTarget);}} on:blur={(event)=>validateField('base',event.currentTarget)} on:invalid={(event)=>validateField('base',event.currentTarget,true)} />
     {#if baseError}<p id="base-error" class="field-error">{baseError}</p>{/if}
     <label class="field-label model-field" for="api-model">{$t("视觉模型名称")}</label><input id="api-model" class="full-width" required bind:value={config.model} maxlength="200" aria-invalid={!!modelError} aria-describedby={modelError?'model-error':undefined} on:input={(event)=>{modelDirty=true;if(modelError)validateField('model',event.currentTarget);}} on:blur={(event)=>validateField('model',event.currentTarget)} on:invalid={(event)=>validateField('model',event.currentTarget,true)} />
     {#if modelError}<p id="model-error" class="field-error">{modelError}</p>{/if}
     <label class="field-label model-field" for="api-key">API Key</label><p id="key-help" class="field-help">{$t("仅在本次打开期间保留；本机服务可留空。")}</p><input id="api-key" class="full-width" type="password" bind:value={apiKey} autocomplete="new-password" spellcheck="false" aria-describedby="key-help" on:input={()=>apiOrigin=origin(config.baseUrl)}/>
     {#if apiKey}<button type="button" class="text-button" on:click={()=>apiKey=''}>{$t("清除 Key")}</button>{/if}
    {/if}
    </details>{/if}
    {/if}
    {#if imageTask&&!completeFrame}<label class="model-consent"><input type="checkbox" bind:checked={reference} on:change={()=>consent=false}/><span>{$t('将上方照片作为风格参考（可选，默认不发送）')}</span></label>{/if}
    <label class="model-consent"><input type="checkbox" bind:checked={consent} disabled={accountBusy}/><span>{completeFrame?$t(mode==='refine'?'同意发送照片与当前边框缩略图，生成一幅精修边框并使用 {0} 的生图额度':'同意发送这张照片的缩略图，生成一幅艺术边框并使用 {0} 的生图额度',{0:config.image.provider==='codex'?'Codex':config.image.model+' · '+config.image.baseUrl}):imageTask&&config.image.provider==='openai'?$t('同意向 {0} 发送设计方向{1}，使用 {2} 生成一张透明图片并消耗该 API 额度',{0:config.image.baseUrl,1:reference?$t('与参考缩略图'):'',2:config.image.model}):imageTask?$t(reference?'同意发送方向与参考缩略图，生成一张图片并使用 Codex 额度':'同意只发送设计方向，生成一张图片并使用 Codex 额度'):config.provider==='pi'?$t(mode==='refine'?'同意通过 {0} 发送原图与配框缩略图，使用 {1} 调用 {2} 精修一轮':'同意通过 {0} 发送此缩略图，使用 {1} 调用 {2} 分析配框',{0:connectionName,1:$t(piProviders.find(p=>p.id===(config.piProvider||'openai'))?.billing||'订阅用量'),2:config.model}):config.provider==='chatgpt'?$t(mode==='refine'?'同意向 OpenAI 发送原图与配框缩略图，使用 {0} 的 ChatGPT 授权用量调用 {1} 精修一轮':'同意向 OpenAI 发送此缩略图，使用 {0} 的 ChatGPT 授权用量调用 {1} 分析配框',{0:chatgptLabel||$t('当前账户'),1:config.model||$t('所选模型')}):mode==='refine'?$t('同意发送原图缩略图与当前配框预览，调用模型一轮并使用所选入口的额度'):config.provider==='codex'?$t("同意将这张缩略图交给当前登录的 Codex 处理，并使用其额度"):$t("同意向 {0} 发送这张缩略图，并使用我的 API 额度", {0:config.baseUrl || '所填接口'})}</span></label>
    <button class="button primary full-width" type="submit" disabled={!consent || !prepared || busy||refined||accountBusy||!imageTask&&(config.provider==='chatgpt'&&!chatgptReady||config.provider==='pi'&&!piReady)}><WandSparkles size={15} />{$t(mode==='refine'?'精修一次':completeFrame?'生成整幅艺术边框':imageTask?'生成一张手绘素材':'生成三套边框建议')}</button>
   </fieldset>
  </form>
  {/if}
  {#if refined}<p class="creative-note" role="status">{$t('本轮精修完成，选择应用后可撤销。')}</p>{/if}
  {#if mode==='refine'&&suggestions.length&&prepared?.before}<div class="refine-comparison" bind:this={refinementComparison} aria-label={$t('精修前后对比')}><figure><figcaption>{$t('精修前')}</figcaption><img src={prepared.before} alt={$t('精修前的完整配框')} style:aspect-ratio={previewAspect} /></figure><figure><figcaption>{$t('精修后')}</figcaption>{#if previews[0]}<img src={previews[0]} alt={$t('精修后的完整配框')} style:aspect-ratio={previewAspect} />{:else}<div class="comparison-loading" role="status"><LoaderCircle size={18} class="spin"/>{$t('准备精修效果…')}</div>{/if}</figure></div>{/if}
  {#if busy}<progress aria-label={$t('模型正在处理')}></progress><div class="model-loading" role="status"><LoaderCircle size={17} class="spin" /><span>{$t(imageTask?'正在绘制艺术边框，通常需要一至三分钟…':'正在分析照片，完成后可预览和选择…')}</span><button class="text-button" on:click={()=>invoke('cancel_art_frames')}>{$t("取消")}</button></div>{/if}
  {#if problem}<p class="model-problem" role="alert">{$t(problem)}</p>{#if !prepared && !loading}<button class="text-button" on:click={initialize}><RotateCcw size={13} />{$t("重新准备")}</button>{/if}{/if}
  {#if !imageTask&&config.provider==='chatgpt'&&connectionProblem}<p class="model-problem" role="alert">{$t(connectionProblem)}</p><button class="text-button" on:click={()=>onconfigure(config)}>{$t('在模型配置中检查连接')}</button>{/if}
  {#if problem&&!imageTask&&config.provider==='chatgpt'}<button class="text-button" on:click={()=>onconfigure(config)}>{$t('在模型配置中检查连接')}</button>{/if}
  {#if problem&&chatgptDiagnostic}<details class="advanced-connection"><summary>{$t('连接诊断')}</summary><code>HTTP {chatgptDiagnostic.status} {chatgptDiagnostic.bodyShape} {chatgptDiagnostic.code||''} {chatgptDiagnostic.param||''} {chatgptDiagnostic.requestId||''}</code></details>{/if}
  {#if suggestions.length}<div class="art-suggestions" class:refine-result={mode==='refine'} class:complete-result={completeFrame} bind:this={resultContainer}>{#each suggestions as suggestion,i}<article>{#if mode!=='refine'}<div class="art-image">{#if previews[i]}<img src={previews[i]} alt={$t("{0} 的实际边框预览（不含水印）", {0:suggestion.title})}/>{:else}<LoaderCircle size={17} class="spin" />{/if}</div>{/if}<div><h3>{suggestion.title}</h3><p>{suggestion.rationale}</p><button class="button secondary compact" on:click={()=>onapply(suggestion)}>{$t("应用到")} {count} {$t("张照片",{count})}</button><button class="text-button" on:click={()=>onsavetemplate(suggestion)}>{$t("保存为边框模板")}</button></div></article>{/each}</div>{/if}
 </div>
 <footer>{$t(imageTask&&config.image.provider==='openai'?'使用所填图像 API 的额度；取消后仍可能计费。没有自动重试或切换服务。':imageTask?'使用 Codex 生图额度；取消后已发出的任务仍可能计费。没有自动重试。':'模型提供设计参数，边框在本机绘制；生成后仍可手动调整')}</footer>
</dialog>

<style>
.sr-only{position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip-path:inset(50%);white-space:nowrap;border:0;}
.image-target{display:grid;grid-template-columns:1fr 1fr;gap:10px;border:0;padding:0;margin-block:18px;}.image-target legend{font-size:12px;margin-bottom:8px;color:#617454;}.image-target label{display:flex;align-items:start;gap:8px;padding:12px;border:1px solid #d5dfcc;border-radius:7px;background:#fff;cursor:pointer;}.image-target label:has(input:checked){background:#edf2e7;border-color:#7d9474;}.image-target input{accent-color:#436c52;}.image-target strong{display:block;font-size:12px;}.image-target small{display:block;font-size:11px;line-height:1.6;color:#748369;margin-top:4px;}.model-body{container-type:inline-size;}.art-suggestions.complete-result article{grid-template-columns:1fr;}.art-suggestions.complete-result .art-image img{max-height:440px;}.art-suggestions.refine-result article{grid-template-columns:1fr;}.refine-comparison{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:12px;margin-block:18px;}.refine-comparison figure{margin:0;min-width:0;}.refine-comparison figcaption{font-size:12px;color:#617454;margin-block-end:8px;}.refine-comparison img{display:block;width:100%;max-height:360px;object-fit:contain;background:#eef1e9;border-radius:5px;}.comparison-loading{display:grid;place-content:center;gap:8px;min-height:150px;font-size:12px;color:#617454;}@container(max-width:420px){.refine-comparison{grid-template-columns:minmax(0,1fr);}}
.connection-summary{display:flex;align-items:center;justify-content:space-between;gap:12px;margin-block:18px 8px;padding:12px;background:#edf2e7;border:1px solid #dce5d3;border-radius:7px;font-size:12px;}.connection-summary small{display:block;color:#748369;margin-block-start:5px;overflow-wrap:anywhere;}.advanced-connection{font-size:11px;color:#7d8974;margin-block:10px;}.advanced-connection>summary{cursor:pointer;}
.creative-tabs{display:flex;gap:5px;padding:4px;background:#edf1e8;border-radius:8px;margin-bottom:18px;}.creative-tabs button{flex:1;border:0;border-radius:5px;padding:9px 5px;background:transparent;color:#6e7b61;font-size:12px;}.creative-tabs button.active{background:#fff;box-shadow:0 1px 3px #26351e14;color:#3f5b33;}.creative-note{font-size:12px;line-height:1.8;background:#edf2e7;border-radius:7px;padding:12px;margin:10px 0;}.refine-before{display:grid;grid-template-columns:130px 1fr;gap:15px;align-items:center;margin:15px 0;}.refine-before img{width:130px;aspect-ratio:1;object-fit:contain;}.refine-before p{font-size:12px;line-height:1.8;}.model-body>progress{display:block;width:100%;height:5px;margin-top:18px;accent-color:#7c9369;}

.art-image{display:grid;place-items:center;min-height:90px;background:#f2f4ed;}.art-image img{display:block;width:100%;max-height:150px;object-fit:contain;}
.style-directions{display:flex;flex-wrap:wrap;gap:6px;margin:8px 0;}.style-directions button{font-size:11px;border:1px solid #d5dfcc;border-radius:20px;background:#f2f6ec;color:#617454;padding:5px 9px;}
textarea{font:12px/1.8 inherit;resize:vertical;min-height:72px;border:1px solid #d5dfcc;border-radius:6px;padding:9px;background:#fff;color:#45563b;}
</style>
