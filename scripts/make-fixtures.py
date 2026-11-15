"""Create synthetic, reproducible QA photos without distributing personal images."""
from pathlib import Path
import hashlib
import json
from PIL import Image, ImageDraw, ImageCms

root = Path(__file__).resolve().parents[1] / 'tests' / 'generated'
(root / '照片').mkdir(parents=True, exist_ok=True)
(root / '另一文件夹').mkdir(exist_ok=True)
srgb = ImageCms.ImageCmsProfile(ImageCms.createProfile('sRGB')).tobytes()
paths = []
for i in range(50):
    size = (640, 420) if i % 2 == 0 else (420, 640)
    if i == 0:
        size = (4000, 3000)
    im = Image.new('RGB', size, (35 + i * 3, 85 + i, 110 + i * 2))
    draw = ImageDraw.Draw(im)
    draw.rectangle((0, 0, size[0] // 3, size[1] // 3), fill=(232, 75, 54))
    draw.rectangle((size[0] * 2 // 3, size[1] * 2 // 3, size[0], size[1]), fill=(242, 216, 94))
    path = root / '照片' / f'照片_{i + 1:02d}.jpg'
    kwargs = {'quality': 90}
    if i % 3 == 0:
        kwargs['icc_profile'] = srgb
    if i == 2:
        exif = Image.Exif()
        exif[274] = 6
        kwargs['exif'] = exif
    im.save(path, **kwargs)
    paths.append(path)

Image.new('RGBA', (440, 300), (32, 140, 98, 100)).save(root / '照片' / '透明照片.png')
paths.append(root / '照片' / '透明照片.png')
Image.new('RGB', (500, 300), 'orange').save(root / '另一文件夹' / '照片_01.jpg')
paths.append(root / '另一文件夹' / '照片_01.jpg')
(root / '照片' / '损坏.jpg').write_bytes(b'not a jpeg\x00\xff')
(root / '照片' / '说明.txt').write_text('This is not a photo.', encoding='utf-8')
(root / '照片' / '子目录').mkdir(exist_ok=True)
watermark = Image.new('RGBA', (320, 100), (0, 0, 0, 0))
wd = ImageDraw.Draw(watermark)
wd.rounded_rectangle((4, 8, 94, 92), 8, fill=(255, 255, 255, 200))
wd.rectangle((120, 30, 310, 43), fill=(255, 255, 255, 220))
wd.rectangle((120, 60, 260, 68), fill=(255, 255, 255, 140))
watermark.save(root / '水印.png')
manifest = {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest() for p in paths}
(root / 'original-hashes.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding='utf-8')
print(json.dumps({'directory': str(root), 'validPhotos': len(paths)}, ensure_ascii=False))
