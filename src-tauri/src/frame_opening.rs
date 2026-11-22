//! Detect the actual transparent photograph opening, independently of model promises.
//! Runs once per cached, bounded PNG; never examines the original photograph.
use crate::model::PixelRect;
use image::RgbaImage;

// Treat the entire predominantly transparent area as the opening, including
// residual alpha that image models often leave around its edge. Using only
// near-zero alpha can map that translucent rim onto the visible border.
pub(crate) const OPENING_ALPHA_THRESHOLD: u8 = 128;

pub fn detect(im: &RgbaImage) -> Option<PixelRect> {
    let (w, h) = im.dimensions();
    if w < 8 || h < 8 || w > 2048 || h > 2048 {
        return None;
    }
    let (cx, cy) = (w / 2, h / 2);
    if im.get_pixel(cx, cy)[3] >= OPENING_ALPHA_THRESHOLD {
        return None;
    }
    let count = w as usize * h as usize;
    let mut seen = Vec::new();
    seen.try_reserve_exact(count).ok()?;
    seen.resize(count, 0u8);
    let mut queue = Vec::<u32>::new();
    queue.try_reserve_exact(count).ok()?;
    let start = cy * w + cx;
    queue.push(start);
    seen[start as usize] = 1;
    let (mut x0, mut y0, mut x1, mut y1) = (cx, cy, cx, cy);
    let mut head = 0;
    while head < queue.len() {
        let p = queue[head];
        head += 1;
        let (x, y) = (p % w, p / w);
        // An open/transparent background has no unambiguous rectangular opening.
        if x == 0 || y == 0 || x == w - 1 || y == h - 1 {
            return None;
        }
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
        for next in [p - 1, p + 1, p - w, p + w] {
            if seen[next as usize] == 0 {
                seen[next as usize] = 1;
                if im.get_pixel(next % w, next / w)[3] < OPENING_ALPHA_THRESHOLD {
                    queue.push(next);
                }
            }
        }
    }
    let (rw, rh) = (x1 - x0 + 1, y1 - y0 + 1);
    if rw < w / 8 || rh < h / 8 || queue.len() as f64 / (rw as f64 * rh as f64) < 0.9 {
        return None;
    }
    Some(PixelRect {
        x: x0,
        y: y0,
        width: rw,
        height: rh,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;
    #[test]
    fn opening_is_a_closed_rectangle_not_an_opaque_or_open_background() {
        let mut im = RgbaImage::from_pixel(100, 80, Rgba([30, 40, 50, 255]));
        assert!(detect(&im).is_none());
        for y in 11..71 {
            for x in 23..79 {
                im.put_pixel(x, y, Rgba([0, 0, 0, 0]));
            }
        }
        let r = detect(&im).unwrap();
        assert_eq!((r.x, r.y, r.width, r.height), (23, 11, 56, 60));
        for y in 0..12 {
            im.put_pixel(50, y, Rgba([0, 0, 0, 0]));
        }
        assert!(detect(&im).is_none());
    }
}
