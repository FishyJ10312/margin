import test from 'node:test';
import assert from 'node:assert/strict';
import {patchAgentSettings,absoluteLocalPath,catalogIds} from '../src/lib/agent-settings.ts';
import {defaultSettings} from '../src/lib/types.ts';

test('Agent settings patch preserves unrelated fields and resolves a selected Chinese font',()=>{
  const before=structuredClone(defaultSettings);before.borderPercent=12;before.watermark.text='FishyJ';
  const after=patchAgentSettings(before,{frameColor:'#F3ECDD',watermark:{kind:'text',fontFamily:'中文字体',sizePercent:2.5}},[{family:'中文字体',path:'/fonts/chinese.otf'}]);
  assert.equal(after.watermark.text,'FishyJ');assert.equal(after.watermark.fontPath,'/fonts/chinese.otf');assert.equal(after.borderPercent,12);assert.deepEqual(after.export,before.export);assert.equal(before.frameColor,'#FFFFFF');
  assert.equal(patchAgentSettings(before,{ratio:null}).ratio,null);
});

test('Unknown fields and executable/configuration-shaped inputs never become settings',()=>{
  for(const patch of [JSON.parse('{"__proto__":{"polluted":true}}'),{watermark:{constructor:{}}},{apiKey:'secret'},{artwork:{}}])assert.throws(()=>patchAgentSettings(defaultSettings,patch));
  assert.throws(()=>patchAgentSettings(defaultSettings,{watermark:{fontPath:'/private/authorization.json'}}));
  assert.throws(()=>patchAgentSettings(defaultSettings,{watermark:{imagePath:'/private/authorization.json'}}));
  assert.equal(({} as {polluted?:boolean}).polluted,undefined);
});

test('Photo selection fails closed on stale IDs and preserves catalog order',()=>{
  assert.deepEqual(catalogIds(['c','a'],['a','b','c']),['a','c']);assert.throws(()=>catalogIds(['gone'],['a','b']));
  for(const path of ['/fixture-data/photo.png','C:\\Photos\\one.jpg','\\\\server\\photos\\one.jpg'])assert.equal(absoluteLocalPath(path),path);
  for(const path of ['relative.png','https://example.invalid/photo.png','file:///fixture-data/photo.png','/bad\0path'])assert.throws(()=>absoluteLocalPath(path));
});
