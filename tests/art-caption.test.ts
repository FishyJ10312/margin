import test from 'node:test';
import assert from 'node:assert/strict';
import {captionWatermark,captionFont} from '../src/lib/art-caption.ts';

const none={kind:'none',text:'',imagePath:null,fontPath:null,fontFamily:null,offsetX:0,offsetY:0,color:'#fff',opacity:1,sizePercent:3,region:'photo',position:'bottom-right',marginPercent:2,cameraTemplate:''};
const art={elements:[],design:{id:'art-museum',colors:['#2B2A27','#B9B1A2','#D9D3C6'],intensity:1}};

test('a directed caption becomes a centred border title in the style ink',()=>{
 const w=captionWatermark(none,{caption:' 清晨的杯子 ',artwork:art},'ZCOOL XiaoWei');
 assert.equal(w.kind,'text');assert.equal(w.text,'清晨的杯子');
 assert.equal(w.region,'border');assert.equal(w.position,'bottom-center');
 assert.equal(w.color,'#2B2A27');assert.equal(w.fontFamily,'ZCOOL XiaoWei');
});
test('an existing watermark or an empty caption is left alone',()=>{
 const mine={...none,kind:'text',text:'© Jeffrey'};
 assert.equal(captionWatermark(mine,{caption:'Title',artwork:art}),mine);
 assert.equal(captionWatermark(none,{caption:'',artwork:art}),none);
 assert.equal(captionFont(['Source Han Sans','站酷小薇体']),'站酷小薇体');
});
