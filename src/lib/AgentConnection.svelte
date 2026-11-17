<script lang="ts">
  import {onMount,onDestroy} from 'svelte';
  import {invoke} from '@tauri-apps/api/core';
  import {X,Plug,Copy,Check,LoaderCircle} from '@lucide/svelte';
  import {t} from './i18n';
  import type {AgentStatus} from './agent-settings';
  export let onclose:()=>void;
  export let onstatus:(status:AgentStatus)=>void;
  let dialog:HTMLDialogElement,configField:HTMLTextAreaElement,copyButton:HTMLButtonElement;
  let status:AgentStatus|null=null,loading=true,busy=false,alive=true,problem='',copyNotice='',copied=false,client='codex';
  let enabled=false,allowWrites=true,statusRevision=0;
  let interval:ReturnType<typeof setInterval>|undefined,copyTimer:ReturnType<typeof setTimeout>|undefined;
  let removeCopyListener=()=>{};
  $: config=client==='codex'?status?.codexToml??'':status?.clientJson??'';
  onMount(()=>{
    dialog.showModal();void refresh();interval=setInterval(()=>{if(!busy)void refresh(false);},1500);
    const copy=()=>void copyConfig();
    if('CommandEvent' in window){
      const command=(event:Event)=>{if(event.target===configField&&(event as Event&{command?:string}).command==='--copy')copy();};
      document.addEventListener('command',command,true);removeCopyListener=()=>document.removeEventListener('command',command,true);
    }else{copyButton.addEventListener('click',copy);removeCopyListener=()=>copyButton.removeEventListener('click',copy);}
  });
  onDestroy(()=>{alive=false;clearInterval(interval);clearTimeout(copyTimer);removeCopyListener();});
  function acceptStatus(result:AgentStatus){status=result;enabled=result.enabled;allowWrites=result.allowWrites;onstatus(result);}
  async function refresh(showError=true){const revision=++statusRevision;try{const result=await invoke<AgentStatus>('get_agent_status');if(alive&&revision===statusRevision)acceptStatus(result);}catch(error){if(alive&&showError&&revision===statusRevision)problem=String(error);}finally{if(alive&&revision===statusRevision)loading=false;}}
  async function access(enabled:boolean,allowWrites:boolean){
    if(busy)return;busy=true;problem='';const revision=++statusRevision;
    try{const result=await invoke<AgentStatus>('set_agent_access',{enabled,allowWrites});if(alive&&revision===statusRevision)acceptStatus(result);}
    catch(error){if(alive){problem=String(error);await refresh(false);}}
    finally{if(alive)busy=false;}
  }
  async function copyConfig(){
    if(!config)return;clearTimeout(copyTimer);copyNotice='';copied=false;
    try{await navigator.clipboard.writeText(config);if(!alive)return;copied=true;copyNotice=$t('配置已复制');}
    catch{if(!alive)return;configField.focus();configField.select();copyNotice=$t('已选中配置，请按 ⌘C / Ctrl+C 复制');}
    copyTimer=setTimeout(()=>{if(alive){copyNotice='';copied=false;}},3500);
  }
</script>

<dialog class="agent-connection" bind:this={dialog} aria-labelledby="agent-connection-title" on:close={onclose}>
  <header><div><h2 id="agent-connection-title"><Plug size={18} aria-hidden="true"/>{$t('Agent 连接')}</h2><p>{$t('让你的 agent 直接使用 Margin')}</p></div><!-- svelte-ignore a11y_autofocus (A user-opened modal starts on its safe close action.) --><button type="button" autofocus class="icon-button" aria-label={$t('关闭 Agent 连接')} on:click={()=>dialog.close()}><X size={18}/></button></header>
  <div class="agent-body">
    <p class="agent-intro">{$t('连接后，agent 可查看照片信息与预览，调整边框和水印，并在本机导出照片。')}</p>
    <div class="agent-access">
      <label class="agent-toggle"><span><strong>{$t('允许本机 agent 连接')}</strong><small>{$t('MCP · 仅限这台电脑')}</small></span><input id="agent-enabled" type="checkbox" bind:checked={enabled} disabled={loading||busy} on:change={event=>void access(event.currentTarget.checked,allowWrites)}/></label>
      <label class="agent-edit-toggle"><input id="agent-writes" type="checkbox" bind:checked={allowWrites} disabled={loading||busy} on:change={event=>void access(enabled,event.currentTarget.checked)}/><span>{$t('允许调整、导入与导出')}</span></label>
      <p class="agent-status" role="status">{#if loading||busy}<LoaderCircle size={13} class="spin"/>{$t(loading?'读取连接状态…':'正在更新连接…')}{:else if status?.enabled}<span class="agent-dot"></span>{$t(status.activeRequests?'Agent 正在操作…':status.allowWrites?'已开启 · 可操作照片':'已开启 · 只读')}{:else}{$t('未开启')}{/if}</p>
    </div>
    <div class="agent-config-heading"><h3>{$t('接入配置')}</h3><label class="agent-client"><span class="sr-only">{$t('配置格式')}</span><select id="agent-client" bind:value={client}><option value="codex">Codex</option><option value="json">{$t('通用 MCP JSON')}</option></select></label></div>
    <p class="agent-help">{$t('在 agent 的 MCP 设置中添加下方配置，随后刷新连接。使用时保持 Margin 打开。')}</p>
    <label class="sr-only" for="agent-config-code">{$t('MCP 接入配置')}</label>
    <textarea id="agent-config-code" bind:this={configField} class="agent-code" readonly spellcheck="false" value={config} rows="5"></textarea>
    <div class="agent-copy-row"><button bind:this={copyButton} type="button" class="button secondary compact" {...{command:'--copy',commandfor:'agent-config-code'}} disabled={!config}>{#if copied}<Check size={14}/>{:else}<Copy size={14}/>{/if}{$t('复制配置')}</button><span role="status" class="agent-copy-notice">{copyNotice}</span></div>
    <details class="agent-details"><summary>{$t('可用操作与连接说明')}</summary><ul><li>{$t('照片选择、导入与批量导出')}</li><li>{$t('边框、模板、水印和字体')}</li><li>{$t('实际加框预览与导出进度')}</li><li>{$t('目录移除及最近 20 次撤销／重做，原图保留')}</li></ul><p>{$t('操作由连接的 agent 发起，Margin 在本机处理照片。模型登录资料独立保存。')}</p>{#if status?.endpoint}<p class="agent-endpoint">{status.endpoint}</p>{/if}<p>{$t('关闭连接后停止接收新指令；已开始的导出可在主界面取消。')}</p></details>
    {#if problem}<p class="agent-problem" role="alert">{$t(problem)}</p>{/if}
  </div>
  <footer><span>{$t('无需额外安装 Node 或 Pi')}</span><button type="button" class="button primary compact" on:click={()=>dialog.close()}>{$t('完成')}</button></footer>
</dialog>

<style>
  .agent-connection{inline-size:min(650px,94vw);max-block-size:88vh;padding:0;border:1px solid var(--border);border-radius:10px;background:var(--surface);color:var(--ink);box-shadow:0 18px 70px #18241c30;overscroll-behavior:contain;}
  .agent-connection::backdrop{background:#15201a50;overflow:hidden;overscroll-behavior:contain;}
  header{display:flex;align-items:start;justify-content:space-between;gap:20px;padding:21px 24px 18px;border-block-end:1px solid var(--border);}
  h2{display:flex;align-items:center;gap:9px;font-size:19px;font-weight:550;}header p{margin:7px 0 0;color:var(--muted);font-size:12px;}
  .agent-body{padding:20px 24px;max-block-size:calc(88vh - 154px);overflow:auto;overscroll-behavior:contain;}.agent-intro{font-size:13px;line-height:1.8;margin:0 0 18px;}
  .agent-access{padding:16px;border:1px solid var(--border);border-radius:7px;background:var(--surface-soft);}.agent-toggle{display:flex;align-items:center;justify-content:space-between;gap:16px;cursor:pointer;}.agent-toggle strong{font-size:13px;font-weight:550;}.agent-toggle small{display:block;font-size:11px;color:var(--muted);margin-block-start:5px;}
  input[type=checkbox]{inline-size:17px;block-size:17px;accent-color:var(--green);cursor:pointer;}.agent-edit-toggle{display:flex;align-items:center;gap:9px;font-size:12px;margin-block-start:14px;cursor:pointer;}.agent-status{display:flex;align-items:center;gap:7px;min-block-size:18px;font-size:11px;color:var(--muted);margin:12px 0 0;}.agent-dot{inline-size:6px;block-size:6px;border-radius:50%;background:var(--green);}
  .agent-config-heading{display:flex;align-items:center;justify-content:space-between;gap:14px;margin-block-start:23px;}h3{font-size:13px;font-weight:550;}.agent-client select{min-block-size:30px;padding:4px 8px;font-size:12px;}.agent-help,.agent-details{font-size:11px;color:var(--muted);line-height:1.8;}.agent-help{margin:9px 0 11px;}
  .agent-code{display:block;inline-size:100%;box-sizing:border-box;min-block-size:118px;resize:vertical;padding:12px;border:1px solid var(--border);border-radius:5px;background:var(--surface-soft);color:var(--ink);font:11px/1.7 ui-monospace,SFMono-Regular,Consolas,monospace;white-space:pre;overflow:auto;}.agent-copy-row{display:flex;align-items:center;gap:12px;margin-block-start:10px;}.agent-copy-notice{font-size:11px;color:var(--muted);}.agent-details{margin-block-start:20px;}.agent-details summary{cursor:pointer;}.agent-details ul{padding-inline-start:18px;margin-block:8px;}.agent-details p{margin-block:8px;}.agent-endpoint{font-family:ui-monospace,monospace;}.agent-problem{padding:11px;background:#fff0e9;color:#9b422a;border-radius:4px;font-size:12px;line-height:1.7;}
  footer{display:flex;align-items:center;justify-content:space-between;gap:15px;padding:13px 24px;border-block-start:1px solid var(--border);}footer span{font-size:11px;color:var(--muted);}.sr-only{position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip-path:inset(50%);white-space:nowrap;border:0;}
</style>
