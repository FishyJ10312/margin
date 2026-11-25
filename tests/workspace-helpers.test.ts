import test from 'node:test';
import assert from 'node:assert/strict';
import {canvasLayout} from '../src/lib/layout-preview.ts';
import {paletteFromPixels} from '../src/lib/photo-palette.ts';

test('Overview geometry matches the native layout regression examples',()=>{
  const at=(ratio:{width:number;height:number}|null)=>canvasLayout(4000,3000,{borderPercent:5,ratio});
  assert.deepEqual([at(null).width,at(null).height],[4300,3300]);
  assert.deepEqual([at({width:1,height:1}).width,at({width:1,height:1}).height],[4300,4300]);
  assert.deepEqual([at({width:4,height:5}).width,at({width:4,height:5}).height],[4300,5375]);
  const portrait=canvasLayout(3000,4000,{borderPercent:5,ratio:null});
  assert.deepEqual([portrait.width,portrait.height,portrait.x,portrait.y],[3300,4300,150,150]);
  const none=canvasLayout(1200,800,{borderPercent:0,ratio:null});
  assert.deepEqual([none.width,none.height,none.x,none.y],[1200,800,0,0]);
  const custom=canvasLayout(1000,1000,{borderPercent:10,ratio:{width:2.35,height:1}});
  assert.equal(custom.height,1200);assert.equal(custom.width,Math.ceil(1200*2.35));assert.equal(custom.photoWidth,1000);
});

test('Photo palette is deterministic, distinct and ordered dark to light',()=>{
  const pixels:[number,number,number][]=[];
  for(let i=0;i<300;i++)pixels.push(i%3===0?[20,30,80]:i%3===1?[230,120,80]:[250,240,220]);
  const palette=paletteFromPixels(pixels,5);
  assert.deepEqual(palette,paletteFromPixels(pixels,5));
  assert.deepEqual(palette,['#141E50','#E67850','#FAF0DC']);
  assert.deepEqual(paletteFromPixels([]),[]);
});
