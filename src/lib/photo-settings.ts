import type { RenderSettings } from './types';

export type ExplicitFrameEdit = Partial<Pick<RenderSettings, 'frameColor' | 'ratio' | 'artwork' | 'borderPercent' | 'decoration'>>;

function changed<T extends object>(before: T, after: T): Partial<T> {
  const patch: Partial<T> = {};
  for (const key of Object.keys(after) as (keyof T)[]) {
    if (JSON.stringify(before[key]) !== JSON.stringify(after[key])) patch[key] = after[key];
  }
  return patch;
}

// Compute the delta once, then apply it to each photo's own settings.
export function photoEdit(before: RenderSettings, after: RenderSettings, explicit: ExplicitFrameEdit) {
  const fields = { ...changed(before, after), ...explicit };
  const decoration = explicit.decoration ?? changed(before.decoration, after.decoration);
  const watermark = changed(before.watermark, after.watermark);
  if (watermark.kind === 'text') watermark.text = after.watermark.text;
  if (watermark.kind === 'camera') watermark.cameraTemplate = after.watermark.cameraTemplate;
  if (watermark.kind === 'image') watermark.imagePath = after.watermark.imagePath;
  return (base: RenderSettings): RenderSettings => ({
    ...base, ...fields,
    decoration: { ...base.decoration, ...decoration },
    watermark: { ...base.watermark, ...watermark },
    export: after.export,
  });
}
