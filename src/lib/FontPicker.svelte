<script lang="ts">
  import {t,language} from './i18n';
 import {tick} from 'svelte';
 import type {FontOption} from './types';
 export let fonts:FontOption[]=[];
 export let selected:string|null=null;
 export let onchange:(font:FontOption|null)=>void;
 let search='',filter='chinese',lastLanguage='',list:HTMLDivElement;
 $: if(lastLanguage!==$language){filter=$language==='en'?'all':'chinese';lastLanguage=$language;}
 function displayName(f:FontOption){return f.bundled?/ZCOOL|站酷/.test(f.family)?$t("站酷小薇 · 宋体风格"):f.family.includes('Ma Shan')?$t("马善政 · 毛笔风格"):$t("思源黑体 · 中文"):f.family;}
 $: builtin=fonts.find(f=>f.bundled);
 $: options=fonts.filter(f=>(filter==='all'||f.chinese)&&(f.family+' '+displayName(f)).toLocaleLowerCase().includes(search.toLocaleLowerCase()));
 $: index=selected?options.findIndex(f=>f.family===selected):Math.max(0,options.findIndex(f=>f.family===builtin?.family));
 async function pick(i:number){const f=options[i];if(!f)return;onchange(f);await tick();list?.querySelector(`[data-index="${i}"]`)?.scrollIntoView({block:'nearest'});}
 function key(e:KeyboardEvent){let i=Math.max(0,index);if(e.key==='ArrowDown')i=index<0?0:i+1;else if(e.key==='ArrowUp')i--;else if(e.key==='Home')i=0;else if(e.key==='End')i=options.length-1;else if(e.key==='PageDown')i+=5;else if(e.key==='PageUp')i-=5;else return;e.preventDefault();void pick(Math.max(0,Math.min(options.length-1,i)));}
</script>
<label class="field-label" for="font-search">{$t("字体")} <span class="font-count">{fonts.length} {$t("种可用")}</span></label>
<div class="font-filters" role="group" aria-label={$t("字体范围")}><button class:chosen={filter==='chinese'} aria-pressed={filter==='chinese'} on:click={()=>filter='chinese'}>{$t("中文字体")}</button><button class:chosen={filter==='all'} aria-pressed={filter==='all'} on:click={()=>filter='all'}>{$t("全部字体")}</button></div>
<input id="font-search" class="full-width" type="search" bind:value={search} placeholder={$t("搜索字体名称 / 中文风格")} on:keydown={(e)=>{if(e.key==='ArrowDown'){e.preventDefault();list.focus();}}}/>
<div bind:this={list} class="font-picker" tabindex="0" role="listbox" aria-label={$t("可用字体")} aria-activedescendant={index>=0?`font-choice-${index}`:undefined} on:keydown={key}>
 {#each options as font,i}<button type="button" tabindex="-1" id={`font-choice-${i}`} data-index={i} role="option" aria-selected={index===i} class:chosen={index===i} on:click={()=>{list.focus();void pick(i);}}><span>{$t(displayName(font))}{#if font.bundled}<small>{$t("内置")}</small>{/if}</span>{#if index===i}<span aria-hidden="true">✓</span>{/if}</button>{/each}
 {#if !options.length}<p class="field-help">{$t("没有匹配的字体")}</p>{/if}
</div>
<p class="field-help">{$t("选中后按 ↑ ↓ 切换，水印细节同步预览")}</p>
<style>
.font-picker{height:158px;overflow:auto;border:1px solid #d8dfd2;border-radius:7px;margin-top:7px;background:#fbfcf9;outline-offset:2px;scrollbar-width:thin;}
[role=option]{width:100%;text-align:left;border:0;background:transparent;display:flex;justify-content:space-between;gap:6px;padding:8px 10px;font-size:12px;cursor:pointer;color:#53604b;min-height:34px;}[role=option] small{display:inline-block;margin-left:6px;color:#90a081;font-size:9px;}
[role=option]:hover{background:#eff3e9;}.chosen{background:#e4ecdc;color:#3e5931;font-weight:550;}.font-filters{display:flex;gap:5px;margin:8px 0;}.font-filters button{border:1px solid #d5dfcc;border-radius:5px;padding:4px 8px;font-size:11px;min-height:26px;color:#65775b;}
</style>
