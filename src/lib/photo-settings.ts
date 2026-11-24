import { defaultBackground, type FrameDecoration, type RenderSettings, type WatermarkSettings } from './types';

export type ExplicitFrameEdit = Partial<Pick<RenderSettings, 'frameColor' | 'ratio' | 'artwork' | 'borderPercent' | 'decoration' | 'background'>>;
export type ExplicitSettingsEdit = Omit<ExplicitFrameEdit, 'decoration'> & {
  decoration?: Partial<FrameDecoration>;
  watermark?: Partial<WatermarkSettings>;
};
export interface PhotoEditPolicy {
  keepLayout?: boolean;
  minimumPadding?: number;
  /** An inspector edit to a watermark is visible on the entire selection.
   * Agent patches instead preserve any fields the caller did not mention. */
  matchWatermarkKind?: boolean;
}

function changed<T extends object>(before: T, after: T): Partial<T> {
  const patch: Partial<T> = {};
  for (const key of Object.keys(after) as (keyof T)[]) {
    if (JSON.stringify(before[key]) !== JSON.stringify(after[key])) patch[key] = after[key];
  }
  return patch;
}

// A delta preserves each photo's unrelated fields. Explicit actions carry their
// chosen values even if the active photo already had them.
export function photoEdit(before: RenderSettings, after: RenderSettings, explicit: ExplicitSettingsEdit, policy: PhotoEditPolicy = {}) {
  const { decoration: _decoration, watermark: _watermark, export: _export, ...ordinary } = changed(before, after);
  const { decoration: explicitDecoration, watermark: explicitWatermark, ...explicitFrame } = explicit;
  const fields = { ...ordinary, ...explicitFrame };
  const decoration = { ...changed(before.decoration, after.decoration), ...explicitDecoration };
  const watermark = { ...changed(before.watermark, after.watermark), ...explicitWatermark };
  const watermarkEdited = Object.keys(watermark).length > 0;
  return (base: RenderSettings): RenderSettings => {
    const mark = { ...base.watermark, ...watermark };
    if (policy.matchWatermarkKind && watermarkEdited && !Object.hasOwn(watermark, 'kind')) mark.kind = after.watermark.kind;
    if (mark.kind !== base.watermark.kind) {
      // Moving between watermark kinds also supplies the required content and
      // a valid size. Placement and other independent controls stay intact.
      if (!Object.hasOwn(watermark, 'sizePercent')) mark.sizePercent = after.watermark.sizePercent;
      if (mark.kind === 'text') mark.text = after.watermark.text;
      if (mark.kind === 'camera') mark.cameraTemplate = after.watermark.cameraTemplate;
      if (mark.kind === 'image') mark.imagePath = after.watermark.imagePath;
      if (mark.kind === 'text' || mark.kind === 'camera') {
        if (!Object.hasOwn(watermark, 'fontPath')) mark.fontPath = after.watermark.fontPath;
        if (!Object.hasOwn(watermark, 'fontFamily')) mark.fontFamily = after.watermark.fontFamily;
      }
    }
    const result = { ...base, ...fields, decoration: { ...base.decoration, ...decoration }, watermark: mark, export: after.export };
    if (policy.keepLayout) {
      result.ratio = base.ratio;
      result.borderPercent = Math.max(base.borderPercent, policy.minimumPadding ?? 0);
    }
    return result;
  };
}

export function frameStyleEdit(style: RenderSettings, keepLayout = false): ExplicitSettingsEdit {
  const styleFields = { frameColor: style.frameColor, decoration: style.decoration, background: style.background ?? { ...defaultBackground }, artwork: style.artwork ?? null };
  return keepLayout ? styleFields : { ...styleFields, ratio: style.ratio, borderPercent: style.borderPercent };
}

export function actualPhotoSettings(inspector: RenderSettings, shared: RenderSettings, own: RenderSettings | null | undefined, editSelected: boolean): RenderSettings {
  // The inspector edits shared settings, but the actual photo still has its
  // own composition until the user explicitly removes that override.
  return !editSelected && own ? { ...own, export: inspector.export ?? shared.export } : inspector;
}

/** Adopt only asset paths normalized by the successful native commit. Do not
 * revert unrelated newer slider edits or a newly selected file while it saves. */
export function adoptOwnedAssets(current:RenderSettings,sent:RenderSettings,committed:RenderSettings):RenderSettings {
  const map=new Map<string,string>();
  const add=(before:string|null|undefined,after:string|null|undefined)=>{
    if(before&&after&&before!==after)map.set(before,after);
  };
  add(sent.watermark.imagePath,committed.watermark.imagePath);
  add(sent.watermark.fontPath,committed.watermark.fontPath);
  add(sent.artwork?.frameImage?.imagePath,committed.artwork?.frameImage?.imagePath);
  for(let i=0;i<(sent.artwork?.stickers?.length??0);i++)add(sent.artwork?.stickers?.[i]?.imagePath,committed.artwork?.stickers?.[i]?.imagePath);
  if(!map.size)return current;
  const path=<T extends string|null|undefined>(value:T):T=>typeof value==='string'?(map.get(value)??value) as T:value;
  const next=structuredClone(current);
  next.watermark.imagePath=path(next.watermark.imagePath);
  next.watermark.fontPath=path(next.watermark.fontPath);
  if(next.artwork?.frameImage)next.artwork.frameImage.imagePath=path(next.artwork.frameImage.imagePath);
  for(const sticker of next.artwork?.stickers??[])sticker.imagePath=path(sticker.imagePath);
  return JSON.stringify(next)===JSON.stringify(current)?current:next;
}
