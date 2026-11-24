<script lang="ts">
  import {t} from './i18n';
  import {Check,SlidersHorizontal,Sparkles} from '@lucide/svelte';
  import {canvasLayout} from './layout-preview';
  import type {Photo,RenderSettings} from './types';
  export let photos:Photo[]=[];
  export let shared:RenderSettings;
  export let selected:Set<string>=new Set();
  export let activeId='';
  export let onselect:(id:string,event:MouseEvent)=>void;
  export let onopen:(id:string)=>void;
  const cell=176;
  function ratioLabel(s:RenderSettings,photo:Photo){
    if(!s.ratio)return $t('原比例');
    const {width,height}=s.ratio;
    return Number.isInteger(width)&&Number.isInteger(height)?`${width}:${height}`:(width/height).toFixed(2)+':1';
  }
  function tile(photo:Photo){
    const s=photo.settings??shared;
    const layout=canvasLayout(photo.width||1,photo.height||1,s);
    const scale=cell/Math.max(layout.width,layout.height);
    return {s,w:layout.width*scale,h:layout.height*scale,px:layout.x*scale,py:layout.y*scale,pw:layout.photoWidth*scale,ph:layout.photoHeight*scale};
  }
</script>

<div class="overview" role="listbox" aria-multiselectable="true" aria-label={$t('照片总览')}>
  {#each photos as photo,index (photo.id)}
    {@const view=tile(photo)}
    <button type="button" role="option" class="overview-item" class:selected={selected.has(photo.id)} class:active={photo.id===activeId}
      aria-selected={selected.has(photo.id)} title={photo.name}
      on:click={event=>onselect(photo.id,event)} on:dblclick={()=>onopen(photo.id)}>
      <span class="overview-stage">
        <span class="overview-check" aria-hidden="true">{#if selected.has(photo.id)}<Check size={11} strokeWidth={3}/>{/if}</span>
        <span class="overview-canvas" style:width="{view.w}px" style:height="{view.h}px" style:background={view.s.frameColor}>
          {#if photo.thumbnail}<img src={photo.thumbnail} alt="" draggable="false" style:left="{view.px}px" style:top="{view.py}px" style:width="{view.pw}px" style:height="{view.ph}px"/>{/if}
        </span>
      </span>
      <span class="overview-caption">
        <span class="overview-index">{String(index+1).padStart(2,'0')}</span>
        <span>{ratioLabel(view.s,photo)} · {view.s.borderPercent}%</span>
        {#if view.s.artwork||view.s.decoration.kind!=='none'}<span class="overview-flag" title={$t('含模板或装饰，单张视图显示准确效果')}><Sparkles size={11}/></span>{/if}
        {#if photo.settings}<span class="overview-flag own" title={$t('单独设置')}><SlidersHorizontal size={11}/></span>{/if}
      </span>
    </button>
  {/each}
</div>
<p class="overview-note">{$t('总览按真实比例和留白排版，只显示底色；模板、装饰和水印请在单张视图查看。双击打开单张。')}</p>

<style>
.overview{display:grid;grid-template-columns:repeat(auto-fill,minmax(212px,1fr));gap:22px 18px;padding:24px 28px 8px;align-content:start;overflow:auto;min-height:0;flex:1;}
.overview-item{display:flex;flex-direction:column;align-items:stretch;gap:8px;padding:0;border:0;background:none;color:var(--muted);cursor:pointer;text-align:left;}
.overview-stage{position:relative;display:grid;place-items:center;aspect-ratio:1;border-radius:10px;border:1px solid transparent;background:var(--ws-well,rgb(255 255 255 / .03));transition:background .12s,border-color .12s;}
.overview-item:hover .overview-stage{background:var(--ws-hover,rgb(255 255 255 / .06));}
.overview-item.selected .overview-stage{border-color:var(--accent,#e3a446);background:var(--accent-soft,rgb(227 164 70 / .12));}
.overview-item:focus-visible{outline:none;}
.overview-item:focus-visible .overview-stage{outline:2px solid var(--accent,#e3a446);outline-offset:2px;}
.overview-canvas{position:relative;display:block;box-shadow:0 6px 18px rgb(0 0 0 / .35);}
.overview-canvas img{position:absolute;display:block;object-fit:fill;}
.overview-check{position:absolute;top:8px;left:8px;width:18px;height:18px;border-radius:50%;border:1.5px solid var(--border-strong,#444);display:grid;place-items:center;color:var(--on-accent,#1a1408);}
.overview-item.selected .overview-check{background:var(--accent,#e3a446);border-color:var(--accent,#e3a446);}
.overview-caption{display:flex;align-items:center;gap:8px;font-size:11.5px;padding-inline:2px;font-variant-numeric:tabular-nums;}
.overview-index{color:var(--faint);}
.overview-flag{display:inline-flex;color:var(--faint);}
.overview-flag.own{color:var(--ws-own,#b7a0ff);}
.overview-item.active .overview-caption{color:var(--ink);}
.overview-note{padding:8px 28px 14px;font-size:11.5px;color:var(--faint);}
</style>
