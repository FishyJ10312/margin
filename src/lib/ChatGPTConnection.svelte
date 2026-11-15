<script lang="ts">
 import {onMount,onDestroy} from 'svelte';
 import {invoke} from '@tauri-apps/api/core';
 import {LoaderCircle,RefreshCw,ExternalLink} from '@lucide/svelte';
 import {t} from './i18n';
 import type {ChatGptState,ChatGptModel} from './types';
 export let accountId:string|null=null;
 export let model='';
 export let oninvalidate:()=>void;
 export let onready:(ready:boolean)=>void;
 export let onbusy:(busy:boolean)=>void;
 export let onlabel:(label:string)=>void;
 export let onproblem:(message:string)=>void=()=>{};
 let state:ChatGptState={accounts:[],activeId:null};
 let models:ChatGptModel[]=[];
 let loading=true,working=false,connecting=false,alive=true;
 let problem='',notice='';
 $: selected=state.accounts.find(a=>a.id===accountId);
 $: ready=!!selected?.connected&&!!selected?.planEnabled&&models.some(m=>m.slug===model)&&!loading&&!working;
 $: onready(ready);
 $: onbusy(working);
 $: onlabel(selected?.label||'');
 $: onproblem(problem);
 onMount(()=>{void initialize();});
 onDestroy(()=>{alive=false;onready(false);onbusy(false);if(connecting)void invoke('cancel_chatgpt_sign_in');});
 function applyState(next:ChatGptState){state=next;accountId=next.activeId||next.accounts[0]?.id||null;models=[];oninvalidate();}
 async function fail(error:unknown){
  if(!alive)return;problem=String(error);
  try{const next=await invoke<ChatGptState>('get_chatgpt_state');if(alive){state=next;if(!state.accounts.some(a=>a.id===accountId&&a.connected&&a.planEnabled))models=[];}}catch{/* Preserve the original failure. */}
 }
 async function readModels(){
  const id=accountId;if(!id||!state.accounts.some(a=>a.id===id&&a.connected&&a.planEnabled))return;
  const result=await invoke<ChatGptModel[]>('list_chatgpt_models',{accountId:id});
  if(!alive||accountId!==id)return;
  models=result;if(!models.some(m=>m.slug===model))model=models[0]?.slug||'';oninvalidate();
 }
 async function initialize(){
  try{const next=await invoke<ChatGptState>('get_chatgpt_state');if(!alive)return;applyState(next);await readModels();}
  catch(e){await fail(e);}finally{if(alive)loading=false;}
 }
 async function connect(id:string|null=null,consent=false){
  if(working)return;working=true;connecting=true;problem='';notice='';oninvalidate();
  try{const next=await invoke<ChatGptState>('chatgpt_sign_in',{accountId:id,consent});if(!alive)return;applyState(next);notice=$t('ChatGPT 登录已完成');await readModels();}
  catch(e){await fail(e);}
  finally{if(alive){working=false;connecting=false;}}
 }
 async function choose(){
  if(!accountId||working)return;working=true;problem='';notice='';models=[];oninvalidate();
  try{const next=await invoke<ChatGptState>('select_chatgpt_account',{accountId});if(!alive)return;applyState(next);await readModels();}
  catch(e){await fail(e);}finally{if(alive)working=false;}
 }
 async function refresh(){
  if(working)return;working=true;problem='';models=[];oninvalidate();
  try{await readModels();}catch(e){await fail(e);}finally{if(alive)working=false;}
 }
 async function disconnect(){
  if(!accountId||working)return;working=true;problem='';notice='';oninvalidate();
  try{const r=await invoke<{state:ChatGptState;remoteRevoked:boolean}>('disconnect_chatgpt',{accountId});if(!alive)return;applyState(r.state);notice=$t(r.remoteRevoked?'已断开此 ChatGPT 连接':'已清除此设备的授权；远程撤销未确认，请在 ChatGPT 用量设置中断开应用');}
  catch(e){await fail(e);}finally{if(alive)working=false;}
 }
 async function usage(){try{await invoke('open_chatgpt_usage');}catch(e){problem=String(e);}}
</script>
<section class="chatgpt-connection" aria-labelledby="chatgpt-title" aria-busy={loading||working}>
 <h3 id="chatgpt-title">{$t('我的 ChatGPT 账户')}</h3>
 <p class="field-help">{$t('登录后用于照片分析、参数配框与精修；手绘生图仍使用本机 Codex。')}</p>
 {#if loading}<p class="connection-status" role="status"><LoaderCircle size={14} class="spin"/>{$t('读取连接状态…')}</p>
 {:else}
  {#if state.accounts.length}
   <label class="field-label model-field" for="chatgpt-account">{$t('当前账户或工作区')}</label>
   <select id="chatgpt-account" name="chatgpt-account" bind:value={accountId} disabled={working} on:change={choose}>
    {#each state.accounts as account}<option value={account.id}>{$t(account.label)} · {account.id.slice(0,6)}</option>{/each}
   </select>
   <p class="connection-status">{$t(!selected?.connected?'此连接尚未完成登录':selected.planEnabled?'已连接 · 已授权使用 ChatGPT 套餐额度':'已登录 · 尚未授权使用套餐额度')}</p>
  {/if}
  <div class="connection-actions">
   {#if !selected?.connected}
    <button type="button" class="chatgpt-signin" disabled={working} on:click={()=>connect(accountId)}>Continue with ChatGPT</button>
   {:else if !selected.planEnabled}
    <button type="button" class="chatgpt-signin" disabled={working} on:click={()=>connect(accountId,true)}>{$t('授权使用 ChatGPT 套餐额度')}</button>
   {:else}
    <button type="button" class="text-button" disabled={working} on:click={()=>connect(accountId)}>{$t('重新登录')}</button>
   {/if}
   {#if state.accounts.length}<button type="button" class="text-button" disabled={working||state.accounts.length>=8} on:click={()=>connect()}>{$t('添加其他账户')}</button>{/if}
   {#if selected?.connected}<button type="button" class="text-button" disabled={working} on:click={disconnect}>{$t('断开此连接')}</button>{/if}
  </div>
  {#if connecting}<div class="login-wait" role="status"><LoaderCircle size={15} class="spin"/><span>{$t('请在官方浏览器页面完成登录与授权，此处会自动更新。')}</span><button type="button" class="text-button" on:click={()=>invoke('cancel_chatgpt_sign_in')}>{$t('取消登录')}</button></div>{/if}
  {#if selected?.connected&&selected.planEnabled}
   <div class="field-title model-field"><label class="field-label" for="chatgpt-model">{$t('此账户可用的模型')}</label><button type="button" class="text-button" disabled={working} on:click={refresh}><RefreshCw size={12}/>{$t('刷新模型')}</button></div>
   <select id="chatgpt-model" name="chatgpt-model" bind:value={model} disabled={working||!models.length} required on:change={oninvalidate}>
    {#if !models.length}<option value="">{$t('请先读取可用模型')}</option>{/if}
    {#each models as item}<option value={item.slug}>{item.displayName}</option>{/each}
   </select>
  {/if}
  <p class="field-help">{$t('登录时不发送照片。只有确认下方分析请求后，才会发送展示的缩略图并使用此账户的授权用量。')}</p>
  <p class="field-help">{$t('授权保存在此设备的系统凭据库，不保存账户密码；不会自动改用 API 额度。')}</p>
  <button type="button" class="text-button usage-link" on:click={usage}><ExternalLink size={12}/>{$t('在 ChatGPT 中管理用量与授权')}</button>
 {/if}
 {#if notice}<p class="field-help" role="status">{notice}</p>{/if}
 {#if problem}<p class="field-error" role="alert">{$t(problem)}</p>{/if}
 {#if problem&&state.diagnostic}<p class="field-help"><code>HTTP {state.diagnostic.status} {state.diagnostic.code||''} {state.diagnostic.param||''} {state.diagnostic.requestId||''}</code></p>{/if}
</section>
<style>
.chatgpt-connection{margin-block:12px;padding:14px;background:#f1f4ec;border:1px solid #d8e1d0;border-radius:8px;}h3{font-size:13px;font-weight:550;margin-block:0 8px;}.connection-actions{display:flex;flex-wrap:wrap;align-items:center;gap:8px;margin-block:12px;}.chatgpt-signin{background:#171717;color:white;border:1px solid #171717;border-radius:7px;min-height:42px;padding:10px 16px;font:inherit;font-size:13px;cursor:pointer;}.chatgpt-signin:disabled{opacity:.5;cursor:default;}.chatgpt-signin:focus-visible{outline:3px solid #526b3d;outline-offset:3px;}.login-wait{display:flex;align-items:center;gap:9px;font-size:12px;line-height:1.8;margin-block:12px;}.login-wait span{flex:1;}.usage-link{margin-block-start:6px;}.connection-status{display:flex;align-items:center;gap:7px;line-height:1.8;}@media(pointer:coarse){.chatgpt-signin{min-height:48px;}}
</style>
