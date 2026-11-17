<script lang="ts">
 import {onMount,onDestroy} from 'svelte';
 import {invoke} from '@tauri-apps/api/core';
 import {LoaderCircle,RefreshCw,ExternalLink,CheckCircle2,AlertCircle} from '@lucide/svelte';
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
 let loading=true,working=false,connecting=false,alive=true,verifiedId='';
 let problem='',notice='';
 $: selected=state.accounts.find(a=>a.id===accountId);
 $: verified=!!accountId&&verifiedId===accountId&&models.length>0&&!problem;
 $: ready=verified&&!!selected?.planEnabled&&models.some(m=>m.slug===model)&&!loading&&!working;
 $: needsLogin=!!selected&&(!selected.connected||selected.status==='reconnect'||selected.status==='signedOut');
 $: status=connecting?'请在官网完成登录，Margin 会自动继续':loading||working?'正在验证 ChatGPT 连接…':verified?'已连接 · 可用模型已验证':problem?(needsLogin?'需要重新连接 ChatGPT':'暂时无法验证连接，可重试'):selected?.connected&&!selected.planEnabled?'请授权使用 ChatGPT 套餐额度':selected?'账户已保存，连接尚未验证':'尚未连接 ChatGPT';
 $: onready(ready);
 $: onbusy(working||loading);
 $: onlabel(selected?.label||'');
 $: onproblem(problem);
 onMount(()=>{void initialize();});
 onDestroy(()=>{alive=false;onready(false);onbusy(false);if(connecting)void invoke('cancel_chatgpt_sign_in');});
 function invalidate(){models=[];verifiedId='';notice='';oninvalidate();}
 function applyState(next:ChatGptState){state=next;accountId=next.activeId||next.accounts[0]?.id||null;invalidate();}
 async function fail(error:unknown){
  if(!alive)return;invalidate();problem=String(error);
  try{const next=await invoke<ChatGptState>('get_chatgpt_state');if(alive)state=next;}catch{/* Preserve the original failure, with no success indicator. */}
 }
 async function readModels(){
  const id=accountId;if(!id||!state.accounts.some(a=>a.id===id&&a.connected&&a.planEnabled))return;
  const result=await invoke<ChatGptModel[]>('list_chatgpt_models',{accountId:id});
  if(!alive||accountId!==id)return;
  if(!result.length)throw $t('此账户暂未返回可用模型，请稍后重试');
  models=result;verifiedId=id;
  if(!models.some(m=>m.slug===model))model=models.find(m=>m.slug==='gpt-6.1-sol')?.slug||models[0].slug;
  oninvalidate();
 }
 async function initialize(){
  try{const next=await invoke<ChatGptState>('get_chatgpt_state');if(!alive)return;applyState(next);await readModels();}
  catch(e){await fail(e);}finally{if(alive)loading=false;}
 }
 async function connect(id:string|null=accountId,consent=false){
  if(working||loading)return;working=true;connecting=true;problem='';invalidate();
  try{const next=await invoke<ChatGptState>('chatgpt_sign_in',{accountId:id,consent});if(!alive)return;connecting=false;applyState(next);await readModels();}
  catch(e){await fail(e);}
  finally{if(alive){working=false;connecting=false;}}
 }
 async function choose(){
  if(!accountId||working)return;working=true;problem='';invalidate();
  try{const next=await invoke<ChatGptState>('select_chatgpt_account',{accountId});if(!alive)return;applyState(next);await readModels();}
  catch(e){await fail(e);}finally{if(alive)working=false;}
 }
 async function refresh(){
  if(working||loading)return;working=true;problem='';invalidate();
  try{await readModels();}catch(e){await fail(e);}finally{if(alive)working=false;}
 }
 async function disconnect(){
  if(!accountId||working)return;working=true;problem='';invalidate();
  try{const r=await invoke<{state:ChatGptState;remoteRevoked:boolean}>('disconnect_chatgpt',{accountId});if(!alive)return;applyState(r.state);notice=$t(r.remoteRevoked?'已断开此 ChatGPT 连接':'已清除此设备的授权；远程撤销未确认，请在 ChatGPT 用量设置中断开应用');}
  catch(e){await fail(e);}finally{if(alive)working=false;}
 }
 async function usage(){try{await invoke('open_chatgpt_usage');}catch(e){problem=String(e);}}
</script>
<section class="chatgpt-connection" aria-label="ChatGPT" aria-busy={loading||working}>
 <p class="connection-intro">{$t('使用自己的 ChatGPT 订阅分析照片、设计边框和精修。')}</p>
 <p class="connection-status" class:verified={verified&&!working&&!loading} role="status" aria-live="polite">
  {#if connecting||loading||working}<LoaderCircle size={15} class="spin"/>{:else if verified}<CheckCircle2 size={15}/>{:else if problem}<AlertCircle size={15}/>{/if}
  <span>{$t(status)}</span>
 </p>
 {#if selected}<p class="account-label">{selected.label}</p>{/if}
 {#if !loading}
  <div class="connection-actions">
   {#if connecting}
    <button type="button" class="button secondary compact" on:click={()=>invoke('cancel_chatgpt_sign_in')}>{$t('取消登录')}</button>
   {:else if !selected||needsLogin}
    <button type="button" class="chatgpt-signin" disabled={working} on:click={()=>connect()}>{$t(selected?'重新连接 ChatGPT':'连接 ChatGPT')}</button>
   {:else if !selected.planEnabled}
    <button type="button" class="chatgpt-signin" disabled={working} on:click={()=>connect(accountId,true)}>{$t('授权使用 ChatGPT 套餐额度')}</button>
   {:else}
    <button type="button" class="text-button" disabled={working} on:click={refresh}><RefreshCw size={13}/>{$t(verified?'刷新模型':'重试连接')}</button>
   {/if}
  </div>
  {#if models.length}
   <label class="field-label model-field" for="chatgpt-model">{$t('照片分析与精修模型')}</label>
   <select id="chatgpt-model" name="chatgpt-model" bind:value={model} disabled={working} required aria-describedby="chatgpt-model-help" on:change={oninvalidate}>
    {#each models as item}<option value={item.slug}>{item.displayName}</option>{/each}
   </select>
   <p id="chatgpt-model-help" class="field-help">{$t('可用模型由当前账户返回，使用你的 ChatGPT 授权用量。')}</p>
  {/if}
  {#if problem}<p class="field-error" role="alert">{$t(problem)}</p>{/if}
  {#if notice}<p class="field-help" role="status">{notice}</p>{/if}
  <p class="privacy-note">{$t('无需安装其他软件或填写 API Key。登录不发送照片，生成前会另行确认。')}</p>
  <p class="privacy-note">{$t('登录授权仅保存在此设备，不保存账户密码。')}</p>
  {#if state.accounts.length}
   <details class="account-management">
    <summary>{$t('管理账户')}</summary>
    <label class="field-label model-field" for="chatgpt-account">{$t('当前账户或工作区')}</label>
    <select id="chatgpt-account" name="chatgpt-account" bind:value={accountId} disabled={working} on:change={choose}>
     {#each state.accounts as account}<option value={account.id}>{account.label} · {account.id.slice(0,6)}</option>{/each}
    </select>
    <div class="connection-actions">
     <button type="button" class="text-button" disabled={working} on:click={()=>connect(accountId)}>{$t('重新登录')}</button>
     <button type="button" class="text-button" disabled={working||state.accounts.length>=8} on:click={()=>connect(null)}>{$t('添加其他账户')}</button>
     <button type="button" class="text-button" disabled={working} on:click={disconnect}>{$t('断开此连接')}</button>
    </div>
    <button type="button" class="text-button usage-link" on:click={usage}><ExternalLink size={12}/>{$t('在 ChatGPT 中管理用量与授权')}</button>
    {#if problem&&state.diagnostic}<p class="field-help"><code>HTTP {state.diagnostic.status} {state.diagnostic.code||''} {state.diagnostic.requestId||''}</code></p>{/if}
   </details>
  {/if}
 {/if}
</section>
<style>
.chatgpt-connection{margin-block:12px;color:var(--ink);}.connection-intro{font-size:13px;line-height:1.8;margin-block-end:20px;}.connection-status{display:flex;align-items:center;gap:8px;font-size:12px;line-height:1.8;color:var(--muted);}.connection-status.verified{color:var(--accent,#413d37);}.account-label{font-size:12px;margin-block:8px;overflow-wrap:anywhere;}.connection-actions{display:flex;flex-wrap:wrap;align-items:center;gap:12px;margin-block:14px;}.chatgpt-signin{background:var(--ink);color:var(--surface);border:1px solid var(--ink);border-radius:7px;min-height:44px;padding:10px 18px;font:inherit;font-size:13px;cursor:pointer;}.chatgpt-signin:disabled{opacity:.5;cursor:default;}.chatgpt-signin:focus-visible{outline:3px solid var(--accent,#4b463f);outline-offset:3px;}.model-field{margin-block:20px 8px;}.privacy-note{font-size:11px;line-height:1.9;color:var(--muted);margin-block:6px;}.account-management{margin-block-start:22px;border-block-start:1px solid var(--border);padding-block-start:16px;font-size:12px;color:var(--muted);}.account-management summary{cursor:pointer;}.usage-link{margin-block-start:4px;}@media(pointer:coarse){.chatgpt-signin{min-height:48px;}}
</style>
