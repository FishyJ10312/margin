import type {FrameSuggestion,RenderSettings,WatermarkSettings} from './types';

/** Bundled serif-style face used for gallery captions, when the app has it. */
export const captionFont=(families:string[])=>families.find(f=>/ZCOOL|站酷/.test(f))??null;

/**
 * The watermark a directed suggestion would set: its caption, centred in the
 * lower border in the style's ink. A watermark the person already uses always
 * wins, so applying a frame never replaces their own signature.
 */
export function captionWatermark(base:WatermarkSettings,s:Pick<FrameSuggestion,'caption'|'artwork'>,fontFamily:string|null=null):WatermarkSettings{
 const caption=s.caption?.trim();
 if(!caption||base.kind!=='none')return base;
 return {...base,kind:'text',text:caption,region:'border',position:'bottom-center',
  color:s.artwork.design?.colors?.[0]??'#3B3833',opacity:0.88,sizePercent:2.2,marginPercent:2,
  offsetX:0,offsetY:0,fontFamily:fontFamily??base.fontFamily,fontPath:fontFamily?null:base.fontPath};
}

/** Render settings for previewing or applying a suggestion. */
export function suggestionSettings(base:RenderSettings,s:FrameSuggestion,fontFamily:string|null=null):RenderSettings{
 return {...base,frameColor:s.frameColor,borderPercent:s.borderPercent,decoration:s.decoration,artwork:s.artwork,
  watermark:captionWatermark(base.watermark,s,fontFamily)};
}
