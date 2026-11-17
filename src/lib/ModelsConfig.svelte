<script lang="ts">
 import {onMount,onDestroy} from 'svelte';
 import {invoke} from '@tauri-apps/api/core';
 import {open} from '@tauri-apps/plugin-dialog';
 import {X,LoaderCircle,SlidersHorizontal,Check} from '@lucide/svelte';
 import ChatGPTConnection from './ChatGPTConnection.svelte';
 import PiConnection from './PiConnection.svelte';
 import ImageConnection from './ImageConnection.svelte';
 import {defaultAiConfig,defaultImageConfig,piProviders,type AiConfig,type AiState,type PiRuntimeInfo} from './types';
 import {t,language} from './i18n';
 export let initialConfig:AiConfig|null=null;
 export let apiKey='';export let apiOrigin='';export let imageApiKey='';export let imageApiOrigin='';
 export let onclose:()=>void;
 export let onsaved:(config:AiConfig)=>void;
 let dialog:HTMLDialogElement;
 let draft:AiConfig=structuredClone(defaultAiConfig),saved:AiConfig=structuredClone(defaultAiConfig);
 let state:AiState|null=null,runtime:PiRuntimeInfo|null=null;
 let pane='chatgpt',model='',loading=true,busy=false,saving=false,ready=false,alive=true,runtimeLoading=false;
 let problem='',notice='';
 const origin=(url:string)=>{try{return new URL(url).origin;}catch{return '';}};
 $: if(!loading&&apiKey&&apiOrigin!==origin(draft.baseUrl)){apiKey='';apiOrigin='';}
 $: selectedPi=piProviders.find(p=>p.id!=='openai'&&pane==='pi:'+p.id);
 $: title=selectedPi?.name||(pane==='chatgpt'?'ChatGPT':pane==='codex'?'本机 Codex':pane==='api'?'兼容视觉 API':'手绘图像生成');
 $: canSave=pane==='codex'||pane==='api'||pane==='image'||ready;
 $: activeAnalysis=saved.provider==='pi'?(saved.piProvider==='openai'?'ChatGPT':(piProviders.find(p=>p.id===saved.piProvider)?.name||saved.piProvider)):saved.provider==='chatgpt'?'ChatGPT':saved.provider==='api'?$t('兼容视觉 API'):'Codex';
 onMount(()=>{dialog.showModal();void initialize();});
 onDestroy(()=>{alive=false;});
 async function initialize(){try{state=await invoke<AiState>('get_ai_state');if(!alive)return;const base=initialConfig||state.config;draft={...defaultAiConfig,...base,image:{...defaultImageConfig,...base.image}};if(draft.provider==='pi'&&(draft.piProvider||'openai')==='openai')draft.provider='chatgpt';saved=structuredClone(draft);select('chatgpt');}catch(e){problem=String(e);}finally{if(alive)loading=false;}}
 function select(value:string){if(busy||saving)return;pane=value;ready=false;notice='';problem='';model=(value==='pi:'+draft.piProvider&&draft.provider==='pi'||value===draft.provider)?draft.model:'';if(value.startsWith('pi:')&&!runtime&&!runtimeLoading)void loadPi();}
 async function loadPi(){runtimeLoading=true;try{const result=await invoke<PiRuntimeInfo>('get_pi_runtime',{path:draft.piPath||null});if(alive)runtime=result;}catch(e){if(alive)problem=String(e);}finally{if(alive)runtimeLoading=false;}}
 async function choosePi(){const path=await open({title:$t('选择 Pi 程序'),multiple:false});if(typeof path!=='string')return;draft.piPath=path;await loadPi();}
 async function autoPi(){draft.piPath=null;await loadPi();}
 async function chooseCodex(){const path=await open({title:$t('选择 Codex 可执行文件'),multiple:false});if(typeof path==='string')draft.codexPath=path;}
 async function save(){
  if(saving||busy||!canSave)return;saving=true;problem='';notice='';
  try{
   const next=structuredClone(draft);next.language=$language;
   if(selectedPi){next.provider='pi';next.piProvider=selectedPi.id;next.model=model;}
   else if(pane==='chatgpt'){next.provider='chatgpt';next.model=model;}
   else if(pane==='codex'){next.provider='codex';next.model=model;}
   else if(pane==='api'){next.provider='api';next.model=model;}
   await invoke('save_ai_config',{config:next});if(!alive)return;draft=next;saved=structuredClone(next);onsaved(structuredClone(next));notice=$t(pane==='image'?'图像生成设置已保存':'照片分析模型已保存');
  }catch(e){if(alive)problem=String(e);}finally{if(alive)saving=false;}
 }
</script>
<dialog class="models-config" bind:this={dialog} aria-labelledby="models-config-title" on:close={onclose}>
 <header><div><h2 id="models-config-title"><SlidersHorizontal size={18}/>{$t('模型配置')}</h2><p>{$t('连接自己的订阅，选择照片分析与图像生成模型')}</p></div><!-- svelte-ignore a11y_autofocus (User-opened native dialog starts on its safe close action.) --><button type="button" class="icon-button" autofocus aria-label={$t('关闭模型配置')} on:click={()=>dialog.close()}><X size={18}/></button></header>
 {#if loading}<div class="settings-loading" role="status"><LoaderCircle size={18} class="spin"/>{$t('读取模型配置…')}</div>
 {:else}
 <div class="models-layout">
  <nav aria-label={$t('模型服务')}>
   <button type="button" class:active={pane==='chatgpt'} aria-pressed={pane==='chatgpt'} disabled={busy||saving} on:click={()=>select('chatgpt')}><strong>ChatGPT</strong><span>{$t('官网登录 · 无需 API Key')}</span></button>
   <button type="button" class:active={pane==='image'} aria-pressed={pane==='image'} disabled={busy||saving} on:click={()=>select('image')}><strong>{$t('图像生成')}</strong><span>{$t('Codex 或图像 API')}</span></button>
   <details class="other-connections"><summary>{$t('其他连接方式')}</summary>
    {#each piProviders.filter(p=>p.id!=='openai') as provider}<button type="button" class:active={pane==='pi:'+provider.id} aria-pressed={pane==='pi:'+provider.id} disabled={busy||saving} on:click={()=>select('pi:'+provider.id)}><strong>{provider.name}</strong><span>{$t(provider.billing)}</span></button>{/each}
    <button type="button" class:active={pane==='codex'} aria-pressed={pane==='codex'} disabled={busy||saving} on:click={()=>select('codex')}><strong>Codex</strong><span>{$t('使用本机登录')}</span></button>
    <button type="button" class:active={pane==='api'} aria-pressed={pane==='api'} disabled={busy||saving} on:click={()=>select('api')}><strong>{$t('兼容视觉 API')}</strong><span>{$t('自己的 API Key')}</span></button>
   </details>
  </nav>
  <div class="models-inspector">
   <div class="model-assignments"><span>{$t('照片分析')} <strong>{activeAnalysis}</strong>{#if saved.model}<small>{saved.model}</small>{/if}</span><span>{$t('手绘生图')} <strong>{saved.image.provider==='codex'?'Codex':'GPT Image API'}</strong></span></div>
   <form id="model-config-form" method="post" aria-labelledby="model-connection-title" on:submit|preventDefault={save}>
    <h3 id="model-connection-title">{$t(title)}</h3>
    {#if selectedPi}
     <p class="field-help">{$t('由本机 Pi 的服务商适配器完成授权、模型目录和请求。连接失败不会切换计费方式。')}</p>
     <div class="runtime-status" hidden={runtimeLoading}><span class:available={runtime?.available} class="runtime-dot"></span><span>{runtime?.available?$t('已检测到 Pi {0}',{0:runtime.version||''}):$t('尚未找到 Pi，需要先安装或选择程序')}</span></div>
     <details><summary>{$t('Pi 程序设置')}</summary><p class="field-help path-label">{draft.piPath||runtime?.path||$t('自动检测')}</p><button type="button" class="text-button" disabled={busy} on:click={choosePi}>{$t('选择 Pi 程序')}</button><button type="button" class="text-button" disabled={busy} on:click={autoPi}>{$t('恢复自动检测')}</button></details>
     {#if runtimeLoading}<p class="connection-status" role="status">{$t('正在检查可选连接组件…')}</p>{:else if runtime?.available}{#key selectedPi.id+String(draft.piPath)}<PiConnection provider={selectedPi.id} path={draft.piPath||null} bind:model onready={v=>ready=v} onbusy={v=>busy=v}/>{/key}{:else}<p class="setup-note">{$t('此高级连接需要本机 Pi。使用 ChatGPT 入口无需安装 Pi 或 Node。')}</p><button type="button" class="text-button" on:click={()=>select('chatgpt')}>{$t('使用 ChatGPT')}</button>{/if}
    {:else if pane==='chatgpt'}
     <ChatGPTConnection bind:accountId={draft.chatgptAccountId} bind:model oninvalidate={()=>{notice='';}} onready={v=>ready=v} onbusy={v=>busy=v} onlabel={()=>{}}/>
    {:else if pane==='codex'}
     <p class="connection-status">{$t(state?.codexLoggedIn?'检测到本机 Codex 登录':'检测到 Codex，请先在 Codex 中登录')}</p>
     <p class="field-help">{$t('分析使用本机 Codex 的登录与额度；手绘生图另有独立设置。')}</p>
     <label class="field-label model-field" for="configured-codex-model">{$t('模型名称（留空使用 Codex 默认模型）')}</label><input id="configured-codex-model" name="configured-codex-model" class="full-width" bind:value={model} maxlength="200" spellcheck="false"/>
     <details><summary>{$t('高级连接')}</summary><p class="field-help path-label">{draft.codexPath||state?.codexPath||$t('尚未找到 Codex')}</p><button type="button" class="text-button" on:click={chooseCodex}>{$t('选择 Codex 可执行文件')}</button><button type="button" class="text-button" on:click={()=>draft.codexPath=null}>{$t('恢复自动检测')}</button></details>
    {:else if pane==='api'}
     <p class="field-help">{$t('使用所填 API 项目的额度；此入口不等同于网站或聊天软件订阅。')}</p>
     <label class="field-label model-field" for="configured-base-url">Base URL</label><input id="configured-base-url" name="configured-base-url" class="full-width" type="url" required bind:value={draft.baseUrl} autocapitalize="none" spellcheck="false"/>
     <label class="field-label model-field" for="configured-api-model">{$t('视觉模型名称')}</label><input id="configured-api-model" name="configured-api-model" class="full-width" required bind:value={model} maxlength="200" autocapitalize="none" spellcheck="false"/>
     <label class="field-label model-field" for="configured-api-key">API Key</label><input id="configured-api-key" name="configured-api-key" class="full-width" type="password" bind:value={apiKey} autocomplete="new-password" spellcheck="false" on:input={()=>apiOrigin=origin(draft.baseUrl)}/>
     <p class="field-help">{$t('Key 只在本次软件会话内保留，不进入模型配置文件或模板。')}</p>
    {:else}
     <p class="field-help">{$t('手绘生图与照片分析分别配置。默认继续使用本机 Codex。')}</p>
     <ImageConnection bind:value={draft.image} bind:apiKey={imageApiKey} bind:keyOrigin={imageApiOrigin} recommendedModels={state?.imageModels||[]} codexPath={draft.codexPath||state?.codexPath||null} loggedIn={state?.codexLoggedIn||false} onchoose={chooseCodex} onreset={()=>draft.codexPath=null} oninvalidate={()=>notice=''}/>
    {/if}
   </form>
   {#if notice}<p class="settings-notice" role="status"><Check size={14}/>{notice}</p>{/if}
   {#if problem}<p class="field-error" role="alert">{$t(problem)}</p>{/if}
  </div>
 </div>
 {/if}
 <footer><p>{$t('保存配置不会发起分析或生图请求。')}</p><div><button type="button" class="button secondary compact" on:click={()=>dialog.close()}>{$t('完成')}</button><button class="button primary compact" type="submit" form="model-config-form" disabled={loading||busy||saving||!canSave}>{#if saving}<LoaderCircle size={14} class="spin"/>{/if}{$t(pane==='image'?'保存图像生成设置':'保存为照片分析模型')}</button></div></footer>
</dialog>
<style>
.other-connections{border-block-start:1px solid var(--border);padding:16px 5px 0;}.other-connections summary{padding:4px 5px;}.models-config{padding:0;width:min(860px,94vw);max-height:88vh;border:1px solid var(--border);border-radius:12px;background:var(--surface);color:var(--ink);box-shadow:0 18px 80px #1e1c1930;}.models-config[open]{display:grid;grid-template-rows:auto minmax(0,1fr) auto;}.models-config:modal{overscroll-behavior:contain;}.models-config::backdrop{background:#211f1c55;overflow:hidden;overscroll-behavior:contain;}header{display:flex;justify-content:space-between;align-items:flex-start;gap:12px;padding:20px 22px;border-block-end:1px solid var(--border);}h2{display:flex;align-items:center;gap:9px;font-size:17px;}header p{font-size:12px;color:var(--muted);margin-block-start:6px;}.models-layout{display:grid;grid-template-columns:205px minmax(0,1fr);min-height:0;}nav{padding:15px 10px;overflow:auto;border-inline-end:1px solid var(--border);background:var(--surface);}nav button{display:block;text-align:start;width:100%;border:1px solid transparent;background:transparent;border-radius:7px;padding:10px;margin-block:3px;color:var(--ink);cursor:pointer;}nav button strong{display:block;font-size:12px;font-weight:550;}nav button span{display:block;margin-block-start:4px;font-size:10px;color:var(--muted);}nav button.active{background:var(--paper);border-color:var(--border);box-shadow:0 1px 3px #27252108;}nav button:focus-visible{outline:2px solid var(--green);outline-offset:1px;}nav button:disabled{opacity:.5;cursor:default;}.models-inspector{padding:20px 24px;overflow:auto;min-width:0;}.model-assignments{display:flex;flex-wrap:wrap;gap:14px;font-size:10px;color:var(--muted);border-block-end:1px solid var(--border);padding-block-end:16px;margin-block-end:18px;}.model-assignments strong{font-weight:550;color:var(--ink);margin-inline:5px;}.model-assignments small{font-size:10px;overflow-wrap:anywhere;}h3{font-size:15px;margin-block-end:10px;}.model-field{margin-block-start:16px;}.runtime-status{display:flex;align-items:center;gap:7px;font-size:12px;margin-block:16px;}.runtime-dot{width:6px;height:6px;border-radius:50%;background:#b5b0a8;}.runtime-dot.available{background:var(--green);}details{font-size:12px;color:var(--muted);margin-block:16px;}summary{cursor:pointer;}details button{margin-block-start:8px;margin-inline-end:12px;}.path-label{overflow-wrap:anywhere;margin-block:9px;}.setup-note{font-size:12px;line-height:1.9;background:var(--surface-soft);border-radius:7px;padding:14px;}.settings-loading{display:flex;align-items:center;justify-content:center;gap:10px;min-height:260px;font-size:13px;}.settings-notice{display:flex;align-items:center;gap:7px;color:var(--ink);font-size:12px;margin-block-start:16px;}footer{padding:14px 20px;border-block-start:1px solid var(--border);display:flex;align-items:center;justify-content:space-between;gap:12px;}footer p{font-size:10px;color:var(--muted);}footer>div{display:flex;gap:8px;}@media(max-width:720px){.models-layout{grid-template-columns:155px minmax(0,1fr)}.models-inspector{padding:16px;}footer{align-items:flex-end;}footer>div{flex-wrap:wrap;justify-content:flex-end;}}@media(pointer:coarse){nav button{min-height:48px;}}
</style>
