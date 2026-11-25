// Isolated UI harness: native calls are mocked. Rust composition/persistence tests run separately.
import {mount} from 'svelte';
import App from '../../src/App.svelte';
import gallery from './gallery.json';
import {defaultSettings,defaultImageConfig,defaultAiConfig,type RenderSettings} from '../../src/lib/types';
import {singlePhotoSelection,restoredPhotoSelection,selectionAfterRemoval,type PhotoSelection} from '../../src/lib/photo-selection';
import '../../src/app.css';
import '../../src/gallery.css';
import '../../src/paper.css';
import '../../src/darkroom.css';
import '../../src/light.css';
import '../../src/workspace.css';
const source=document.createElement('canvas');source.width=900;source.height=600;const ctx=source.getContext('2d')!;const sky=ctx.createLinearGradient(0,0,0,600);sky.addColorStop(0,'#245d91');sky.addColorStop(.6,'#6eb9cb');sky.addColorStop(1,'#dbbc80');ctx.fillStyle=sky;ctx.fillRect(0,0,900,600);ctx.fillStyle='#f3dba1';ctx.beginPath();ctx.arc(650,110,40,0,Math.PI*2);ctx.fill();const thumbnail=source.toDataURL();
const settings:RenderSettings={...defaultSettings,watermark:{...defaultSettings.watermark,kind:'text',text:'留白 FishyJ',color:'#182e39',region:'border',marginPercent:1}};settings.borderPercent=12;
let templates=JSON.parse(localStorage.getItem('liubai.qa.templates')??'[]');const requests:any[]=[];
const fonts=[{family:'Noto Sans SC',path:'',chinese:true,bundled:true},{family:'站酷小薇体',path:'',chinese:true,bundled:true},{family:'Ma Shan Zheng',path:'',chinese:true,bundled:true},{family:'Times New Roman',path:'/synthetic/font.ttf',chinese:false,bundled:false}];
if(new URLSearchParams(location.search).has('long-font-list'))fonts.push(...Array.from({length:80},(_,i)=>({family:'QA Font '+String(i+1).padStart(2,'0'),path:'/synthetic/font-'+i+'.ttf',chinese:true,bundled:false})));
// Fixture EXIF for every imported photo; the real resolver lives in src-tauri/src/camera_info.rs.
const fixtureCamera:Record<string,string>={camera:'SONY ILCE-7M4',make:'SONY',model:'ILCE-7M4',lens:'FE 35mm F1.4 GM',focal:'35mm',focal35:'35mm',aperture:'f/1.8',shutter:'1/250s',iso:'ISO 100',date:'2026.11.17'};
function cameraCaption(template=''){return template.replace(/\{(\w+)\}/g,(m,k)=>fixtureCamera[k]??m).replace(/\n/g,'   ');}
function composed(s:RenderSettings,detail=false){const c=document.createElement('canvas');c.width=detail?240:780;c.height=detail?100:600;const p=c.getContext('2d')!;p.fillStyle=s.frameColor;p.fillRect(0,0,c.width,c.height);if(s.background?.kind==='blur'&&!detail){p.save();p.filter=`blur(${4+s.background.blur*40}px)`;p.drawImage(source,-40,-40,c.width+80,c.height+80);p.restore();p.globalAlpha=s.background.dim;p.fillStyle=s.frameColor;p.fillRect(0,0,c.width,c.height);p.globalAlpha=1;}p.drawImage(source,detail?0:40,detail?-115:40,detail?240:700,detail?160:467);const family=s.watermark.fontFamily==='Ma Shan Zheng'?'cursive':['Times New Roman','站酷小薇体'].includes(s.watermark.fontFamily??'')?'serif':'sans-serif';p.font=`${s.watermark.fontFamily==='Times New Roman'?'italic ':''}${detail?s.watermark.sizePercent*9:18}px ${family}`;p.globalAlpha=s.watermark.opacity;p.fillStyle=s.watermark.color;if(s.watermark.kind==='camera'){p.font=`${detail?16:13}px ${family}`;p.fillText(cameraCaption(s.watermark.cameraTemplate),detail?12:40,detail?76:565);}else p.fillText(s.watermark.text,detail?30+s.watermark.offsetX/20:500,detail?76+s.watermark.offsetY/10:560);return c.toDataURL();}
const params=new URLSearchParams(location.search);
if(params.has('legacy-scroll')){const supports=CSS.supports.bind(CSS);CSS.supports=((property:string,value?:string)=>property==='overscroll-behavior'?false:value===undefined?supports(property):supports(property,value)) as typeof CSS.supports;}
let releaseStartup=()=>{};
const startupGate=params.has('slow-start')?new Promise<void>(resolve=>releaseStartup=resolve):Promise.resolve();
const importControl={stateRequested:false,stateReturned:false,importDelay:0,activeImports:0,maxImports:0};
const removalControl={fail:false,hold:false,pending:[] as (()=>void)[]};
const undoControl={fail:false,hold:false,pending:[] as (()=>void)[]};
const redoControl={fail:false,hold:false,pending:[] as (()=>void)[]};
const removalHistory:{id:string;order:string[];photos:any[];selection:PhotoSelection}[]=[];
const redoHistory:typeof removalHistory=[];
const selectionControl={hold:false,pending:[] as (()=>void)[],active:0,maxActive:0};
const settingsControl={hold:false,fail:false,pending:[] as (()=>void)[]};
const exportControl={directory:'/synthetic/exports' as string|null,holdDirectory:false,directoryPending:[] as (()=>void)[],holdStart:false,startPending:[] as (()=>void)[],fail:false};
const modelControl={holdSave:false,savePending:[] as (()=>void)[],holdGeneration:false,generationPending:[] as (()=>void)[],failGeneration:false};
const previewControl={hold:false,failNext:false,pending:[] as (()=>void)[],active:0,maxActive:0,rendered:[] as {photoId:string;settings:RenderSettings;dataUrl:string}[]};
const detailControl={delay:0,active:0,maxActive:0,rendered:[] as {settings:RenderSettings;dataUrl:string}[]};
const windowControl={scale:2,inner:{width:2360,height:1600},outer:{width:2360,height:1648},position:{x:100,y:80},area:{position:{x:0,y:48},size:{width:3200,height:1952}},fullscreen:false,maximized:false,rejectResize:false};
let fixturePhotos=params.has('empty')?[]:[{id:'synthetic',name:'synthetic-landscape.png',path:'/synthetic/photo.png',width:900,height:600,thumbnail}];
const agentState={enabled:false,allowWrites:true,endpoint:null as string|null,toolCount:14,activeRequests:0,lastTool:null as string|null,clientJson:JSON.stringify({mcpServers:{margin:{command:'/mock/Margin',args:['--mcp','--connection-file','/mock/agent-mcp/connection.json']}}},null,2),codexToml:'[mcp_servers.margin]\ncommand = "/mock/Margin"\nargs = ["--mcp", "--connection-file", "/mock/agent-mcp/connection.json"]\n'};
const agentControl={ready:false,failAccess:false,pending:new Map<string,{resolve:(value:any)=>void;reject:(error:any)=>void}>()};
let catalogRevision=0,fixtureSelection=singlePhotoSelection(fixturePhotos[0]?.id);
const historyKey='margin.qa.history.'+(params.get('history-persistence')??'disabled');
if(params.has('history-persistence')){
 const saved=JSON.parse(localStorage.getItem(historyKey)??'null');
 if(saved){fixturePhotos=saved.photos;removalHistory.push(...saved.undo);redoHistory.push(...saved.redo);catalogRevision=saved.revision;fixtureSelection=saved.selection;Object.assign(settings,saved.settings);}
}
function persistCatalog(){if(params.has('history-persistence'))localStorage.setItem(historyKey,JSON.stringify({photos:fixturePhotos,undo:removalHistory,redo:redoHistory,revision:catalogRevision,selection:fixtureSelection,settings}));}
const catalogResult=()=>({catalogRevision,photoSelection:structuredClone(fixtureSelection)});
const historySnapshot=()=>({undo:removalHistory.map(r=>({id:r.id,count:r.photos.length,selection:structuredClone(r.selection)})),redo:redoHistory.map(r=>({id:r.id,count:r.photos.length,selection:structuredClone(r.selection)}))});
let id=Date.now();
let chatgptState:any={accounts:[],activeId:null,diagnostic:null};
let cancelLogin:((reason:string)=>void)|null=null;
const authControl={scenario:'success',modelsDelay:0,holdModels:false,modelsPending:[] as (()=>void)[]};
let aiConfig={...structuredClone(defaultAiConfig),styleDirection:'Ocean line art'};
if(params.has('legacy-style')){aiConfig.frameDesign.style='film';aiConfig.frameDesign.placement='corners';aiConfig.designDensity='rich';}
if(params.has('connected-chatgpt')){chatgptState={accounts:[{id:'fixture-account',label:'Fixture account',connected:true,planEnabled:true,status:'saved'}],activeId:'fixture-account'};Object.assign(aiConfig,{provider:'chatgpt',chatgptAccountId:'fixture-account',model:'mock-vision-balanced'});}
if(params.has('image-api-configured'))aiConfig.image={...defaultImageConfig,provider:'openai'};
if(params.has('portrait-photo')&&fixturePhotos.length){
 fixturePhotos[0]={...fixturePhotos[0],name:'synthetic-portrait.png',width:600,height:900};
 settings.ratio={width:1,height:1};
}
if(new URLSearchParams(location.search).has('refine')){settings.artwork={elements:[{kind:'curve',x:0,y:0,width:100,height:100,rotation:0,color:'#457982',opacity:.8,strokeWidth:2,filled:false,points:[[100,80],[250,130],[400,40],[550,90]]}]};aiConfig={...aiConfig,provider:'pi',piProvider:'openai',model:'gpt-6.1-sol'};}
const piAccounts:Record<string,boolean>={openai:true};
const callbacks=new Map<number,Function>(),listeners=new Map<number,{event:string;handler:number}>();
const piLogins=new Map<string,{resolve:Function;reject:Function;provider:string}>();
function piCatalog(provider:string){return {provider,connected:!!piAccounts[provider],authType:piAccounts[provider]?'oauth':null,models:[{id:'gpt-6.1-sol',name:'Vision Balanced',input:['text','image'],contextWindow:100000,reasoning:true},{id:'mock-vision-detailed',name:'Vision Detailed',input:['text','image'],contextWindow:100000,reasoning:true}]};}
function emit(event:string,payload:any){for(const [eventId,v]of listeners)if(v.event===event)callbacks.get(v.handler)?.({event,id:eventId,payload});}
Object.assign(window,{__TAURI_INTERNALS__:{metadata:{currentWindow:{label:'main'},currentWebview:{label:'main'}},transformCallback:(callback:Function)=>{const key=id++;callbacks.set(key,callback);return key;},unregisterCallback:(key:number)=>callbacks.delete(key),invoke:async(cmd:string,args:any={})=>{
 requests.push({cmd,args,time:performance.now()});if(requests.length>300)requests.shift();
 await new Promise(r=>setTimeout(r,15));
 if(cmd==='plugin:window|is_fullscreen')return windowControl.fullscreen;
 if(cmd==='get_agent_status')return structuredClone(agentState);
 if(cmd==='set_agent_access'){if(agentControl.failAccess)throw 'Fixture agent connection unavailable';agentState.enabled=args.enabled;agentState.allowWrites=args.allowWrites;agentState.endpoint=args.enabled?'http://127.0.0.1:39221/mcp':null;return structuredClone(agentState);}
 if(cmd==='agent_window_ready'){agentControl.ready=args.ready;return;}
 if(cmd==='complete_agent_command'){const task=agentControl.pending.get(args.requestId);if(task){agentControl.pending.delete(args.requestId);agentState.activeRequests--;if(args.error)task.reject(args.error);else task.resolve(args.result);}return;}
 if(cmd==='plugin:window|is_maximized')return windowControl.maximized;
 if(cmd==='plugin:window|scale_factor')return windowControl.scale;
 if(cmd==='plugin:window|inner_size')return {...windowControl.inner};
 if(cmd==='plugin:window|outer_size')return {...windowControl.outer};
 if(cmd==='plugin:window|outer_position')return {...windowControl.position};
 if(cmd==='plugin:window|current_monitor')return {name:'Fixture screen',scaleFactor:windowControl.scale,position:windowControl.area.position,size:windowControl.area.size,workArea:windowControl.area};
 if(cmd==='plugin:window|set_size'){if(windowControl.rejectResize)throw 'Fixture window manager declined';const size=args.value.toJSON().Physical;windowControl.outer={width:size.width+windowControl.outer.width-windowControl.inner.width,height:size.height+windowControl.outer.height-windowControl.inner.height};windowControl.inner=size;return;}
 if(cmd==='plugin:window|set_position'){windowControl.position=args.value.toJSON().Physical;return;}
 if(cmd==='get_state'){importControl.stateRequested=true;await startupGate;importControl.stateReturned=true;return {settings:structuredClone(settings),presets:[],frameTemplates:structuredClone(templates),photos:structuredClone(fixturePhotos),batch:null,warnings:[],catalogHistory:historySnapshot(),...catalogResult()};}
 if(cmd==='save_settings'){if(settingsControl.hold)await new Promise<void>(resolve=>settingsControl.pending.push(resolve));if(settingsControl.fail)throw 'Fixture cannot save settings';Object.assign(settings,structuredClone(args.settings));persistCatalog();return structuredClone(args.settings);}
 if(cmd==='save_photo_selection'){
  selectionControl.active++;selectionControl.maxActive=Math.max(selectionControl.maxActive,selectionControl.active);
  try{if(selectionControl.hold)await new Promise<void>(resolve=>selectionControl.pending.push(resolve));if(args.catalogRevision!==catalogRevision)return false;fixtureSelection=restoredPhotoSelection(fixturePhotos.map(p=>p.id),args.selection);persistCatalog();return true;}finally{selectionControl.active--;}
 }
 if(cmd==='reorder_photos'){const current=fixturePhotos.map(p=>p.id);if(args.catalogRevision!==catalogRevision||args.order.length!==current.length||new Set(args.order).size!==current.length||!current.every((id:string)=>args.order.includes(id)))throw 'Catalog changed';const byId=new Map(fixturePhotos.map(p=>[p.id,p]));fixturePhotos=args.order.map((id:string)=>byId.get(id)!);catalogRevision++;persistCatalog();return {order:[...args.order],...catalogResult()};}
 if(cmd==='remove_photos'){
  if(removalControl.hold)await new Promise<void>(resolve=>removalControl.pending.push(resolve));if(removalControl.fail)throw 'Fixture cannot save catalog';
  const removed=fixturePhotos.filter(photo=>args.ids.includes(photo.id));if(!removed.length)return null;
  const before=structuredClone(args.selection??fixtureSelection),order=fixturePhotos.map(p=>p.id);
  const receipt={id:'removal-'+id++,count:removed.length,photoIds:removed.map(p=>p.id)};redoHistory.length=0;removalHistory.push({id:receipt.id,order,photos:structuredClone(removed),selection:before});if(removalHistory.length>20)removalHistory.shift();
  fixtureSelection=selectionAfterRemoval(order,before,args.ids);catalogRevision++;fixturePhotos=fixturePhotos.filter(photo=>!args.ids.includes(photo.id));persistCatalog();return {...receipt,...catalogResult()};
 }
 if(cmd==='undo_photo_removal'){
  if(undoControl.hold)await new Promise<void>(resolve=>undoControl.pending.push(resolve));if(undoControl.fail)throw 'Fixture cannot restore catalog';
  const record=removalHistory.at(-1);if(!record||record.id!==args.undoId)throw 'Stale undo receipt';
  const idMap:Record<string,string>={};for(const photo of record.photos){const existing=fixturePhotos.find(p=>p.path===photo.path);if(existing)idMap[photo.id]=existing.id;else{fixturePhotos.push(structuredClone(photo));idMap[photo.id]=photo.id;}}
  const pool=new Map(fixturePhotos.map(photo=>[photo.id,photo]));const order=[...record.order.map(p=>idMap[p]||p),...fixturePhotos.map(p=>p.id)];fixturePhotos=[];for(const key of order){const photo=pool.get(key);if(photo){fixturePhotos.push(photo);pool.delete(key);}}
  fixtureSelection=restoredPhotoSelection(fixturePhotos.map(p=>p.id),record.selection,idMap);catalogRevision++;removalHistory.pop();const targets=new Set(Object.values(idMap));redoHistory.push({id:record.id,order:[],photos:structuredClone(fixturePhotos.filter(p=>targets.has(p.id))),selection:structuredClone(fixtureSelection)});persistCatalog();return {photos:structuredClone(fixturePhotos),idMap,...catalogResult()};
 }
 if(cmd==='redo_photo_removal'){
  if(redoControl.hold)await new Promise<void>(resolve=>redoControl.pending.push(resolve));if(redoControl.fail)throw 'Fixture cannot redo catalog removal';
  const record=redoHistory.at(-1);if(!record||record.id!==args.redoId)throw 'Stale redo receipt';
  const ids=new Set(record.photos.map(p=>p.id));const removed=fixturePhotos.filter(p=>ids.has(p.id));if(removed.length!==ids.size)throw 'Catalog changed';
  const receipt={id:record.id,count:removed.length,photoIds:removed.map(p=>p.id)};
  const before=structuredClone(args.selection??fixtureSelection),order=fixturePhotos.map(p=>p.id);
  removalHistory.push({id:record.id,order,photos:structuredClone(removed),selection:before});redoHistory.pop();fixtureSelection=selectionAfterRemoval(order,before,receipt.photoIds);fixturePhotos=fixturePhotos.filter(p=>!ids.has(p.id));catalogRevision++;persistCatalog();return {...receipt,...catalogResult()};
 }
 if(cmd==='plugin:dialog|open'&&args.options?.directory){if(exportControl.holdDirectory)await new Promise<void>(resolve=>exportControl.directoryPending.push(resolve));return exportControl.directory;}
 if(cmd==='start_batch'){
  if(exportControl.holdStart)await new Promise<void>(resolve=>exportControl.startPending.push(resolve));
  if(exportControl.fail)throw 'Fixture export cannot start';
  return {id:'fixture-export-'+id++,revision:1,status:'running',outputDirectory:args.outputDirectory,items:fixturePhotos.filter(photo=>args.photoIds.includes(photo.id)).map(photo=>({id:'job-'+id++,photoId:photo.id,name:photo.name,status:'pending',outputPath:null,error:null}))};
 }
 if(cmd==='cancel_batch')return;
 if(cmd==='import_photos'){
  importControl.activeImports++;importControl.maxImports=Math.max(importControl.maxImports,importControl.activeImports);
  try{await new Promise(r=>setTimeout(r,importControl.importDelay));const added=[],errors=[];
   for(const path of args.paths as string[]){if(path.includes('corrupt')){errors.push('corrupt.jpg: test-only damaged image');continue;}if(!/\.(jpe?g|png|heic|heif)$/i.test(path)||fixturePhotos.some(p=>p.path===path))continue;const photo={id:'import-'+id++,name:path.split(/[\\/]/).pop()!,path,width:900,height:600,thumbnail};fixturePhotos.push(photo);added.push(photo);}
   if(added.length){redoHistory.length=0;catalogRevision++;if(!fixtureSelection.activeId)fixtureSelection=singlePhotoSelection(fixturePhotos[0]?.id);}persistCatalog();return {photos:added,errors,...catalogResult()};
  }finally{importControl.activeImports--;}
 }

 if(cmd==='get_preview'){
  previewControl.active++;previewControl.maxActive=Math.max(previewControl.maxActive,previewControl.active);
  const captured=structuredClone(args.settings);
  try{if(previewControl.hold)await new Promise<void>(resolve=>previewControl.pending.push(resolve));if(previewControl.failNext){previewControl.failNext=false;throw 'Fixture preview unavailable';}const dataUrl=composed(captured);previewControl.rendered.push({photoId:args.photoId,settings:captured,dataUrl});return {dataUrl,width:1080,height:780,photoBounds:{x:90,y:90,width:900,height:600},warning:null};}finally{previewControl.active--;}
 }
 if(cmd==='get_watermark_detail'){
  detailControl.active++;detailControl.maxActive=Math.max(detailControl.maxActive,detailControl.active);
  try{await new Promise(r=>setTimeout(r,detailControl.delay));const dataUrl=composed(args.settings,true);detailControl.rendered.push({settings:structuredClone(args.settings),dataUrl});return {dataUrl,width:160,height:26,viewWidth:240,viewHeight:100,watermarkBounds:{x:30,y:50,width:160,height:26},position:{x:450+args.settings.watermark.offsetX,y:720+args.settings.watermark.offsetY,width:160,height:26},limits:{minX:-450,maxX:150,minY:-80,maxY:40}};}finally{detailControl.active--;}
 }
 if(cmd==='get_camera_info')return {...fixtureCamera};
 if(cmd==='set_photo_settings'){
  if(settingsControl.hold)await new Promise<void>(resolve=>settingsControl.pending.push(resolve));
  if(settingsControl.fail)throw 'Fixture cannot save settings';
  for(const photo of fixturePhotos as any[])if(Object.hasOwn(args.updates,photo.id))photo.settings=structuredClone(args.updates[photo.id]);
  if(args.shared)Object.assign(settings,structuredClone(args.shared));
  persistCatalog();return {updates:structuredClone(args.updates),shared:structuredClone(args.shared)};
 }
 if(cmd==='quick_frame_styles')return structuredClone([defaultSettings,...['keyline','scan-35mm'].map(id=>gallery.find(frame=>frame.id===id)!.settings)]);
 if(cmd==='list_fonts')return fonts;
 if(cmd==='list_builtin_frames')return structuredClone(gallery);
 if(cmd==='get_frame_template_previews')return Object.fromEntries(templates.map((t:any)=>[t.id,t.thumbnail]));
 if(cmd==='save_frame_template'){const v={id:'template-'+id++,name:args.name,settings:{...args.settings,watermark:{...defaultSettings.watermark},export:defaultSettings.export},thumbnail};templates=[...templates,v];localStorage.setItem('liubai.qa.templates',JSON.stringify(templates));return v;}
 if(cmd==='delete_frame_template'){templates=templates.filter((t:any)=>t.id!==args.id);localStorage.setItem('liubai.qa.templates',JSON.stringify(templates));return;}
 if(cmd==='get_ai_state')return {imageModels:['gpt-image-2.5-flare','gpt-image-2.6-sunburst','gpt-image-1-mini'],config:structuredClone(aiConfig),codexPath:'/mock/codex',codexLoggedIn:true};
 if(cmd==='save_ai_config'){if(modelControl.holdSave)await new Promise<void>(resolve=>modelControl.savePending.push(resolve));aiConfig=structuredClone(args.config);return;}
 if(cmd==='get_pi_runtime')return {available:true,path:'/mock/pi',version:'fixture-only'};
 if(cmd==='list_pi_models')return piCatalog(args.provider);
 if(cmd==='connect_pi_subscription')return new Promise((resolve,reject)=>{piLogins.set(args.sessionId,{resolve,reject,provider:args.provider});emit('pi-auth-event',{sessionId:args.sessionId,provider:args.provider,phase:'prompt',host:'fixture.invalid',deviceCode:null,prompt:{id:'fixture-prompt',kind:'manual_code',message:'Fixture authorization code',placeholder:'No real account',options:[]}});});
 if(cmd==='answer_pi_prompt'){const item=piLogins.get(args.sessionId);if(!item)throw '授权步骤已结束，请重新连接';piAccounts[item.provider]=true;item.resolve(piCatalog(item.provider));piLogins.delete(args.sessionId);return;}
 if(cmd==='cancel_pi_connection'){piLogins.get(args.sessionId)?.reject('已取消 Pi 请求；已发出的请求可能仍计入服务商用量');piLogins.delete(args.sessionId);return;}
 if(cmd==='disconnect_pi_subscription'){piAccounts[args.provider]=false;return piCatalog(args.provider);}
 if(cmd==='get_chatgpt_state')return structuredClone(chatgptState);
 if(cmd==='chatgpt_sign_in'){
  if(authControl.scenario==='cancel')return new Promise((_,reject)=>cancelLogin=reject);
  const accountId=args.accountId||crypto.randomUUID();const account={id:accountId,label:'Local mock account '+(chatgptState.accounts.length+1),connected:true,planEnabled:authControl.scenario!=='identity-only',status:'saved'};
  chatgptState={...chatgptState,activeId:accountId,accounts:[...chatgptState.accounts.filter((a:any)=>a.id!==accountId),account]};return structuredClone(chatgptState);
 }
 if(cmd==='cancel_chatgpt_sign_in'){cancelLogin?.('已取消 ChatGPT 登录');cancelLogin=null;return;}
 if(cmd==='select_chatgpt_account'){chatgptState.activeId=args.accountId;return structuredClone(chatgptState);}
 if(cmd==='list_chatgpt_models'){
  if(authControl.holdModels)await new Promise<void>(resolve=>authControl.modelsPending.push(resolve));
  await new Promise(r=>setTimeout(r,authControl.modelsDelay));
  if(authControl.scenario==='vault-failure'){chatgptState.accounts=chatgptState.accounts.map((a:any)=>a.id===args.accountId?{...a,connected:false,status:'reconnect'}:a);throw '无法读取本机登录授权，请点击重新连接 ChatGPT';}
  if(authControl.scenario==='models-outage'){chatgptState.accounts=chatgptState.accounts.map((a:any)=>a.id===args.accountId?{...a,status:'unavailable'}:a);throw '无法读取 ChatGPT 模型列表，请检查网络';}
  if(authControl.scenario==='no-models')return [];
  chatgptState.accounts=chatgptState.accounts.map((a:any)=>a.id===args.accountId?{...a,status:'connected'}:a);
  return [{slug:'mock-vision-balanced',displayName:'Mock Vision · Balanced'},{slug:'mock-vision-detailed',displayName:'Mock Vision · Detailed'}];
 }
 if(cmd==='disconnect_chatgpt'){chatgptState.accounts=chatgptState.accounts.map((a:any)=>a.id===args.accountId?{...a,connected:false,planEnabled:false,status:'signedOut'}:a);return {state:structuredClone(chatgptState),remoteRevoked:true};}
 if(cmd==='open_chatgpt_usage')return;
 if(cmd==='prepare_ai_photo')return {token:'mock',dataUrl:thumbnail,width:768,height:512,geometry:{canvasWidth:1080,canvasHeight:780,photoBounds:{x:83,y:115,width:834,height:770},borderPercent:12}};
 if(cmd==='prepare_art_refinement')return {token:'refine',dataUrl:thumbnail,width:768,height:512,before:composed(args.settings),geometry:{canvasWidth:1080,canvasHeight:780,photoBounds:{x:83,y:115,width:834,height:770},borderPercent:12}};
 if(cmd==='list_image_models')return ['gpt-image-2.6-flare','gpt-image-2.6-sunburst'];
 if(cmd.startsWith('generate_')){if(modelControl.holdGeneration)await new Promise<void>(resolve=>modelControl.generationPending.push(resolve));if(modelControl.failGeneration)throw 'Fixture model failure';}
 if(cmd==='generate_complete_frame')return {imagePath:'/mock/owned-complete-frame.png',thumbnail,width:1024,height:1024,summary:'A complete beach illustration with waves, umbrellas and shells'};
 if(cmd==='generate_handdrawn')return {imagePath:'/mock/owned-art.png',thumbnail,width:1024,height:1024,summary:'Original watercolor fish and coral illustration'};
 if(cmd==='local_art_suggestions')return ['art-museum','art-editorial','art-field','art-deckle','art-ink','art-cyanotype','art-wash','art-collage','art-botanical'].map((id,n)=>({title:'Style '+(n+1),rationale:'Coloured from the photo',frameColor:'#F3F0E8',borderPercent:14,decoration:{...defaultSettings.decoration,kind:'none'},artwork:{elements:[],design:{id,colors:['#2B2A27','#B9B1A2','#D9D3C6'],intensity:1,scale:1}}}));
 if(cmd==='generate_art_frames')return (args.token==='refine'?[1]:[1,2,3]).map(n=>({title:'Design '+n,rationale:'Ocean-inspired line work',frameColor:'#F3ECDD',borderPercent:12,decoration:defaultSettings.decoration,artwork:{elements:[{kind:'curve',points:[[10,20],[250,60],[750,30],[990,50]],x:0,y:0,width:100,height:100,rotation:0,color:'#457982',opacity:.8,strokeWidth:2,filled:false}]}}));
 if(cmd==='plugin:event|listen'&&params.has('fail-progress')&&args.event==='batch-progress')throw 'Fixture progress listener unavailable';
 if(cmd==='plugin:event|listen'&&params.has('fail-drop')&&args.event==='tauri://drag-enter')throw 'Fixture drop listener unavailable';
 if(cmd==='plugin:event|listen'){const eventId=id++;listeners.set(eventId,{event:args.event,handler:args.handler});return eventId;}
 if(cmd==='plugin:event|unlisten'){listeners.delete(args.eventId);return;}
 return null;
}}});
Object.assign(window,{__qa:{agentControl,agentState,agent:(name:string,args:any={})=>{if(!agentState.enabled)return Promise.reject('Agent connection is disabled');if(!agentState.allowWrites&&!['margin_get_workspace','margin_get_preview','margin_get_export_status','margin_list_fonts','margin_list_templates'].includes(name))return Promise.reject('Agent connection is read-only');return new Promise((resolve,reject)=>{const requestId=crypto.randomUUID();agentControl.pending.set(requestId,{resolve,reject});agentState.activeRequests++;agentState.lastTool=name;emit('agent-command',{requestId,name,arguments:args});});},requests,fonts,removalControl,undoControl,redoControl,selectionControl,settingsControl,get savedSettings(){return structuredClone(settings);},exportControl,modelControl,authControl,importControl,previewControl,detailControl,windowControl,emitBatch:(batch:any)=>emit('batch-progress',batch),releaseStartup:()=>releaseStartup(),dragEnter:(paths:string[])=>emit('tauri://drag-enter',{paths,position:{x:100,y:100}}),dragLeave:()=>emit('tauri://drag-leave',{}),drop:(paths:string[])=>{emit('tauri://drag-enter',{paths,position:{x:100,y:100}});emit('tauri://drag-drop',{paths,position:{x:100,y:100}});},get catalog(){return {photos:structuredClone(fixturePhotos),history:historySnapshot(),...catalogResult()};},get dropReady(){return [...listeners.values()].some(v=>v.event==='tauri://drag-leave');},get aiConfig(){return structuredClone(aiConfig);},get chatgptState(){return structuredClone(chatgptState);}}});
mount(App,{target:document.querySelector('#app')!});
