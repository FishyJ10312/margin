// Real result UI with an isolated native transport. No model calls or credentials.
import assert from 'node:assert/strict';
const task=await taskSpace('Margin art result contrast');
console.log({spaceId:task.spaceId});
const page=task.page('p1');
const results=[];
for(const theme of ['dark','light']){
  await page.goto('http://127.0.0.1:1420/tests/ui/app.html?connected-chatgpt&image-api-configured');
  await page.waitForSelector('.art-trigger:not([disabled])');
  await page.selectOption('.language-select','en');
  await page.selectOption('.theme-select',theme);
  await page.click('.art-trigger');
  await page.waitForSelector('#generation-method');
  await page.selectOption('#generation-method','image');
  await page.waitForSelector('#use-local-codex');
  await page.click('#use-local-codex');
  await page.waitForSelector('.model-dialog form > fieldset > .model-consent input:not([disabled])');
  await page.click('.model-dialog form > fieldset > .model-consent input');
  await page.click('.model-dialog [type=submit]');
  await page.waitForSelector('.art-suggestions img');
  const contrast=await page.evaluate(()=>{
    const card=document.querySelector('.art-suggestions article');
    function luminance(color){
      const [r,g,b]=color.match(/[\d.]+/g).map(Number).slice(0,3).map(v=>v/255).map(v=>v<=.04045?v/12.92:((v+.055)/1.055)**2.4);
      return .2126*r+.7152*g+.0722*b;
    }
    const background=luminance(getComputedStyle(card).backgroundColor);
    return ['h3','p'].map(selector=>{
      const text=luminance(getComputedStyle(card.querySelector(selector)).color);
      return (Math.max(text,background)+.05)/(Math.min(text,background)+.05);
    });
  });
  console.log({theme,contrast});
  results.push({theme,contrast});
}
assert.ok(results.every(r=>r.contrast.every(ratio=>ratio>=4.5)),JSON.stringify(results));
console.log('PASS: completed artwork titles and explanations remain readable in both themes.');
await task.finish({keep:[]});
