import type {RenderSettings,FontOption} from './types';
import type {ExplicitSettingsEdit} from './photo-settings';
export interface AgentStatus {enabled:boolean;allowWrites:boolean;endpoint:string|null;toolCount:number;activeRequests:number;lastTool:string|null;clientJson:string;codexToml:string}
export interface AgentCommand {requestId:string;name:string;arguments:Record<string,unknown>}

const keys={
  ratio:['width','height'],
  decoration:['kind','color','opacity','scale'],
  watermark:['kind','text','imagePath','fontPath','fontFamily','offsetX','offsetY','color','opacity','sizePercent','region','position','marginPercent','cameraTemplate'],
  export:['format','jpegQuality','longEdge','namePattern','keepCameraInfo'],
};
const top=['ratio','borderPercent','frameColor','decoration','watermark','export'];

export function record(value:unknown):Record<string,unknown>{
  if(!value||typeof value!=='object'||Array.isArray(value))throw new Error('Expected an object');
  return value as Record<string,unknown>;
}

export function patchAgentSettings(current:RenderSettings,value:unknown,fonts:FontOption[]=[]):RenderSettings{
  const patch=record(value),next=structuredClone(current);
  if(!Object.keys(patch).length)throw new Error('Settings patch is empty');
  for(const [key,value] of Object.entries(patch)){
    if(!top.includes(key))throw new Error('Unsupported settings field: '+key);
    if(key==='ratio'&&value===null){next.ratio=null;continue;}
    if(key==='borderPercent'||key==='frameColor'){
      (next as unknown as Record<string,unknown>)[key]=value;continue;
    }
    const section=key as keyof typeof keys,fields=record(value);
    const base=(next as unknown as Record<string,Record<string,unknown>|null>)[key]??{};
    const merged={...base};
    for(const [field,entry] of Object.entries(fields)){
      if(!keys[section].includes(field))throw new Error('Unsupported settings field: '+key+'.'+field);
      merged[field]=entry;
    }
    (next as unknown as Record<string,unknown>)[key]=merged;
  }
  const watermark=patch.watermark?record(patch.watermark):null;
  if(watermark&&typeof watermark.fontFamily==='string'&&!Object.hasOwn(watermark,'fontPath')){
    const font=fonts.find(font=>font.family===watermark.fontFamily);
    if(!font)throw new Error('Font family is unavailable; use margin_list_fonts');
    next.watermark.fontPath=font.path||null;
  }
  if(watermark?.imagePath!=null){if(watermark.imagePath==='')next.watermark.imagePath=null;else if(!absoluteLocalPath(watermark.imagePath).toLowerCase().endsWith('.png'))throw new Error('Watermark image must be a local PNG');}
  if(watermark?.fontPath!=null){if(watermark.fontPath==='')next.watermark.fontPath=null;else if(!(/\.(?:otf|ttf|ttc|otc)$/i).test(absoluteLocalPath(watermark.fontPath)))throw new Error('Font must be an OTF, TTF, TTC or OTC file');}
  return next;
}

/** Preserve the intent of the validated patch instead of diffing only the
 * active photo. Identical values still apply to the other selected photos. */
export function explicitAgentEdit(value:unknown,next:RenderSettings):ExplicitSettingsEdit {
  const patch=record(value),explicit:ExplicitSettingsEdit={};
  if(Object.hasOwn(patch,'ratio'))explicit.ratio=next.ratio;
  if(Object.hasOwn(patch,'borderPercent'))explicit.borderPercent=next.borderPercent;
  if(Object.hasOwn(patch,'frameColor'))explicit.frameColor=next.frameColor;
  const pick=<T extends object>(fields:Record<string,unknown>,normalized:T):Partial<T>=>{
    const result:Partial<T>={};
    for(const key of Object.keys(fields) as (keyof T)[])result[key]=normalized[key];
    return result;
  };
  if(patch.decoration)explicit.decoration=pick(record(patch.decoration),next.decoration);
  if(patch.watermark){
    const fields=record(patch.watermark);
    explicit.watermark=pick(fields,next.watermark);
    if(Object.hasOwn(fields,'fontFamily'))explicit.watermark.fontPath=next.watermark.fontPath;
  }
  return explicit;
}

export function absoluteLocalPath(value:unknown):string{
  if(typeof value!=='string'||value.includes('\0')||!(/^(?:\/|[A-Za-z]:[\\/]|\\\\[^\\])/).test(value))throw new Error('Use an absolute local path');
  return value;
}

export function catalogIds(value:unknown,known:string[]):string[]{
  if(!Array.isArray(value)||value.some(id=>typeof id!=='string'))throw new Error('Photo IDs must be an array');
  const allowed=new Set(known),wanted=new Set(value as string[]);
  if([...wanted].some(id=>!allowed.has(id)))throw new Error('Some photo IDs are not in the current catalog; refresh the workspace');
  return known.filter(id=>wanted.has(id));
}

export const readOnlyAgentTools=new Set(['margin_get_workspace','margin_get_preview','margin_get_export_status','margin_list_fonts','margin_list_templates']);
