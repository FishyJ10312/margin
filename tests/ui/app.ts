// Isolated UI harness: native calls are mocked. Rust composition/persistence tests run separately.
import {mount} from 'svelte';
import App from '../../src/App.svelte';
import gallery from './gallery.json';
import {defaultSettings,defaultImageConfig,defaultAiConfig,type RenderSettings} from '../../src/lib/types';
import '../../src/app.css';
const source=document.createElement('canvas');source.width=900;source.height=600;const ctx=source.getContext('2d')!;const sky=ctx.createLinearGradient(0,0,0,600);sky.addColorStop(0,'#245d91');sky.addColorStop(.6,'#6eb9cb');sky.addColorStop(1,'#dbbc80');ctx.fillStyle=sky;ctx.fillRect(0,0,900,600);ctx.fillStyle='#f3dba1';ctx.beginPath();ctx.arc(650,110,40,0,Math.PI*2);ctx.fill();const thumbnail=source.toDataURL();
const settings:RenderSettings={...defaultSettings,watermark:{...defaultSettings.watermark,kind:'text',text:'留白 FishyJ',color:'#182e39',region:'border',marginPercent:1}};settings.borderPercent=12;
let templates=JSON.parse(localStorage.getItem('liubai.qa.templates')??'[]');const requests:any[]=[];
const fonts=[{family:'Noto Sans SC',path:'',chinese:true,bundled:true},{family:'站酷小薇体',path:'',chinese:true,bundled:true},{family:'Ma Shan Zheng',path:'',chinese:true,bundled:true},{family:'Times New Roman',path:'/synthetic/font.ttf',chinese:false,bundled:false}];
if(new URLSearchParams(location.search).has('long-font-list'))fonts.push(...Array.from({length:80},(_,i)=>({family:'QA Font '+String(i+1).padStart(2,'0'),path:'/synthetic/font-'+i+'.ttf',chinese:true,bundled:false})));
function composed(s:RenderSettings,detail=false){const c=document.createElement('canvas');c.width=detail?500:780;c.height=detail?160:600;const p=c.getContext('2d')!;p.fillStyle=s.frameColor;p.fillRect(0,0,c.width,c.height);p.drawImage(source,detail?0:40,detail?-200:40,detail?500:700,detail?333:467);p.font=`${detail?26:18}px serif`;p.globalAlpha=s.watermark.opacity;p.fillStyle=s.watermark.color;p.fillText(s.watermark.text,detail?30+s.watermark.offsetX/20:500,detail?100+s.watermark.offsetY/10:560);return c.toDataURL();}
const params=new URLSearchParams(location.search);
if(params.has('legacy-scroll')){const supports=CSS.supports.bind(CSS);CSS.supports=((property:string,value?:string)=>property==='overscroll-behavior'?false:value===undefined?supports(property):supports(property,value)) as typeof CSS.supports;}
let releaseStartup=()=>{};
const startupGate=params.has('slow-start')?new Promise<void>(resolve=>releaseStartup=resolve):Promise.resolve();
const importControl={stateRequested:false,stateReturned:false,importDelay:0,activeImports:0,maxImports:0};
let fixturePhotos=params.has('empty')?[]:[{id:'synthetic',name:'synthetic-landscape.png',path:'/synthetic/photo.png',width:900,height:600,thumbnail}];
let id=Date.now();
let chatgptState:any={accounts:[],activeId:null,diagnostic:null};
let cancelLogin:((reason:string)=>void)|null=null;
const authControl={scenario:'success'};
let aiConfig={...structuredClone(defaultAiConfig),styleDirection:'Ocean line art'};
if(params.has('legacy-style')){aiConfig.frameDesign.style='film';aiConfig.frameDesign.placement='corners';aiConfig.designDensity='rich';}
if(params.has('connected-chatgpt')){chatgptState={accounts:[{id:'fixture-account',label:'Fixture account',connected:true,planEnabled:true}],activeId:'fixture-account'};Object.assign(aiConfig,{provider:'chatgpt',chatgptAccountId:'fixture-account',model:'mock-vision-balanced'});}
if(new URLSearchParams(location.search).has('refine')){settings.artwork={elements:[{kind:'curve',x:0,y:0,width:100,height:100,rotation:0,color:'#457982',opacity:.8,strokeWidth:2,filled:false,points:[[100,80],[250,130],[400,40],[550,90]]}]};aiConfig={...aiConfig,provider:'pi',piProvider:'openai',model:'gpt-6.1-sol'};}
const piAccounts:Record<string,boolean>={openai:true};
const callbacks=new Map<number,Function>(),listeners=new Map<number,{event:string;handler:number}>();
const piLogins=new Map<string,{resolve:Function;reject:Function;provider:string}>();
function piCatalog(provider:string){return {provider,connected:!!piAccounts[provider],authType:piAccounts[provider]?'oauth':null,models:[{id:'gpt-6.1-sol',name:'Vision Balanced',input:['text','image'],contextWindow:100000,reasoning:true},{id:'mock-vision-detailed',name:'Vision Detailed',input:['text','image'],contextWindow:100000,reasoning:true}]};}
function emit(event:string,payload:any){for(const [eventId,v]of listeners)if(v.event===event)callbacks.get(v.handler)?.({event,id:eventId,payload});}
Object.assign(window,{__TAURI_INTERNALS__:{metadata:{currentWindow:{label:'main'},currentWebview:{label:'main'}},transformCallback:(callback:Function)=>{const key=id++;callbacks.set(key,callback);return key;},unregisterCallback:(key:number)=>callbacks.delete(key),invoke:async(cmd:string,args:any={})=>{
 requests.push({cmd,args,time:performance.now()});if(requests.length>300)requests.shift();
 await new Promise(r=>setTimeout(r,15));
 if(cmd==='get_state'){importControl.stateRequested=true;await startupGate;importControl.stateReturned=true;return {settings,presets:[],frameTemplates:structuredClone(templates),photos:structuredClone(fixturePhotos),batch:null,warnings:[]};}
 if(cmd==='import_photos'){
  importControl.activeImports++;importControl.maxImports=Math.max(importControl.maxImports,importControl.activeImports);
  try{await new Promise(r=>setTimeout(r,importControl.importDelay));const added=[],errors=[];
   for(const path of args.paths as string[]){if(path.includes('corrupt')){errors.push('corrupt.jpg: test-only damaged image');continue;}if(!/\.(jpe?g|png|heic|heif)$/i.test(path)||fixturePhotos.some(p=>p.path===path))continue;const photo={id:'import-'+id++,name:path.split(/[\\/]/).pop()!,path,width:900,height:600,thumbnail};fixturePhotos.push(photo);added.push(photo);}
   return {photos:added,errors};
  }finally{importControl.activeImports--;}
 }

 if(cmd==='get_preview')return {dataUrl:composed(args.settings),width:1080,height:780,photoBounds:{x:90,y:90,width:900,height:600},warning:null};
 if(cmd==='get_watermark_detail')return {dataUrl:composed(args.settings,true),width:160,height:26,viewWidth:500,viewHeight:160,watermarkBounds:{x:30,y:60,width:160,height:26},position:{x:450+args.settings.watermark.offsetX,y:720+args.settings.watermark.offsetY,width:160,height:26},limits:{minX:-450,maxX:150,minY:-80,maxY:40}};
 if(cmd==='list_fonts')return fonts;
 if(cmd==='list_builtin_frames')return structuredClone(gallery);
 if(cmd==='save_frame_template'){const v={id:'template-'+id++,name:args.name,settings:{...args.settings,watermark:{...defaultSettings.watermark},export:defaultSettings.export},thumbnail};templates=[...templates,v];localStorage.setItem('liubai.qa.templates',JSON.stringify(templates));return v;}
 if(cmd==='delete_frame_template'){templates=templates.filter((t:any)=>t.id!==args.id);localStorage.setItem('liubai.qa.templates',JSON.stringify(templates));return;}
 if(cmd==='get_ai_state')return {imageModels:['gpt-image-2.5-flare','gpt-image-2.6-sunburst','gpt-image-1-mini'],config:structuredClone(aiConfig),codexPath:'/mock/codex',codexLoggedIn:true};
 if(cmd==='save_ai_config'){aiConfig=structuredClone(args.config);return;}
 if(cmd==='get_pi_runtime')return {available:true,path:'/mock/pi',version:'fixture-only'};
 if(cmd==='list_pi_models')return piCatalog(args.provider);
 if(cmd==='connect_pi_subscription')return new Promise((resolve,reject)=>{piLogins.set(args.sessionId,{resolve,reject,provider:args.provider});emit('pi-auth-event',{sessionId:args.sessionId,provider:args.provider,phase:'prompt',host:'fixture.invalid',deviceCode:null,prompt:{id:'fixture-prompt',kind:'manual_code',message:'Fixture authorization code',placeholder:'No real account',options:[]}});});
 if(cmd==='answer_pi_prompt'){const item=piLogins.get(args.sessionId);if(!item)throw '授权步骤已结束，请重新连接';piAccounts[item.provider]=true;item.resolve(piCatalog(item.provider));piLogins.delete(args.sessionId);return;}
 if(cmd==='cancel_pi_connection'){piLogins.get(args.sessionId)?.reject('已取消 Pi 请求；已发出的请求可能仍计入服务商用量');piLogins.delete(args.sessionId);return;}
 if(cmd==='disconnect_pi_subscription'){piAccounts[args.provider]=false;return piCatalog(args.provider);}
 if(cmd==='get_chatgpt_state')return structuredClone(chatgptState);
 if(cmd==='chatgpt_sign_in'){
  if(authControl.scenario==='cancel')return new Promise((_,reject)=>cancelLogin=reject);
  const accountId=args.accountId||crypto.randomUUID();const account={id:accountId,label:'Local mock account '+(chatgptState.accounts.length+1),connected:true,planEnabled:authControl.scenario!=='identity-only'};
  chatgptState={...chatgptState,activeId:accountId,accounts:[...chatgptState.accounts.filter((a:any)=>a.id!==accountId),account]};return structuredClone(chatgptState);
 }
 if(cmd==='cancel_chatgpt_sign_in'){cancelLogin?.('已取消 ChatGPT 登录');cancelLogin=null;return;}
 if(cmd==='select_chatgpt_account'){chatgptState.activeId=args.accountId;return structuredClone(chatgptState);}
 if(cmd==='list_chatgpt_models')return [{slug:'mock-vision-balanced',displayName:'Mock Vision · Balanced'},{slug:'mock-vision-detailed',displayName:'Mock Vision · Detailed'}];
 if(cmd==='disconnect_chatgpt'){chatgptState.accounts=chatgptState.accounts.map((a:any)=>a.id===args.accountId?{...a,connected:false,planEnabled:false}:a);return {state:structuredClone(chatgptState),remoteRevoked:true};}
 if(cmd==='open_chatgpt_usage')return;
 if(cmd==='prepare_ai_photo')return {token:'mock',dataUrl:thumbnail,width:768,height:512,geometry:{canvasWidth:1080,canvasHeight:780,photoBounds:{x:83,y:115,width:834,height:770},borderPercent:12}};
 if(cmd==='prepare_art_refinement')return {token:'refine',dataUrl:thumbnail,width:768,height:512,before:composed(args.settings),geometry:{canvasWidth:1080,canvasHeight:780,photoBounds:{x:83,y:115,width:834,height:770},borderPercent:12}};
 if(cmd==='list_image_models')return ['gpt-image-2.6-flare','gpt-image-2.6-sunburst'];
 if(cmd==='generate_complete_frame')return {imagePath:'/mock/owned-complete-frame.png',thumbnail,width:1024,height:1024,summary:'A complete beach illustration with waves, umbrellas and shells'};
 if(cmd==='generate_handdrawn')return {imagePath:'/mock/owned-art.png',thumbnail,width:1024,height:1024,summary:'Original watercolor fish and coral illustration'};
 if(cmd==='generate_art_frames')return (args.token==='refine'?[1]:[1,2,3]).map(n=>({title:'Design '+n,rationale:'Ocean-inspired line work',frameColor:'#F3ECDD',borderPercent:12,decoration:defaultSettings.decoration,artwork:{elements:[{kind:'curve',points:[[10,20],[250,60],[750,30],[990,50]],x:0,y:0,width:100,height:100,rotation:0,color:'#457982',opacity:.8,strokeWidth:2,filled:false}]}}));
 if(cmd==='plugin:event|listen'&&params.has('fail-progress')&&args.event==='batch-progress')throw 'Fixture progress listener unavailable';
 if(cmd==='plugin:event|listen'&&params.has('fail-drop')&&args.event==='tauri://drag-enter')throw 'Fixture drop listener unavailable';
 if(cmd==='plugin:event|listen'){const eventId=id++;listeners.set(eventId,{event:args.event,handler:args.handler});return eventId;}
 if(cmd==='plugin:event|unlisten'){listeners.delete(args.eventId);return;}
 return null;
}}});
Object.assign(window,{__qa:{requests,fonts,authControl,importControl,releaseStartup:()=>releaseStartup(),dragEnter:(paths:string[])=>emit('tauri://drag-enter',{paths,position:{x:100,y:100}}),dragLeave:()=>emit('tauri://drag-leave',{}),drop:(paths:string[])=>{emit('tauri://drag-enter',{paths,position:{x:100,y:100}});emit('tauri://drag-drop',{paths,position:{x:100,y:100}});},get dropReady(){return [...listeners.values()].some(v=>v.event==='tauri://drag-leave');},get aiConfig(){return structuredClone(aiConfig);},get chatgptState(){return structuredClone(chatgptState);}}});
mount(App,{target:document.querySelector('#app')!});
