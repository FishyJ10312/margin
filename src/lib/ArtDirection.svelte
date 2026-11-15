<script lang="ts">
 import {t} from './i18n';
 import type {FrameDesign} from './types';
 export let value:FrameDesign;
 export let oninvalidate:()=>void=()=>{};
 const styles:[FrameDesign['style'],string,string][]=[['graphic','现代平面','色块、几何与清晰线条'],['editorial','摄影装帧','卡纸、留白与精细线框'],['film','复古胶片','相纸边界与片孔节奏'],['linework','克制线稿','少量轮廓，干净细线'],['collage','纸片拼贴','纸角、色带与轻手账'],['illustration','轻绘插画','主动选择手绘图案'],['custom','自由描述','由你的补充要求定义']];
</script>
<section class="art-directions" aria-label={$t('AI 边框风格')}>
 <fieldset class="look-field"><legend>{$t('先选一种设计语言')}</legend><div class="look-grid">{#each styles as [id,label,description]}<label class="look-card" class:selected={value.style===id}><input type="radio" name="art-look" value={id} bind:group={value.style} on:change={oninvalidate}/>
 <svg viewBox="0 0 120 78" aria-hidden="true"><rect x="1" y="1" width="118" height="76" rx="3" fill={id==='film'?'#293330':'#F4F1E8'}/><rect x="29" y="13" width="62" height="49" fill="#C6D3CC"/>
 {#if id==='graphic'}<path d="M9 13h13v20H9zM98 43h13v18H98z" fill="#527B7D"/><path d="M8 64h29v5H8zM91 6h21v4H91z" fill="#C68C66"/>
 {:else if id==='editorial'}<rect x="24" y="8" width="72" height="59" fill="none" stroke="#78847C" stroke-width=".7"/><path d="M39 71h42" stroke="#A4967B"/>
 {:else if id==='film'}{#each [8,24,40,56,72,88,104] as x}<rect {x} y="5" width="8" height="4" rx="1" fill="#D4C5A6"/><rect {x} y="68" width="8" height="4" rx="1" fill="#D4C5A6"/>{/each}
 {:else if id==='linework'}<path d="M6 68C17 56 12 36 24 28M10 57q-7-11-4-18 11 3 9 12m1-11q-1-13 8-17 2 13-5 18M97 7q13 8 17 23" fill="none" stroke="#678073" stroke-width="1.1"/>
 {:else if id==='collage'}<path d="M4 51l23 4-4 17L3 68z" fill="#93A184"/><path d="M95 7l21-4 2 17-21 2z" fill="#C9A67E"/>
 {:else if id==='illustration'}<path d="M11 70q9-17 8-36m-5 18-9-9m14 0 7-8" fill="none" stroke="#65806E"/>{#each [[10,34],[19,29],[27,34]] as [cx,cy]}<circle {cx} {cy} r="5" fill="#CB9380"/>{/each}<circle cx="106" cy="14" r="7" fill="#D2AE6F"/>
 {:else}<path d="m12 56 8-8m-8 0 8 8m82-38 6 6-6 6-6-6z" fill="none" stroke="#779182" stroke-width="1.5"/>{/if}</svg>
 <strong>{$t(label)}</strong><small>{$t(description)}</small></label>{/each}</div></fieldset>
 <details class="direction-details"><summary>{$t('布局、配色与材质')}</summary>
  <fieldset class="look-field placement"><legend>{$t('装饰放在哪里')}</legend>{#each [['corners','对角点缀'],['sides','四边分布'],['bottom','底部重点']] as [id,label]}<label><input type="radio" name="art-placement" value={id} bind:group={value.placement} on:change={oninvalidate}/>{$t(label)}</label>{/each}</fieldset>
  <label class="field-label" for="art-palette">{$t('配色方式')}</label><select id="art-palette" bind:value={value.palette} on:change={oninvalidate}><option value="photo">{$t('跟随照片 · 低饱和配色')}</option><option value="mono">{$t('黑白与象牙色')}</option><option value="custom">{$t('自定义三色')}</option></select>
  {#if value.palette==='custom'}<div class="direction-colors">{#each value.colors as color,i}<label><span>{$t('颜色 {0}',{0:i+1})}</span><input type="color" value={color} aria-label={$t('自定义颜色 {0}',{0:i+1})} on:input={e=>{value.colors[i]=e.currentTarget.value;oninvalidate();}}/></label>{/each}</div>{/if}
  <label class="surface"><input type="checkbox" bind:checked={value.texture} on:change={oninvalidate}/>{$t('添加轻微纸张纹理')}</label><p class="field-help">{$t('默认使用干净平面质感；选择插画才采用手绘图案。')}</p>
 </details>
</section>
<style>
.art-directions .look-field{border:0;padding:0;margin:0 0 14px;min-width:0;}.look-field legend{font-size:12px;color:#617454;margin-bottom:8px;}.look-grid{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:8px;}.look-card{display:flex;flex-direction:column;position:relative;gap:5px;padding:7px;border:1px solid #d7e1ce;border-radius:7px;background:#fff;cursor:pointer;}.look-card.selected{border-color:#6d875e;box-shadow:inset 0 0 0 1px #6d875e;background:#f0f5e9;}.look-card:focus-within{outline:2px solid #6c865e;outline-offset:2px;}.look-card input{position:absolute;top:10px;left:10px;accent-color:#5a7650;}.look-card svg{display:block;width:100%;height:auto;max-height:72px;}.look-card strong{font-size:12px;font-weight:550;}.look-card small{font-size:10px;line-height:1.5;color:#7c8d71;}.direction-details{font-size:11px;color:#718466;margin-block:12px;}.direction-details summary{cursor:pointer;padding:7px 0;}.art-directions .placement{display:flex;flex-wrap:wrap;gap:12px;margin-top:10px;}.placement label,.surface{display:flex;align-items:center;gap:5px;}.placement input,.surface input{accent-color:#5d7853;}.direction-details select{width:100%;margin-block:6px 12px;}.direction-colors{display:flex;gap:14px;margin-bottom:12px;}.direction-colors label{display:flex;align-items:center;gap:6px;}.direction-colors input{width:28px;height:26px;padding:1px;border:1px solid #d5dfce;border-radius:4px;}.surface{margin-block:10px;}
</style>
