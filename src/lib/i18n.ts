import {derived,writable} from 'svelte/store';
import baseEnglish from './en.json';
import chatgptEnglish from './chatgpt-en.json';
import modelConfigEnglish from './model-config-en.json';
import refinementEnglish from './refinement-en.json';
import artFrameEnglish from './art-frame-en.json';
import frameStyleEnglish from './frame-style-en.json';
import importEnglish from './import-en.json';
import simpleDesignEnglish from './simple-design-en.json';
import filmStyleEnglish from './film-style-en.json';
import galleryEnglish from './gallery-en.json';
import accountEnglish from './account-en.json';
import agentEnglish from './agent-en.json';
import cameraEnglish from './camera-en.json';
import {brand} from './brand';
const english={...baseEnglish,...chatgptEnglish,...modelConfigEnglish,...refinementEnglish,...artFrameEnglish,...frameStyleEnglish,...importEnglish,...simpleDesignEnglish,...filmStyleEnglish,...galleryEnglish,...accountEnglish,...agentEnglish,...cameraEnglish};
export type Language='zh-CN'|'en';
let initial:Language='en';
try{const saved=localStorage.getItem('liubai.language');if(saved==='en'||saved==='zh-CN')initial=saved;}catch{/* Storage may be unavailable during a browser preview. */}
export const language=writable<Language>(initial);
language.subscribe(value=>{if(typeof document!=='undefined'){document.documentElement.lang=value;document.title=value==='en'?brand.englishTitle:brand.chineseTitle;}try{localStorage.setItem('liubai.language',value);}catch{/* Language remains available for this session. */}});
const reverse=Object.fromEntries(Object.entries(english).map(([k,v])=>[v,k]));
export function translate(value:string,locale:Language,params:Record<string,string|number>={}):string{
 const dictionary=english as Record<string,string>;
 let message=locale==='en'?(dictionary[value]??value):(reverse[value]??value);
 if(locale==='en'&&((value===' {0} 张'&&params['0']===1)||(value==='张照片'&&params.count===1)))message=value==='张照片'?'photo':' {0} photo';
 for(const [key,replacement] of Object.entries(params))message=message.replaceAll(`{${key}}`,String(replacement));
 if(locale==='en'&&message===value&&value.includes('：'))message=value.split('：').map(part=>dictionary[part]??part).join(': ');
 return message;
}
export const t=derived(language,locale=>(value:string,params:Record<string,string|number>={})=>translate(value,locale,params));
