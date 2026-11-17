<script lang="ts">
 import {onMount,onDestroy} from 'svelte';
 import {invoke} from '@tauri-apps/api/core';
 import {listen,type UnlistenFn} from '@tauri-apps/api/event';
 import {LoaderCircle,RefreshCw,ExternalLink} from '@lucide/svelte';
 import {t} from './i18n';
 import type {PiAuthEvent,PiCatalog,PiPrompt} from './types';
 export let provider:string;
 export let path:string|null=null;
 export let model='';
 export let onready:(ready:boolean)=>void;
 export let onbusy:(busy:boolean)=>void;
 let catalog:PiCatalog|null=null,loading=true,working=false,alive=true;
 let problem='',notice='',search='',sessionId='',host='',deviceCode='',answer='';
 let prompt:PiPrompt|null=null;
 let unlisten:UnlistenFn|undefined;
 let listenerReady:Promise<void>;
 $: options=catalog?.models||[];
 $: filtered=options.filter(m=>(m.name+' '+m.id).toLowerCase().includes(search.toLowerCase()));
 $: selected=options.find(m=>m.id===model);
 $: visible=selected&&!filtered.some(m=>m.id===model)?[selected,...filtered]:filtered;
 $: onready(!!catalog?.connected&&options.some(m=>m.id===model)&&!working&&!loading);
 $: onbusy(working);
 onMount(()=>{
  listenerReady=listen<PiAuthEvent>('pi-auth-event',e=>{
   const v=e.payload;if(!alive||v.sessionId!==sessionId)return;
   if(v.host)host=v.host;if(v.deviceCode)deviceCode=v.deviceCode;
   if(v.phase==='prompt'){prompt=v.prompt;answer=prompt?.options[0]?.id||'';}
   if(v.phase==='waiting')prompt=null;
  }).then(stop=>{if(alive)unlisten=stop;else stop();}).catch(e=>{if(alive)problem=String(e);});
  void refresh();
 });
 onDestroy(()=>{alive=false;unlisten?.();onready(false);onbusy(false);if(sessionId)void invoke('cancel_pi_connection',{sessionId});});
 function apply(v:PiCatalog){catalog=v;if(!v.models.some(m=>m.id===model))model=v.models.find(m=>m.id==='gpt-6.1-sol')?.id||v.models[0]?.id||'';}
 async function refresh(){loading=true;problem='';try{const v=await invoke<PiCatalog>('list_pi_models',{path,provider,refresh:false});if(alive)apply(v);}catch(e){if(alive)problem=String(e);}finally{if(alive)loading=false;}}
 async function connect(){
  if(working)return;working=true;problem='';notice='';host='';deviceCode='';prompt=null;sessionId=crypto.randomUUID();
  try{await listenerReady;const v=await invoke<PiCatalog>('connect_pi_subscription',{path,provider,sessionId});if(alive){apply(v);notice=$t('账户授权已保存到本机 Pi');}}
  catch(e){if(alive)problem=String(e);}finally{if(alive){working=false;sessionId='';prompt=null;answer='';}}
 }
 async function sendAnswer(){if(!prompt||!sessionId)return;const promptId=prompt.id;try{await invoke('answer_pi_prompt',{sessionId,promptId,value:answer});answer='';prompt=null;}catch(e){problem=String(e);}}
 async function reopen(){try{await invoke('reopen_pi_authorization',{sessionId});}catch(e){problem=String(e);}}
 async function disconnect(){working=true;problem='';notice='';try{const v=await invoke<PiCatalog>('disconnect_pi_subscription',{path,provider});if(alive){apply(v);notice=$t('已退出 Pi 中此服务的本机连接');}}catch(e){if(alive)problem=String(e);}finally{if(alive)working=false;}}
</script>
<section class="pi-connection" aria-busy={working||loading}>
 <p class="connection-status" role="status">{$t(loading?'正在读取 Pi 的模型与授权…':catalog?.connected?(provider==='openrouter'?'已保存账户授权 · 使用账户余额':'已保存订阅授权 · 请求时由 Pi 刷新'):catalog?.authType==='api_key'?'Pi 中保存的是 API Key；此订阅入口不会使用它':'尚未连接此服务的订阅')}</p>
 <p class="field-help">{$t('与本机 Pi 共用授权。这里登录或退出，也会影响 Pi 中同一服务的连接。')}</p>
 <div class="pi-actions">
  <button type="button" class="button secondary compact" disabled={working||loading} on:click={connect}>{$t(provider==='openrouter'?(catalog?.connected?'重新登录账户':'登录并连接账户'):(catalog?.connected?'重新登录订阅':'登录并连接订阅'))}</button>
  <button type="button" class="text-button" disabled={working||loading} on:click={refresh}><RefreshCw size={13}/>{$t('检查授权与模型')}</button>
  {#if catalog?.connected}<button type="button" class="text-button" disabled={working} on:click={disconnect}>{$t('退出 Pi 中此服务')}</button>{/if}
 </div>
 {#if working&&sessionId}
  <div class="pi-login" role="status"><LoaderCircle size={15} class="spin"/><span>{$t('等待服务商授权，不会发送照片。')}</span><button type="button" class="text-button" on:click={()=>invoke('cancel_pi_connection',{sessionId})}>{$t('取消登录')}</button></div>
  {#if host}<div class="auth-page"><span>{$t('官方授权页面')} · {host}</span><button type="button" class="text-button" on:click={reopen}><ExternalLink size={12}/>{$t('在浏览器打开')}</button></div>{/if}
  {#if deviceCode}<p class="device-code">{$t('设备授权码')} <code>{deviceCode}</code></p>{/if}
  {#if prompt}<div class="auth-prompt">
   <label class="field-label" for="pi-auth-answer">{prompt.message}</label>
   {#if prompt.kind==='select'}<select id="pi-auth-answer" name="pi-auth-answer" bind:value={answer}>{#each prompt.options as o}<option value={o.id}>{o.label}</option>{/each}</select>
   {:else}<input id="pi-auth-answer" name="pi-auth-answer" class="full-width" type={prompt.kind==='secret'||prompt.kind==='manual_code'?'password':'text'} bind:value={answer} placeholder={prompt.placeholder} maxlength="32768" autocomplete="one-time-code" autocapitalize="none" spellcheck="false"/>{/if}
   <p class="field-help">{$t('这里只粘贴授权码或回调链接；账户密码请在服务商官网输入。')}</p>
   <button type="button" class="button secondary compact" disabled={!answer} on:click={sendAnswer}>{$t('继续授权')}</button>
  </div>{/if}
 {/if}
 {#if options.length}
  <label class="field-label model-field" for="pi-model-search">{$t('搜索支持图片输入的模型')}</label>
  <input id="pi-model-search" name="pi-model-search" class="full-width" type="search" bind:value={search} placeholder={$t('输入模型名称或 ID')} disabled={working}/>
  <label class="field-label model-field" for="pi-model">{$t('分析与配框模型')}</label>
  <select id="pi-model" name="pi-model" bind:value={model} disabled={working}>{#each visible as m}<option value={m.id}>{m.name} · {m.id}</option>{/each}</select>
  {#if !filtered.length}<p class="field-help">{$t('没有匹配项，保留当前模型选择。')}</p>{/if}
  <p class="field-help">{$t('Pi 模型目录不代表套餐一定允许调用；实际可用性由服务商确认。')}</p>
 {/if}
 {#if notice}<p class="field-help" role="status">{notice}</p>{/if}
 {#if problem}<p class="field-error" role="alert">{$t(problem)}</p>{/if}
</section>
<style>
.pi-actions{display:flex;flex-wrap:wrap;align-items:center;gap:10px;margin-block:14px;}.pi-login{display:flex;align-items:center;gap:8px;font-size:12px;line-height:1.8;background:#f1eee8;border-radius:7px;padding:12px;}.pi-login span{flex:1;}.auth-page{display:flex;gap:10px;align-items:center;justify-content:space-between;font-size:12px;margin-block:12px;}.auth-prompt{border-inline-start:2px solid #a29886;padding-inline-start:12px;margin-block:14px;}.device-code{font-size:12px;margin-block:14px;}.device-code code{display:inline-block;font-size:20px;letter-spacing:.1em;background:#f2f0ec;padding:8px 14px;user-select:text;}.model-field{margin-block-start:16px;}
</style>
