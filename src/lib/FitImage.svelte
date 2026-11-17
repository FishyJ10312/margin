<script lang="ts">
 import {onMount,onDestroy} from 'svelte';
 export let src='',width=1,height=1,zoom=1,alt='';
 let host:HTMLDivElement,boxWidth=240,boxHeight=150,observer:ResizeObserver;
 let drag:{x:number;y:number;left:number;top:number}|null=null;
 $: fit=Math.min((boxWidth-20)/Math.max(width,1),(boxHeight-20)/Math.max(height,1));
 $: drawWidth=Math.max(1,width*fit*zoom);
 $: drawHeight=Math.max(1,height*fit*zoom);
 onMount(()=>{observer=new ResizeObserver(entries=>{boxWidth=entries[0].contentRect.width;boxHeight=entries[0].contentRect.height;});observer.observe(host);});
 onDestroy(()=>observer?.disconnect());
 function start(e:PointerEvent){if(e.button!==0||!e.isPrimary||zoom<=1)return;host.setPointerCapture(e.pointerId);drag={x:e.clientX,y:e.clientY,left:host.scrollLeft,top:host.scrollTop};}
 function move(e:PointerEvent){if(drag){host.scrollLeft=drag.left+drag.x-e.clientX;host.scrollTop=drag.top+drag.y-e.clientY;}}
 function key(e:KeyboardEvent){const offsets:Record<string,[number,number]>={ArrowLeft:[-30,0],ArrowRight:[30,0],ArrowUp:[0,-30],ArrowDown:[0,30]};if(offsets[e.key]){e.preventDefault();host.scrollBy(...offsets[e.key]);}}
</script>
<!-- svelte-ignore a11y_no_noninteractive_tabindex a11y_no_noninteractive_element_interactions (Scrollable image region supports keyboard panning.) -->
<div bind:this={host} class="fit-image" class:zoomed={zoom>1} role="region" aria-label={alt} tabindex="0" on:pointerdown={start} on:pointermove={move} on:lostpointercapture={()=>drag=null} on:keydown={key}>
 <div class="image-plane" style:width={`${Math.max(boxWidth,drawWidth+20)}px`} style:height={`${Math.max(boxHeight,drawHeight+20)}px`}>
  {#if src}<img {src} {alt} width={Math.max(1,width)} height={Math.max(1,height)} style:width={`${drawWidth}px`} style:height={`${drawHeight}px`} draggable="false"/>{/if}
 </div>
</div>
<style>
.fit-image{width:100%;height:100%;overflow:auto;overscroll-behavior:contain;scrollbar-width:thin;background:repeating-conic-gradient(#e4e0d9 0% 25%,#f6f3ef 0% 50%) 50%/16px 16px;border-radius:6px;outline-offset:-2px;touch-action:none;}
.image-plane{display:flex;align-items:center;justify-content:center;min-width:0;min-height:0;}
img{display:block;max-width:none;max-height:none;object-fit:contain;user-select:none;-webkit-user-select:none;pointer-events:none;flex-shrink:0;}
.zoomed{cursor:grab;}.zoomed:active{cursor:grabbing;}
</style>
