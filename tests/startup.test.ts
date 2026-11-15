import assert from 'node:assert/strict';
import test from 'node:test';
import {startupDeadline} from '../src/lib/startup.ts';

test('an unanswered startup produces an actionable failure instead of an endless spinner',async()=>{
 await assert.rejects(startupDeadline(new Promise(()=>{}),5),/重新加载/);
});
test('startup preserves successful responses and actual errors',async()=>{
 assert.equal(await startupDeadline(Promise.resolve('ready'),20),'ready');
 await assert.rejects(startupDeadline(Promise.reject(new Error('read failed')),20),/read failed/);
});
