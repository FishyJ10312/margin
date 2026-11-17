import { mount } from 'svelte';
import ColorSampler from '../../src/lib/ColorSampler.svelte';
import '../../src/app.css';
import '../../src/gallery.css';
import '../../src/paper.css';
import '../../src/darkroom.css';
const state={requests:[] as {cmd:string;x:number;y:number;time:number}[],updates:[] as number[]};
const fixture=(color:string)=>`data:image/svg+xml,${encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" width="21" height="21"><rect width="21" height="21" fill="${color}"/></svg>`)}`;
Object.assign(window,{__TAURI_INTERNALS__:{invoke:async(cmd:string,args:any)=>{
 state.requests.push({cmd,x:args.x??0,y:args.y??0,time:performance.now()});await new Promise(r=>setTimeout(r,20));
 if(cmd==='get_sampler_image'){const x=args.x==null?0:Math.max(0,Math.floor(args.x/512)*512-16),y=args.y==null?0:Math.max(0,Math.floor(args.y/512)*512-16);return {x,y,width:args.x==null?800:Math.min(544,4000-x),height:args.y==null?600:Math.min(544,3000-y),dataUrl:fixture('#1256b3')};}
 if(cmd==='get_watermark_detail')return {dataUrl:fixture('#fff'),viewWidth:160,viewHeight:60,width:40,height:12,watermarkBounds:{x:10,y:10,width:40,height:12},position:{x:10,y:10,width:40,height:12},limits:{minX:-200,maxX:200,minY:-100,maxY:100}};
 return {x:args.x??0,y:args.y??0,hex:'#1256B3',patchDataUrl:fixture('#1256b3')};}}});
const canvas=document.createElement('div');canvas.className='canvas';document.querySelector('#root')!.append(canvas);
const img=document.createElement('img');img.className='rendered-photo';img.style.cssText='width:640px;height:480px;max-width:none;max-height:none';img.src=fixture('#8ab');canvas.append(img);
mount(ColorSampler,{target:canvas,props:{image:img,photo:{id:'fixture',name:'fixture',path:'synthetic',width:4000,height:3000,thumbnail:img.src},preview:{dataUrl:img.src,width:4300,height:3300,photoBounds:{x:150,y:150,width:4000,height:3000}},oncolor:()=>{},onclose:()=>{}}});
new MutationObserver(()=>{state.updates.push(performance.now());}).observe(canvas,{subtree:true,characterData:true,childList:true});
Object.assign(window,{runPickerMotion:async()=>{
 await new Promise(r=>setTimeout(r,100));const start=performance.now();state.updates=[];state.requests=[];
 const surface=canvas.querySelector('.sampling-surface')!;const rect=img.getBoundingClientRect();
 for(let i=0;i<45;i++){surface.dispatchEvent(new MouseEvent('mousemove',{bubbles:true,clientX:rect.left+rect.width*(.2+i*.012),clientY:rect.top+rect.height*.5}));await new Promise(r=>setTimeout(r,16));}
 const end=performance.now();const count=state.updates.filter(t=>t>=start&&t<=end).length;
 const result={updatesDuringMovement:count,nativeRequestsDuringMovement:state.requests.filter(r=>r.time<end).length,elapsedMs:end-start,pass:count>=12};
 document.querySelector('#result')!.textContent=JSON.stringify(result,null,2);return result;
},qaState:state});

import PhotoViewport from '../../src/lib/PhotoViewport.svelte';
import FontPicker from '../../src/lib/FontPicker.svelte';
import WatermarkDetail from '../../src/lib/WatermarkDetail.svelte';
import {defaultSettings} from '../../src/lib/types';
const tests=document.createElement('div');tests.style.cssText='position:relative;width:820px;height:600px;display:flex;flex-direction:column';document.body.append(tests);
const photo={id:'fixture',name:'fixture',path:'synthetic',width:4000,height:3000,thumbnail:img.src};
mount(PhotoViewport,{target:tests,props:{photo,preview:{dataUrl:img.src,width:4300,height:3300,photoBounds:{x:150,y:150,width:4000,height:3000}},oncolor:()=>{},onclose:()=>{}}});
let chosen='';const fontRoot=document.createElement('div');fontRoot.style.width='270px';document.body.append(fontRoot);
mount(FontPicker,{target:fontRoot,props:{fonts:[{family:'Noto Sans SC',path:'',chinese:true,bundled:true},{family:'A Test',path:'a.ttf',chinese:true},{family:'B Test',path:'b.ttf',chinese:true}],selected:null,onchange:(f:any)=>{chosen=f?.family??'';}}});
mount(WatermarkDetail,{target:fontRoot,props:{photo,settings:{...defaultSettings,watermark:{...defaultSettings.watermark,kind:'text',text:'中文署名'}},onlimits:()=>{}}});
Object.assign(window,{runZoomAndFonts:async()=>{
 await new Promise(r=>setTimeout(r,200));state.requests=[];
 const host=tests.querySelector('.photo-viewport')!;const r=host.getBoundingClientRect();const before=(tests.querySelector('.zoom-photo') as HTMLElement).style.transform;
 for(let i=0;i<30;i++){host.dispatchEvent(new WheelEvent('wheel',{bubbles:true,cancelable:true,deltaY:-5,clientX:r.left+r.width/2,clientY:r.top+r.height/2}));await new Promise(r=>setTimeout(r,16));}
 const after=(tests.querySelector('.zoom-photo') as HTMLElement).style.transform;
 const list=fontRoot.querySelector('[role=listbox]') as HTMLElement;list.focus();list.dispatchEvent(new KeyboardEvent('keydown',{bubbles:true,key:'ArrowDown'}));
 await new Promise(r=>setTimeout(r,20));
 const detailBefore=(fontRoot.querySelector('.fit-image img') as HTMLElement).getBoundingClientRect().width;
 const detail=fontRoot.querySelector('[aria-label="水印局部放大倍数"]') as HTMLSelectElement;detail.value='4';detail.dispatchEvent(new Event('change',{bubbles:true}));
 await new Promise(r=>setTimeout(r,30));
 return {before,after,font:chosen,zoomNativeRequests:state.requests.length,detailZoomWidth:(fontRoot.querySelector('.fit-image img') as HTMLElement)?.getBoundingClientRect().width,detailFitWidth:detailBefore,pass:before!==after&&chosen==='A Test'&&state.requests.length===0};
}});
