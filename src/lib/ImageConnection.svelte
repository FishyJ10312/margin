<script lang="ts">
 import {invoke} from '@tauri-apps/api/core';
 import {onDestroy} from 'svelte';
 import {Eye,EyeOff,RefreshCw} from '@lucide/svelte';
 import {t} from './i18n';
 import type {ImageConfig} from './types';
 export let value:ImageConfig;
 export let apiKey='';
 export let keyOrigin='';
 function origin(url:string){try{return new URL(url).origin;}catch{return '';}}
 $: if(apiKey&&keyOrigin!==origin(value.baseUrl)){apiKey='';keyOrigin='';}
 export let recommendedModels:string[]=[];
 export let codexPath:string|null=null;
 export let loggedIn=false;
 export let onchoose:()=>void;
 export let onreset:()=>void;
 export let oninvalidate:()=>void;
 let remoteModels:string[]|null=null,listing=false,manual=false,reveal=false;
 let status='',problem='',active=true;
 $: modelOptions=Array.from(new Set([...(remoteModels??recommendedModels),value.model].filter(Boolean)));
 onDestroy(()=>{active=false;});
 function changed(){status='';problem='';oninvalidate();}
 function providerChanged(){remoteModels=null;manual=false;reveal=false;changed();}
 function selectModel(event:Event){const selected=(event.currentTarget as HTMLSelectElement).value;manual=selected==='__custom__';if(!manual)value={...value,model:selected};changed();}
 async function loadModels(){
  if(listing)return;listing=true;status='';problem='';
  try{const models=await invoke<string[]>('list_image_models',{config:value,apiKey});if(active){remoteModels=models;status=$t('已读取 {0} 个 GPT 图像模型',{0:models.length});}}
  catch(e){if(active)problem=String(e);}finally{if(active)listing=false;}
 }
</script>
<section class="image-connection" aria-labelledby="image-connection-title">
 <h3 id="image-connection-title">{$t('生图入口')}</h3>
 <label class="field-label" for="image-provider">{$t('生成服务')}</label>
 <select id="image-provider" name="image-provider" bind:value={value.provider} disabled={listing} on:change={providerChanged}>
  <option value="codex">{$t('本机 Codex · 使用现有登录')}</option>
  <option value="openai">{$t('GPT Image API · 自选模型')}</option>
 </select>
 {#if value.provider==='codex'}
  <p class="connection-status">{$t(codexPath?(loggedIn?'检测到本机 Codex 登录':'检测到 Codex，请先在 Codex 中登录'):'尚未找到 Codex，可在高级连接中选择程序')}</p>
  <p class="field-help">{$t('图像模型由 Codex 内置工具管理；此入口使用 Codex 额度。')}</p>
  <details class="advanced-watermark"><summary>{$t('高级连接')}</summary>
   <label class="field-label" for="image-codex-model">{$t('Codex 推理模型（可选，不是图像模型 ID）')}</label>
   <input id="image-codex-model" name="image-codex-model" class="full-width" bind:value={value.codexModel} maxlength="200" autocapitalize="none" spellcheck="false" on:input={changed}/>
   <button type="button" class="file-button" on:click={onchoose}><span>{codexPath||$t('选择 Codex 可执行文件')}</span></button>
   <button type="button" class="text-button" on:click={onreset}>{$t('恢复自动检测')}</button>
  </details>
 {:else}
  <p class="image-billing">{$t('直接调用所填图像 API，使用该 API 项目的额度；不会使用本机 Codex 登录。')}</p>
  <label class="field-label model-field" for="image-base-url">Base URL</label>
  <input id="image-base-url" name="image-base-url" class="full-width" type="url" required maxlength="2048" bind:value={value.baseUrl} readonly={listing} autocapitalize="none" autocorrect="off" spellcheck="false" aria-describedby="image-base-help" on:input={()=>{remoteModels=null;changed();}}/>
  <p id="image-base-help" class="field-help">{$t('默认 OpenAI 官方地址；兼容服务需支持 Images API 和透明 PNG。')}</p>
  <label class="field-label model-field" for="image-api-key">API Key</label>
  <div class="image-key"><input id="image-api-key" name="image-api-key" class="full-width" type={reveal?'text':'password'} bind:value={apiKey} required={origin(value.baseUrl)==='https://api.openai.com'} readonly={listing} autocomplete="new-password" autocapitalize="none" autocorrect="off" spellcheck="false" aria-describedby="image-key-help" on:input={()=>{keyOrigin=origin(value.baseUrl);changed();}}/><button type="button" class="icon-button" aria-pressed={reveal} aria-label={$t(reveal?'隐藏 API Key':'显示 API Key')} on:click={()=>reveal=!reveal}>{#if reveal}<EyeOff size={15}/>{:else}<Eye size={15}/>{/if}</button></div>
  <p id="image-key-help" class="field-help">{$t('只保留在本次软件会话内，不写入设置或模板；OpenAI 官方接口必填。')}</p>
  {#if apiKey}<button type="button" class="text-button" disabled={listing} on:click={()=>{apiKey='';changed();}}>{$t('清除 Key')}</button>{/if}
  <div class="field-title model-field"><label class="field-label" for="image-model">{$t('图像模型')}</label><button type="button" class="text-button" disabled={listing} on:click={loadModels}><RefreshCw size={12} class={listing?'spin':''}/>{$t(listing?'读取中…':'读取可用模型')}</button></div>
  <select id="image-model" name="image-model" value={manual?'__custom__':value.model} disabled={listing} on:change={selectModel}>
   {#each modelOptions as model}<option value={model}>{model}{remoteModels&&!remoteModels.includes(model)?$t("（当前选择，接口未列出）"):""}</option>{/each}<option value="__custom__">{$t('手动填写模型 ID')}</option>
  </select>
  {#if manual}<label class="field-label model-field" for="image-custom-model">{$t('自定义图像模型 ID')}</label><input id="image-custom-model" name="image-custom-model" class="full-width" bind:value={value.model} required maxlength="200" pattern="[A-Za-z0-9._:\/\-]+" readonly={listing} autocapitalize="none" autocorrect="off" spellcheck="false" on:input={changed}/>{/if}
  <p class="field-help">{$t('Flare 适合快速生成；Sunburst 偏向精细编辑。选择模型不会自动发起生图请求。')}</p>
  {#if status}<p class="field-help" role="status">{status}</p>{/if}
  {#if problem}<p class="field-error" role="alert">{$t(problem)}</p>{/if}
  <div class="image-options"><div><label class="field-label" for="image-quality">{$t('生成质量')}</label><select id="image-quality" name="image-quality" bind:value={value.quality} on:change={changed}><option value="low">{$t('低 · 更快')}</option><option value="medium">{$t('中 · 均衡')}</option><option value="high">{$t('高 · 更多细节')}</option></select></div><div><label class="field-label" for="image-size">{$t('素材画布')}</label><select id="image-size" name="image-size" bind:value={value.size} on:change={changed}><option value="1024x1024">1024 × 1024</option><option value="1536x1024">1536 × 1024</option><option value="1024x1536">1024 × 1536</option></select></div></div>
 {/if}
</section>
<style>
.image-connection{margin-block:18px 10px;padding-block-start:14px;border-top:1px solid #e2ddd5;}h3{font-size:13px;font-weight:550;margin-block-end:12px;}.image-key{display:flex;align-items:center;gap:6px;}.image-key input{min-width:0;flex:1;}.image-options{display:grid;grid-template-columns:1fr 1fr;gap:12px;margin-block-start:14px;}.image-options>div{min-width:0;}.image-options select{width:100%;}.image-billing{font-size:11px;line-height:1.8;background:#f1eee8;padding:10px;border-radius:6px;margin-block:10px;}
</style>
