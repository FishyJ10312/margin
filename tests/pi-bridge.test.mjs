// Runs the real adapter against an isolated fake SDK/store. Never loads Pi accounts.
import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,mkdir,writeFile,readFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {spawn} from 'node:child_process';
import {createInterface} from 'node:readline';
const adapter=await readFile(new URL('../src-tauri/resources/pi-bridge.mjs',import.meta.url),'utf8');
async function fixture(options={}){
 const root=await mkdtemp(join(tmpdir(),'liubai-pi-sdk-'));
 const sdk=join(root,'node_modules/@earendil-works/pi-ai');
 await mkdir(join(sdk,'dist/providers'),{recursive:true});await mkdir(join(root,'dist/core'),{recursive:true});
 await writeFile(join(root,'package.json'),JSON.stringify({type:'module'}));
 await writeFile(join(sdk,'package.json'),JSON.stringify({type:'module',name:'@earendil-works/pi-ai'}));
 const config=JSON.stringify({...options,marker:join(root,'write-finished')});
 await writeFile(join(root,'dist/core/auth-storage.js'),`
  import {writeFile} from 'node:fs/promises';
  const config=${config};
  export class ReadOnlyAuthStorage{
   async read(){return {type:config.authType||'oauth',access:'FAKE_ACCESS_TOKEN',refresh:'FAKE_REFRESH_TOKEN'};}
   async list(){return [{providerId:config.provider||'openai',type:config.authType||'oauth'}];}
  }
  export class AuthStorage extends ReadOnlyAuthStorage{
   static create(){return new AuthStorage();}
   async modify(){await new Promise(r=>setTimeout(r,80));if(config.writeFailure)throw new Error('FAKE_REFRESH_TOKEN');await writeFile(config.marker,'complete');}
   async delete(){return this.modify();}
  }
 `);
 await writeFile(join(sdk,'dist/index.js'),`
  const config=${config};
  export function createModels({credentials,authContext}){
   const vision={id:'fixture-vision',name:'Fixture vision',input:['text','image'],contextWindow:1000};
   return {
    setProvider(){},
    async checkAuth(){if(await authContext.env('OPENAI_API_KEY')!==undefined)throw new Error('ambient-key-fallback');return true;},
    getModels(){return [vision,{id:'text-only',name:'Text only',input:['text']}];},
    getModel(){return vision;},
    async login(provider,kind,{prompt,notify,signal}){
     notify({type:'auth_url',url:'https://auth.openai.com/authorize?state=fixture'});
     const code=await prompt({type:'manual_code',message:'Synthetic code',signal});
     if(code!=='fixture-code')throw new Error('401 FAKE_ACCESS_TOKEN');
     await credentials.modify(provider,()=>({type:'oauth',access:'FAKE_ACCESS_TOKEN'}));
     return {type:'oauth',access:'FAKE_ACCESS_TOKEN',refresh:'FAKE_REFRESH_TOKEN'};
    },
    async logout(provider){await credentials.delete(provider);},
    streamSimple(model,context,options){
     if(config.requireNoRetries&&options.maxRetries!==0)throw new Error('unsafe-retry-policy');
     if(context.tools.length!==0||context.messages[0].content[1].mimeType!=='image/jpeg')throw new Error('unsafe-context');
     // Deliberately do not await the write; the adapter must keep the child alive.
     void credentials.modify('openai',()=>({type:'oauth',access:'FAKE_ACCESS_TOKEN'}));
     return {
      async *[Symbol.asyncIterator](){yield {type:'text_delta',delta:'fixture response'};},
      async result(){return {stopReason:config.incomplete?'length':'stop',content:[{type:'text',text:'fixture response'}]};}
     };
    }
   };
  }
 `);
 for(const [id,name] of Object.entries({openai:'openaiProvider',anthropic:'anthropicProvider','github-copilot':'githubCopilotProvider',openrouter:'openrouterProvider'}))await writeFile(join(sdk,'dist/providers',id+'.js'),`export const ${name}=()=>({});`);
 async function run(operation,extra={},respond){
  return await new Promise((resolve,reject)=>{
   const child=spawn(process.execPath,['--input-type=module','--eval',adapter],{stdio:['pipe','pipe','pipe'],env:{...process.env,OPENAI_API_KEY:'FAKE_AMBIENT_KEY'}});
   const events=[];let output='',stderr='';
   const timeout=setTimeout(()=>{child.kill();reject(new Error('adapter fixture timed out'));},5000);
   child.stdout.on('data',data=>output+=data);child.stderr.on('data',data=>stderr+=data);
   createInterface({input:child.stdout}).on('line',line=>{const event=JSON.parse(line);events.push(event);respond?.(event,value=>child.stdin.write(JSON.stringify(value)+'\n'));});
   child.once('error',reject);child.once('exit',code=>{clearTimeout(timeout);resolve({events,output,stderr,code});});
   child.stdin.write(JSON.stringify({type:'init',operation,piRoot:root,provider:options.provider||'openai',model:'fixture-vision',hostId:'fixture-host',prompt:'test',images:['data:image/jpeg;base64,eA=='],...extra})+'\n');
  });
 }
 return {root,run,cleanup:()=>rm(root,{recursive:true,force:true})};
}
test('catalog never treats an API key as a subscription and only exposes vision metadata',async()=>{
 for(const provider of ['openai','openrouter']){
  const f=await fixture({provider,authType:'api_key'});
  try{const r=await f.run('catalog');assert.equal(r.code,0);const c=r.events[0].value;assert.equal(c.connected,provider==='openrouter');assert.equal(c.models.length,1);assert.equal(c.models[0].id,'fixture-vision');assert.doesNotMatch(r.output,/FAKE_ACCESS|FAKE_REFRESH|FAKE_AMBIENT/);}finally{await f.cleanup();}
 }
});
test('OAuth prompt roundtrip discards credentials and completes the locked store write',async()=>{
 const f=await fixture();try{
  const r=await f.run('login',{},(e,send)=>{if(e.type==='prompt')send({type:'answer',id:e.id,value:'fixture-code'});});
  assert.ok(r.events.some(e=>e.type==='auth_url'));assert.ok(r.events.some(e=>e.type==='result'&&e.value.connected));
  assert.equal(await readFile(join(f.root,'write-finished'),'utf8'),'complete');assert.doesNotMatch(r.output+r.stderr,/FAKE_ACCESS|FAKE_REFRESH|FAKE_AMBIENT/);
 }finally{await f.cleanup();}
});
test('canceling an OAuth prompt aborts the SDK and exits without a result',async()=>{
 const f=await fixture();try{
  const r=await f.run('login',{},(e,send)=>{if(e.type==='prompt')send({type:'cancel'});});
  assert.ok(r.events.some(e=>e.type==='prompt_cancel'));assert.ok(r.events.some(e=>e.type==='error'&&e.code==='cancelled'));assert.ok(!r.events.some(e=>e.type==='result'));
 }finally{await f.cleanup();}
});
test('inference waits for rotating credentials and rejects partial output',async()=>{
 for(const incomplete of [false,true]){
  const f=await fixture({incomplete});try{
   const r=await f.run('infer');assert.equal(await readFile(join(f.root,'write-finished'),'utf8'),'complete');
   if(incomplete){assert.ok(r.events.some(e=>e.type==='error'&&e.code==='incomplete'));assert.ok(!r.events.some(e=>e.type==='result'));}
   else assert.equal(r.events.find(e=>e.type==='result').value.text,'fixture response');
   assert.doesNotMatch(r.output+r.stderr,/FAKE_ACCESS|FAKE_REFRESH|FAKE_AMBIENT/);
  }finally{await f.cleanup();}
 }
});
test('a rejected credential write overrides a result with a redacted auth failure',async()=>{
 const f=await fixture({writeFailure:true});try{
  const r=await f.run('infer');assert.ok(r.events.some(e=>e.type==='error'&&e.code==='auth'));assert.doesNotMatch(r.output+r.stderr,/FAKE_REFRESH/);
 }finally{await f.cleanup();}
});

test('explicit inference disables SDK client retries',async()=>{
 const f=await fixture({requireNoRetries:true});try{const r=await f.run('infer');assert.ok(r.events.some(e=>e.type==='result'),'The adapter must explicitly disable SDK retries');}finally{await f.cleanup();}
});
