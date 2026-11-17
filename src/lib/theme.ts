import {get,writable} from 'svelte/store';

/** Interface theme. "system" follows the macOS / Windows appearance. */
export type ThemeChoice='dark'|'light'|'system';
const key='margin.theme';
function stored():ThemeChoice{
  try{const value=localStorage.getItem(key);if(value==='dark'||value==='light'||value==='system')return value;}catch{/* Storage may be unavailable; keep the default. */}
  return 'dark';
}
const media=typeof matchMedia==='function'?matchMedia('(prefers-color-scheme: dark)'):null;
function apply(choice:ThemeChoice){
  if(typeof document==='undefined')return;
  const resolved=choice==='system'?(media?.matches===false?'light':'dark'):choice;
  document.documentElement.dataset.theme=resolved;
  document.querySelector('meta[name="theme-color"]')?.setAttribute('content',resolved==='dark'?'#121212':'#f7f4ee');
}
export const theme=writable<ThemeChoice>(stored());
theme.subscribe(choice=>{apply(choice);try{localStorage.setItem(key,choice);}catch{/* The choice still applies to this session. */}});
media?.addEventListener?.('change',()=>apply(get(theme)));
