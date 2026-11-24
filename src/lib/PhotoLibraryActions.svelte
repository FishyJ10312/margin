<script lang="ts">
  import {onDestroy,tick} from 'svelte';
  import {Trash2,LoaderCircle,FileImage} from '@lucide/svelte';
  import {t} from './i18n';
  import type {Photo} from './types';

  export let photos:Photo[]=[];
  export let selectedIds:string[]=[];
  export let disabled=false;
  export let selectionDisabled=false;
  export let onselectall:()=>void;
  export let onclearselection:()=>void;
  export let canUndo=false;
  export let onundo:()=>void;
  export let canRedo=false;
  export let onredo:()=>void;
  /** Arrow keys move the active photo; Shift extends the selection. */
  export let onstep:(delta:number,extend:boolean)=>void=()=>{};
  export let oncontextselect:(id:string)=>void;
  export let onremove:(ids:string[])=>Promise<void>;
  export let onerror:(message:string)=>void;
  let dialog:HTMLDialogElement;
  let targets:Photo[]=[],busy=false,problem='',alive=true;
  let invoker:HTMLElement|null=null;
  const nativeFocusgroup=typeof HTMLElement!=='undefined'&&'focusGroup' in HTMLElement.prototype;
  let menuPanel:HTMLDivElement,menuItem:HTMLButtonElement;
  let context:{ids:string[];trigger:HTMLElement;x:number;y:number}|null=null,menuRevision=0;
  let menuOpen=false,menuLeft=0,menuTop=0;
  const message=(error:unknown)=>typeof error==='string'?error:error instanceof Error?error.message:$t('操作未完成，请重试');

  export async function requestRemoval(ids:string[],trigger:HTMLElement|null=document.activeElement as HTMLElement|null){
    if(disabled||busy||document.querySelector('dialog[open]'))return;
    const requested=new Set(ids);
    targets=photos.filter(photo=>requested.has(photo.id));
    if(!targets.length)return;
    invoker=trigger;
    invoker?.focus({preventScroll:true});
    problem='';
    await tick();
    if(alive&&!disabled&&!document.querySelector('dialog[open]'))dialog.showModal();
    else targets=[];
  }

  async function confirmRemoval(){
    if(busy||disabled||!targets.length)return;
    busy=true;problem='';
    try{
      await onremove(targets.map(photo=>photo.id));
      if(!alive)return;
      dialog.close('removed');
      await tick();
      // Native dialog restores focus itself unless the removed thumbnail was its
      // invoker. The toolbar's remove button now targets a different selection,
      // so continue from the surviving photo there too.
      if(!invoker?.isConnected||invoker.classList.contains('remove-photo')){
        const next=document.querySelector<HTMLElement>('.filmstrip .thumbnail.active, .empty-state button');
        next?.focus({preventScroll:true});
      }
    }catch(error){
      if(alive){if(dialog.open)problem=message(error);else onerror(message(error));}
    }finally{if(alive)busy=false;}
  }

  function closeMenu(restoreFocus=false){
    const trigger=context?.trigger;
    if(restoreFocus)trigger?.focus({preventScroll:true});
    menuOpen=false;context=null;menuRevision++;
  }

  function chooseContext(){
    const chosen=context;
    closeMenu(true);
    if(chosen)void requestRemoval(chosen.ids,chosen.trigger);
  }

  function outsideMenu(event:PointerEvent){
    if(menuOpen&&event.button!==2&&!menuPanel.contains(event.target as Node))closeMenu();
  }

  async function showMenu(id:string,trigger:HTMLElement,x:number,y:number){
    if(disabled||busy||document.querySelector('dialog[open]')||!photos.some(photo=>photo.id===id))return;
    const revision=++menuRevision;
    const ids=selectedIds.includes(id)?[...selectedIds]:[id];
    context={ids,trigger,x,y};oncontextselect(id);trigger.focus({preventScroll:true});menuOpen=true;
    await tick();
    if(!alive||disabled||revision!==menuRevision||document.querySelector('dialog[open]')){closeMenu();return;}
    // Keep the context menu under app control: WebKit auto-popovers can dismiss
    // during the same macOS context-click gesture that opened them.
    const box=menuPanel.getBoundingClientRect();
    menuLeft=Math.max(8,Math.min(x,innerWidth-box.width-8));
    menuTop=Math.max(8,Math.min(y,innerHeight-box.height-8));
    menuItem.focus({preventScroll:true});
  }

  export function openPhotoMenu(event:MouseEvent,id:string){
    event.preventDefault();
    const trigger=event.currentTarget as HTMLElement;
    void showMenu(id,trigger,event.clientX,event.clientY);
  }

  function key(event:KeyboardEvent){
    if(menuOpen){
      if(event.key==='Escape'){event.preventDefault();closeMenu(true);}
      else if(event.key==='Tab')closeMenu(true);
      else if(!nativeFocusgroup&&['ArrowUp','ArrowDown','Home','End'].includes(event.key)){event.preventDefault();menuItem.focus();}
      return;
    }
    if(event.defaultPrevented||event.repeat||event.isComposing||busy||document.querySelector('dialog[open]'))return;
    const target=event.target instanceof Element?event.target:null;
    if(target?.closest('input,textarea,select,[contenteditable]:not([contenteditable="false"]),[role="textbox"],[role="combobox"],[role="listbox"]'))return;
    if(!selectionDisabled&&(event.metaKey||event.ctrlKey)&&event.key.toLowerCase()==='a'&&!event.altKey&&!event.shiftKey){
      event.preventDefault();onselectall();return;
    }
    if(!selectionDisabled&&event.key==='Escape'&&!event.metaKey&&!event.ctrlKey&&!event.altKey&&!event.shiftKey){
      if(selectedIds.length){event.preventDefault();onclearselection();}
      return;
    }
    if(!selectionDisabled&&['ArrowUp','ArrowDown','ArrowLeft','ArrowRight'].includes(event.key)&&!event.metaKey&&!event.ctrlKey&&!event.altKey
      &&!target?.closest('[role="tab"],[role="tablist"],[role="menu"],[role="slider"],.pan-surface,.mark-handle')){
      event.preventDefault();onstep(event.key==='ArrowUp'||event.key==='ArrowLeft'?-1:1,event.shiftKey);return;
    }
    if(disabled)return;
    if((event.metaKey||event.ctrlKey)&&event.key.toLowerCase()==='z'&&!event.altKey){
      if(event.shiftKey&&canRedo){event.preventDefault();onredo();}
      else if(!event.shiftKey&&canUndo){event.preventDefault();onundo();}
      return;
    }
    const thumbnail=target?.closest<HTMLButtonElement>('.thumbnail[data-photo-id]');
    if((event.key==='ContextMenu'||event.key==='F10'&&event.shiftKey)&&thumbnail){
      event.preventDefault();
      const box=thumbnail.getBoundingClientRect();
      void showMenu(thumbnail.dataset.photoId!,thumbnail,box.left+box.width/2,box.bottom);
    }else if(['Backspace','Delete'].includes(event.key)&&!event.metaKey&&!event.ctrlKey&&!event.altKey&&!event.shiftKey){
      if(!selectedIds.length)return;
      event.preventDefault();
      void requestRemoval(selectedIds,thumbnail??document.activeElement as HTMLElement|null);
    }
  }

  onDestroy(()=>{alive=false;menuRevision++;});
</script>

<svelte:window on:keydown={key} on:pointerdown={outsideMenu} on:resize={()=>{if(menuOpen)closeMenu();}}/>

<div bind:this={menuPanel} class="photo-context-menu" hidden={!menuOpen} data-open={menuOpen} style:left={`${menuLeft}px`} style:top={`${menuTop}px`}>
  <div {...{focusgroup:'menu nomemory'}} role={nativeFocusgroup?undefined:'menu'} aria-label={$t('照片操作')}>
    <button bind:this={menuItem} type="button" role={nativeFocusgroup?undefined:'menuitem'} on:click={chooseContext}><Trash2 size={15}/><span>{$t((context?.ids.length??0)>1?'从目录中移除 {0} 张照片…':'从目录中移除…',{0:context?.ids.length??0})}</span><kbd aria-hidden="true">⌫</kbd></button>
  </div>
</div>

<dialog bind:this={dialog} class="photo-removal-dialog" aria-labelledby="photo-removal-title" aria-describedby="photo-removal-description" on:cancel={event=>{if(busy)event.preventDefault();}} on:close={()=>{if(!dialog.open)targets=[];}}>
  <div class="removal-heading"><span class="removal-icon"><Trash2 size={20}/></span><h2 id="photo-removal-title">{$t(targets.length>1?'从目录中移除 {0} 张照片？':'从目录中移除这张照片？',{0:targets.length})}</h2></div>
  <p id="photo-removal-description">{$t('只从照片目录中移除，原始文件仍保留在磁盘中。')}</p>
  {#if targets.length===1}<div class="removal-photo">{#if targets[0].thumbnail}<img src={targets[0].thumbnail} alt=""/>{:else}<FileImage size={24}/>{/if}<span>{targets[0].name}</span></div>{:else if targets.length>1}<div class="removal-photo"><FileImage size={24}/><span>{$t('已选中 {0} 张照片',{0:targets.length})}</span></div>{/if}
  <p class="removal-status" class:has-error={!!problem} role={problem?'alert':'status'}>{#if problem}{problem}{:else if busy}<LoaderCircle size={14} class="spin"/>{$t('正在从目录中移除…')}{/if}</p>
  <form on:submit|preventDefault={confirmRemoval}>
    <button type="button" class="button secondary" disabled={busy} on:click={()=>dialog.close('cancel')}>{$t('取消')}</button>
    <button type="submit" class="button primary confirm-photo-removal" disabled={busy||disabled||!targets.length}>{$t('从目录中移除')}</button>
  </form>
</dialog>

<style>
  .photo-context-menu{position:fixed;inset:auto;margin:0;padding:5px;min-inline-size:214px;max-inline-size:calc(100vw - 16px);border:1px solid var(--border);border-radius:7px;background:var(--surface);color:var(--ink);box-shadow:0 8px 24px #18161424;z-index:1000;}
  .photo-context-menu button{display:flex;align-items:center;gap:9px;inline-size:100%;min-block-size:34px;padding:7px 10px;border:0;border-radius:4px;background:transparent;color:inherit;font-size:12px;text-align:start;white-space:nowrap;}
  .photo-context-menu button:hover,.photo-context-menu button:focus{background:var(--surface-soft);}
  .photo-context-menu button:focus-visible{outline:2px solid var(--green);outline-offset:-2px;}
  kbd{margin-inline-start:auto;padding-inline-start:18px;color:var(--muted);font:12px ui-sans-serif,system-ui,sans-serif;}
  .photo-removal-dialog{inline-size:min(430px,calc(100vw - 40px));max-block-size:calc(100vh - 40px);padding:24px;border:1px solid var(--border);border-radius:10px;background:var(--surface);color:var(--ink);box-shadow:0 16px 60px #18161430;overflow:auto;}
  .photo-removal-dialog:modal{overscroll-behavior:contain;}
  .photo-removal-dialog::backdrop{background:#1b191655;overflow:hidden;overscroll-behavior:contain;}
  .removal-heading{display:flex;align-items:center;gap:12px;}
  .removal-icon{display:grid;place-items:center;flex:0 0 38px;block-size:38px;border:1px solid var(--border);border-radius:8px;background:var(--surface-soft);color:var(--muted);}
  h2{font-size:17px;line-height:1.45;font-weight:550;}
  #photo-removal-description{margin-block:16px;font-size:13px;line-height:1.65;color:var(--muted);}
  .removal-photo{display:flex;align-items:center;gap:12px;min-inline-size:0;padding:10px;border:1px solid var(--border);border-radius:6px;background:var(--surface-soft);font-size:12px;}
  .removal-photo img{inline-size:60px;block-size:46px;object-fit:contain;flex-shrink:0;}
  .removal-photo :global(svg){flex-shrink:0;}.removal-photo span{overflow-wrap:anywhere;}
  .removal-status{min-block-size:24px;margin-block:10px;display:flex;align-items:center;gap:6px;font-size:12px;line-height:1.5;color:var(--muted);}.removal-status.has-error{color:var(--red,#a34a37);}
  form{display:flex;justify-content:flex-end;gap:8px;}.button{min-block-size:36px;}
</style>
