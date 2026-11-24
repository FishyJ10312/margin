<script lang="ts">
  import {X} from '@lucide/svelte';
  import {t} from './i18n';
  let dialog:HTMLDialogElement;
  const mac=typeof navigator!=='undefined'&&/Mac|iPhone|iPad/.test(navigator.platform||navigator.userAgent);
  const mod=mac?'⌘':'Ctrl', alt=mac?'⌥':'Alt', shift=mac?'⇧':'Shift';
  /** Every entry here is handled somewhere in the app; keep them in sync. */
  const groups:{title:string;rows:[string[][],string][]}[]=[
    {title:'照片',rows:[
      [[[mod,'O']],'导入照片'],
      [[['←'],['→']],'上一张 / 下一张'],
      [[[shift,'←'],[shift,'→']],'扩展选择'],
      [[[shift,'点击']],'连续选择'],
      [[[mod,'点击']],'加选 / 取消单张'],
      [[[mod,'A']],'全选'],
      [[['Esc']],'取消选择'],
      [[[alt,'←'],[alt,'→']],'把选中的照片前移 / 后移'],
      [[['拖动缩略图']],'调整顺序'],
      [[['Delete']],'移除选中的照片'],
    ]},
    {title:'编辑',rows:[
      [[[mod,'Z']],'撤销'],
      [[[mod,shift,'Z']],'重做'],
      [[[mod,shift,'C']],'复制当前照片的设置'],
      [[[mod,shift,'V']],'粘贴设置到选中的照片'],
      [[['按住「对比」']],'查看原片'],
      [[['Esc']],'退出样式试看'],
    ]},
    {title:'画面（先点一下预览）',rows:[
      [[['+'],['−']],'放大 / 缩小'],
      [[['0']],'适合窗口'],
      [[['1']],'100% 实际像素'],
      [[['←↑→↓']],'平移画面'],
    ]},
    {title:'其他',rows:[
      [[['?']],'打开这张速查表'],
    ]},
  ];
  export function open(){if(!dialog.open)dialog.showModal();}
</script>

<dialog bind:this={dialog} class="shortcut-sheet" aria-labelledby="shortcut-title">
  <header>
    <h2 id="shortcut-title">{$t('键盘快捷键')}</h2>
    <button type="button" class="icon-button" aria-label={$t('关闭快捷键速查')} on:click={()=>dialog.close()}><X size={16}/></button>
  </header>
  <div class="shortcut-groups">
    {#each groups as group}
      <section>
        <h3>{$t(group.title)}</h3>
        <dl>
          {#each group.rows as [combos,label]}
            <div class="shortcut-row">
              <dt>{$t(label)}</dt>
              <dd>{#each combos as combo,i}{#if i}<span class="or">/</span>{/if}<span class="combo">{#each combo as key}<kbd>{$t(key)}</kbd>{/each}</span>{/each}</dd>
            </div>
          {/each}
        </dl>
      </section>
    {/each}
  </div>
</dialog>

<style>
.shortcut-sheet{padding:0;width:min(640px,92vw);max-height:84vh;border:1px solid var(--ws-line-2);border-radius:12px;background:var(--ws-panel);color:var(--ws-text);box-shadow:var(--ws-shadow);font-family:var(--ws-sans);}
.shortcut-sheet::backdrop{background:rgb(0 0 0 / .45);}
header{display:flex;align-items:center;justify-content:space-between;padding:16px 20px;border-bottom:1px solid var(--ws-line);}
h2{font-size:14px;font-weight:600;}
.icon-button{color:var(--ws-muted);}
.shortcut-groups{display:grid;grid-template-columns:repeat(auto-fit,minmax(260px,1fr));gap:4px 28px;padding:14px 20px 20px;overflow:auto;}
h3{font-size:10.5px;letter-spacing:.08em;text-transform:uppercase;color:var(--ws-faint);margin:12px 0 6px;font-weight:600;}
dl{margin:0;}
.shortcut-row{display:flex;align-items:center;justify-content:space-between;gap:12px;padding:5px 0;border-bottom:1px solid var(--ws-line);font-size:12.5px;}
.shortcut-row:last-child{border-bottom:0;}
dt{color:var(--ws-text);}
dd{margin:0;display:flex;align-items:center;gap:4px;flex-shrink:0;}
.combo{display:inline-flex;gap:3px;}
.or{color:var(--ws-faint);font-size:11px;}
kbd{font:inherit;font-size:11px;min-width:20px;padding:2px 6px;text-align:center;border:1px solid var(--ws-line-2);border-bottom-width:2px;border-radius:5px;background:var(--ws-raised);color:var(--ws-muted);}
</style>
