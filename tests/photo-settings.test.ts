import assert from 'node:assert/strict';
import {test} from 'node:test';
import {defaultSettings} from '../src/lib/types.ts';
import {adoptOwnedAssets, frameStyleEdit, photoEdit} from '../src/lib/photo-settings.ts';

test('A native asset receipt adopts the saved copy without reverting newer edits',()=>{
  const sent=structuredClone(defaultSettings);
  sent.watermark.kind='image';
  sent.watermark.imagePath='/import/logo.png';
  const owned=structuredClone(sent);
  owned.watermark.imagePath='/owned/logo.png';
  const newer=structuredClone(sent);
  newer.borderPercent=9;
  const adopted=adoptOwnedAssets(newer,sent,owned);
  assert.equal(adopted.watermark.imagePath,'/owned/logo.png');
  assert.equal(adopted.borderPercent,9);
  newer.watermark.imagePath='/import/new-logo.png';
  assert.equal(adoptOwnedAssets(newer,sent,owned).watermark.imagePath,'/import/new-logo.png');
});

test('Explicit equal-active patches still apply to all selected photos',()=>{
  const active=structuredClone(defaultSettings);
  const another={...active,frameColor:'#000000',borderPercent:17};
  assert.equal(photoEdit(active,active,{frameColor:active.frameColor})(another).frameColor,active.frameColor);
  assert.equal(photoEdit(active,active,{frameColor:active.frameColor})(another).borderPercent,17);
});

test('Keep-layout frames preserve each ratio and never reduce film padding',()=>{
  const active=structuredClone(defaultSettings);
  const style={...active,ratio:{width:4,height:5},borderPercent:10};
  const apply=photoEdit(active,style,frameStyleEdit(style,true),{keepLayout:true,minimumPadding:10});
  const wide={...active,ratio:{width:16,height:9},borderPercent:18};
  assert.equal(apply(wide).borderPercent,18);
  assert.deepEqual(apply(wide).ratio,wide.ratio);
  assert.equal(apply(active).borderPercent,10);
});
