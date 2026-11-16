<script lang="ts">
 import {t} from './i18n';
 import type {FrameDesign} from './types';
 export let value:FrameDesign;
 export let oninvalidate:()=>void=()=>{};
</script>
<div class="art-directions">
 <label class="field-label" for="art-placement">{$t('装饰布局')}</label>
 <select id="art-placement" bind:value={value.placement} on:change={oninvalidate}>
  <option value="auto">{$t('由 AI 根据照片决定')}</option>
  <option value="corners">{$t('对角点缀')}</option><option value="sides">{$t('四边分布')}</option><option value="bottom">{$t('底部重点')}</option>
 </select>
 <label class="field-label" for="art-palette">{$t('配色方式')}</label>
 <select id="art-palette" bind:value={value.palette} on:change={oninvalidate}><option value="photo">{$t('呼应照片配色')}</option><option value="mono">{$t('黑白与象牙色')}</option><option value="custom">{$t('自定义三色')}</option></select>
 {#if value.palette==='custom'}<div class="direction-colors">{#each value.colors as color,i}<label><span>{$t('颜色 {0}',{0:i+1})}</span><input type="color" value={color} aria-label={$t('自定义颜色 {0}',{0:i+1})} on:input={e=>{value.colors[i]=e.currentTarget.value;oninvalidate();}}/></label>{/each}</div>{/if}
 <label class="surface"><input type="checkbox" bind:checked={value.texture} on:change={oninvalidate}/>{$t('添加轻微纸张纹理')}</label>
</div>
<style>
.art-directions{display:grid;gap:8px;}.direction-colors{display:flex;flex-wrap:wrap;gap:12px;}.direction-colors label,.surface{display:flex;align-items:center;gap:6px;font-size:12px;}.direction-colors input{width:28px;height:28px;padding:1px;border:1px solid var(--border);border-radius:4px;}.surface{margin-block:4px;}.surface input{accent-color:var(--green);}
</style>
