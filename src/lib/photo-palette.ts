/** Frame-colour suggestions taken from a photo's small catalogue thumbnail.
 * They are only suggestions: choosing one sets an exact HEX, and the eyedropper
 * remains the way to sample original pixels. */

export type Rgb = [number, number, number];

const hex = ([r, g, b]: Rgb) => '#' + [r, g, b].map(v => Math.round(v).toString(16).padStart(2, '0')).join('').toUpperCase();
const distance = (a: Rgb, b: Rgb) => (a[0] - b[0]) ** 2 + (a[1] - b[1]) ** 2 + (a[2] - b[2]) ** 2;
const luminance = ([r, g, b]: Rgb) => 0.2126 * r + 0.7152 * g + 0.0722 * b;

/** Deterministic k-means over sampled pixels. Returns up to `count` distinct
 * colours ordered dark to light. */
export function paletteFromPixels(pixels: Rgb[], count = 5): string[] {
  if (!pixels.length) return [];
  // Seed by spreading over luminance so the result is stable for the same photo.
  const sorted = [...pixels].sort((a, b) => luminance(a) - luminance(b));
  let centres: Rgb[] = Array.from({ length: Math.min(count, sorted.length) }, (_, i) =>
    [...sorted[Math.floor((i + 0.5) * sorted.length / Math.min(count, sorted.length))]] as Rgb);
  for (let round = 0; round < 8; round++) {
    const sums = centres.map(() => [0, 0, 0, 0]);
    for (const pixel of pixels) {
      let best = 0, bestDistance = Infinity;
      centres.forEach((centre, index) => { const d = distance(pixel, centre); if (d < bestDistance) { bestDistance = d; best = index; } });
      const sum = sums[best]; sum[0] += pixel[0]; sum[1] += pixel[1]; sum[2] += pixel[2]; sum[3]++;
    }
    centres = centres.map((centre, index) => sums[index][3] ? [sums[index][0] / sums[index][3], sums[index][1] / sums[index][3], sums[index][2] / sums[index][3]] as Rgb : centre);
  }
  const distinct: Rgb[] = [];
  for (const centre of centres.sort((a, b) => luminance(a) - luminance(b))) {
    if (distinct.every(other => distance(other, centre) > 18 ** 2)) distinct.push(centre);
  }
  return distinct.map(hex);
}

/** Reads a data-URL thumbnail in the browser and returns its palette. */
export async function photoPalette(source: string, count = 5): Promise<string[]> {
  if (!source || typeof document === 'undefined') return [];
  const image = new Image();
  image.decoding = 'async';
  image.src = source;
  await image.decode();
  const scale = Math.min(1, 64 / Math.max(image.naturalWidth, image.naturalHeight));
  const width = Math.max(1, Math.round(image.naturalWidth * scale)), height = Math.max(1, Math.round(image.naturalHeight * scale));
  const canvas = document.createElement('canvas');
  canvas.width = width; canvas.height = height;
  const context = canvas.getContext('2d', { willReadFrequently: true });
  if (!context) return [];
  context.drawImage(image, 0, 0, width, height);
  const data = context.getImageData(0, 0, width, height).data;
  const pixels: Rgb[] = [];
  for (let i = 0; i < data.length; i += 4) if (data[i + 3] > 200) pixels.push([data[i], data[i + 1], data[i + 2]]);
  return paletteFromPixels(pixels, count);
}
