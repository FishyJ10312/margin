// Optional, on-demand Pi AI adapter. No agent, extensions, tools or shell runs.
// Only the installed SDK handles its credentials; never send them over stdout.
import {createInterface} from 'node:readline';
import {pathToFileURL} from 'node:url';
import {join,dirname} from 'node:path';
import {existsSync,readFileSync} from 'node:fs';
const output=process.stdout.write.bind(process.stdout);
for(const key of ['log','info','warn','error','debug'])console[key]=()=>{};
const send=value=>output(JSON.stringify(value)+'\n');
const lines=createInterface({input:process.stdin,crlfDelay:Infinity});
const controller=new AbortController();
const replies=new Map();let nextId=0;
let initialize;
const initialized=new Promise(resolve=>initialize=resolve);
lines.on('line',line=>{
 try{const m=JSON.parse(line);if(m.type==='init')initialize(m);
  else if(m.type==='cancel')controller.abort(new Error('cancelled'));
  else if(m.type==='answer'){const p=replies.get(m.id);if(p){replies.delete(m.id);p.resolve(String(m.value));}}
 }catch{controller.abort(new Error('protocol'));}
});
const pending=new Set();let credentialWriteFailed=false;
const track=p=>{pending.add(p);void p.then(()=>pending.delete(p),()=>{credentialWriteFailed=true;pending.delete(p);});return p;};
const prompt=p=>new Promise((resolve,reject)=>{
 const id=String(++nextId);const signals=[p.signal,controller.signal].filter(Boolean);
 const finish=()=>{for(const s of signals)s.removeEventListener('abort',abort);replies.delete(id);};
 const abort=()=>{finish();send({type:'prompt_cancel',id});reject(new Error('cancelled'));};
 if(signals.some(s=>s.aborted)){abort();return;}
 for(const s of signals)s.addEventListener('abort',abort,{once:true});
 replies.set(id,{resolve:value=>{finish();resolve(value);}});
 send({type:'prompt',id,kind:p.type,message:String(p.message).slice(0,1500),placeholder:String(p.placeholder||'').slice(0,250),options:(p.options||[]).slice(0,20).map(o=>({id:String(o.id),label:String(o.label),description:String(o.description||'')}))});
});
const allowed={openai:'openaiProvider',anthropic:'anthropicProvider','github-copilot':'githubCopilotProvider',openrouter:'openrouterProvider'};
let models;
try{
 const m=await initialized;if(!allowed[m.provider])throw new Error('unsupported-provider');
 const sdkRoot=[join(m.piRoot,'node_modules/@earendil-works/pi-ai'),join(dirname(m.piRoot),'pi-ai')].find(root=>{
  try{return existsSync(join(root,'dist/index.js'))&&JSON.parse(readFileSync(join(root,'package.json'),'utf8')).name==='@earendil-works/pi-ai';}catch{return false;}
 });if(!sdkRoot)throw new Error('missing-sdk');
 const sdk=await import(pathToFileURL(join(sdkRoot,'dist/index.js')).href);
 const factory=await import(pathToFileURL(join(sdkRoot,'dist/providers/'+m.provider+'.js')).href);
 const stores=await import(pathToFileURL(join(m.piRoot,'dist/core/auth-storage.js')).href);
 const writable=m.operation==='login'||m.operation==='logout'||m.operation==='infer'||m.refresh;
 const storage=writable?stores.AuthStorage.create():new stores.ReadOnlyAuthStorage();
 const credentials={read:(...a)=>storage.read(...a),list:(...a)=>storage.list(...a),modify:(...a)=>track(storage.modify(...a)),delete:(...a)=>track(storage.delete(...a))};
 models=sdk.createModels({credentials,authContext:{env:async()=>undefined,fileExists:async()=>false}});
 models.setProvider(factory[allowed[m.provider]]());
 const catalog=async()=>{
  const metadata=(await storage.list({signal:controller.signal})).find(c=>c.providerId===m.provider);
  const check=await models.checkAuth(m.provider,{signal:controller.signal});
  // An API key is not silently treated as a subscription. OpenRouter's OAuth
  // intentionally issues a permanent account key; its UI says account balance.
  const connected=!!check&&(metadata?.type==='oauth'||m.provider==='openrouter'&&metadata?.type==='api_key');
  return {provider:m.provider,connected,authType:metadata?.type||null,models:models.getModels(m.provider).filter(v=>v.input?.includes('image')).slice(0,600).map(v=>({id:v.id,name:v.name,input:v.input,contextWindow:v.contextWindow||0,reasoning:!!v.reasoning}))};
 };
 if(m.operation==='login'){
  await models.login(m.provider,'oauth',{signal:controller.signal,prompt,notify:e=>{
   if(e.type==='auth_url')send({type:'auth_url',url:e.url});
   else if(e.type==='device_code')send({type:'device_code',userCode:e.userCode,url:e.verificationUri});
   else if(e.type==='progress'||e.type==='info')send({type:'progress',message:'waiting'});
  }},{getDeviceId:()=>m.hostId,agentName:'Liubai'});
  send({type:'result',value:await catalog()});
 }else if(m.operation==='logout'){
  await models.logout(m.provider,{signal:controller.signal});send({type:'result',value:await catalog()});
 }else if(m.operation==='catalog'){
  if(m.refresh)await models.refresh({providers:[m.provider],signal:controller.signal});
  send({type:'result',value:await catalog()});
 }else if(m.operation==='infer'){
  const info=await catalog();if(!info.connected)throw new Error('subscription-not-connected');
  const model=models.getModel(m.provider,m.model);if(!model||!model.input?.includes('image'))throw new Error('vision-model-required');
  const content=[{type:'text',text:m.prompt}];
  for(const url of m.images||[]){if(typeof url!=='string'||url.length>2*1024*1024||!url.startsWith('data:image/jpeg;base64,'))throw new Error('invalid-image');content.push({type:'image',data:url.split(',')[1],mimeType:'image/jpeg'});}
  const stream=models.streamSimple(model,{messages:[{role:'user',content,timestamp:Date.now()}],tools:[]},{signal:controller.signal,maxTokens:10000,reasoning:'low',maxRetries:0});
  let textBytes=0;for await(const event of stream){if(event.type==='text_delta'){textBytes+=Buffer.byteLength(event.delta);if(textBytes>256*1024){controller.abort();throw new Error('output-too-large');}}}
  const result=await stream.result();if(result.stopReason!=='stop')throw new Error(result.stopReason==='aborted'?'cancelled':'incomplete-response');
  const text=result.content.filter(b=>b.type==='text').map(b=>b.text).join('');if(!text||Buffer.byteLength(text)>256*1024)throw new Error('invalid-output');
  send({type:'result',value:{text}});
 }else throw new Error('unsupported-operation');
}catch(error){
 const message=String(error?.message||'');
 const code=controller.signal.aborted?'cancelled':message==='subscription-not-connected'?'auth':message==='vision-model-required'?'model':/large|invalid-output/.test(message)?'output':/quota|usage.limit|credit|429/i.test(message)?'quota':/auth|credential|refresh|401|403/i.test(message)?'auth':message==='incomplete-response'?'incomplete':'runtime';
 send({type:'error',code});
}finally{
 // Do not exit halfway through Pi's locked, rotating credential write.
 await Promise.allSettled([...pending]);if(credentialWriteFailed)send({type:'error',code:'auth'});lines.close();process.stdin.destroy();
}
