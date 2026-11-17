use super::*;

#[test]
fn encoder_storage_full_error_is_reported_for_jpg_and_png() {
    struct QuotaWriter(usize);
    impl Write for QuotaWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.0 == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::StorageFull,
                    "injected ENOSPC-like writer fault",
                ));
            }
            let count = self.0.min(bytes.len());
            self.0 -= count;
            Ok(count)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let pixels = RgbImage::from_pixel(16, 16, image::Rgb([120, 80, 50]));
    for format in ["jpg", "png"] {
        let mut s = RenderSettings::default();
        s.export.format = format.into();
        let mut writer = QuotaWriter(128);
        let error = encode_rgb_image(&pixels, &s, &mut writer).unwrap_err();
        assert!(error.contains("ENOSPC"), "{format}: {error}");
        assert_eq!(writer.0, 0);
        let mut healthy = Vec::new();
        encode_rgb_image(&pixels, &s, &mut healthy).unwrap();
        let decoded = image::load_from_memory(&healthy).unwrap().to_rgb8();
        assert_eq!(decoded.dimensions(), pixels.dimensions());
        assert!(decoded
            .as_raw()
            .iter()
            .zip(pixels.as_raw())
            .all(|(a, b)| a.abs_diff(*b) <= 2));
    }
}

#[test]
fn high_bit_depth_heic_requires_explicit_color_information() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("ambiguous.heic");
    fs::write(
        &source,
        include_bytes!("../../tests/fixtures/apple-10bit-ambiguous.heic"),
    )
    .unwrap();
    let s = RenderSettings::default();
    let error = preview(&source, &s, 200)
        .err()
        .expect("Unspecified 10-bit color must not be guessed");
    assert!(error.contains("颜色信息"), "{error}");
    let mut explicit = fs::read(&source).unwrap();
    let n = explicit.windows(4).position(|s| s == b"nclx").unwrap();
    explicit[n + 4..n + 6].copy_from_slice(&1u16.to_be_bytes());
    explicit[n + 6..n + 8].copy_from_slice(&13u16.to_be_bytes());
    let accepted = dir.path().join("explicit-srgb.heic");
    fs::write(&accepted, explicit).unwrap();
    assert!(preview(&accepted, &s, 200).is_ok());
}

fn with_cicp(path: &Path, values: [u8; 4]) {
    let bytes = fs::read(path).unwrap();
    let mut chunk = Vec::from(4u32.to_be_bytes());
    chunk.extend_from_slice(b"cICP");
    chunk.extend_from_slice(&values);
    let mut crc = u32::MAX;
    for b in &chunk[4..] {
        crc ^= u32::from(*b);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    chunk.extend_from_slice(&(!crc).to_be_bytes());
    // Insert after the mandatory IHDR, before any profile and image chunks.
    let mut tagged = bytes[..33].to_vec();
    tagged.extend(chunk);
    tagged.extend_from_slice(&bytes[33..]);
    fs::write(path, tagged).unwrap();
}

#[test]
fn hdr_png_is_rejected_before_preview_or_export_and_next_photo_works() {
    let dir = tempfile::tempdir().unwrap();
    let s = RenderSettings::default();
    for tc in [16, 18] {
        let path = dir.path().join(format!("hdr-{tc}.png"));
        let pixels = image::ImageBuffer::<image::Rgb<u16>, Vec<u16>>::from_pixel(
            8,
            6,
            image::Rgb([40_000, 12_000, 4_000]),
        );
        pixels.save(&path).unwrap();
        with_cicp(&path, [9, tc, 0, 1]);
        let error = preview(&path, &s, 100)
            .err()
            .expect("HDR PNG must not be silently relabelled SDR");
        assert!(error.contains("HDR"), "{error}");
        assert!(inspect_image(&path).is_err());
        let output = dir.path().join(format!("wrong-{tc}.jpg"));
        assert!(export_image(&path, &s, &output).is_err());
        assert!(!output.exists());
    }
    let normal = dir.path().join("normal.png");
    RgbImage::from_pixel(8, 6, image::Rgb([120, 80, 40]))
        .save(&normal)
        .unwrap();
    assert!(preview(&normal, &s, 100).is_ok());
}

#[test]
fn png_gamma_is_applied_without_a_higher_priority_profile() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("linear.png");
    let mut encoder = png::Encoder::new(fs::File::create(&path).unwrap(), 8, 6);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_source_gamma(png::ScaledFloat::new(1.0));
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&vec![128; 8 * 6 * 3])
        .unwrap();
    let mut s = RenderSettings::default();
    s.border_percent = 0.0;
    s.export.format = "png".into();
    let target = dir.path().join("srgb.png");
    export_image(&path, &s, &target).unwrap();
    let output = image::open(target).unwrap().to_rgb8();
    assert!(
        output
            .get_pixel(2, 2)
            .0
            .iter()
            .all(|v| v.abs_diff(188) <= 1),
        "linear 0.5 must convert to about sRGB 188, got {:?}",
        output.get_pixel(2, 2)
    );
}

#[test]
fn png_legacy_gamma_cannot_override_icc_or_srgb() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = RenderSettings::default();
    s.border_percent = 0.0;
    s.export.format = "png".into();
    for label in ["icc", "srgb"] {
        let path = dir.path().join(format!("{label}.png"));
        let mut info = png::Info::with_size(8, 6);
        info.color_type = png::ColorType::Rgb;
        info.bit_depth = png::BitDepth::Eight;
        info.gama_chunk = Some(png::ScaledFloat::new(1.0));
        if label == "icc" {
            info.icc_profile = Some(std::borrow::Cow::Owned(Profile::new_srgb().icc().unwrap()));
        } else {
            info.srgb = Some(png::SrgbRenderingIntent::RelativeColorimetric);
        }
        png::Encoder::with_info(fs::File::create(&path).unwrap(), info)
            .unwrap()
            .write_header()
            .unwrap()
            .write_image_data(&vec![128; 8 * 6 * 3])
            .unwrap();
        let output = dir.path().join(format!("{label}-out.png"));
        export_image(&path, &s, &output).unwrap();
        assert!(image::open(output)
            .unwrap()
            .to_rgb8()
            .get_pixel(1, 1)
            .0
            .iter()
            .all(|v| v.abs_diff(128) <= 1));
    }
}

#[test]
fn oversized_headers_and_sparse_files_fail_before_pixels_are_allocated() {
    let dir = tempfile::tempdir().unwrap();
    let oversized = dir.path().join("too-large.jpg");
    fs::File::create(&oversized)
        .unwrap()
        .set_len(MAX_FILE_BYTES + 1)
        .unwrap();
    assert!(inspect_image(&oversized).unwrap_err().contains("128 MB"));
    assert!(rgba_buffer(u32::MAX, u32::MAX)
        .unwrap_err()
        .contains("溢出"));
    let path = dir.path().join("huge.png");
    let encoder = png::Encoder::new(fs::File::create(&path).unwrap(), 100_000, 100_000);
    drop(encoder.write_header().unwrap());
    assert!(inspect_image(&path).is_err());
    let healthy = dir.path().join("healthy.png");
    RgbImage::from_pixel(32, 24, image::Rgb([80, 100, 120]))
        .save(&healthy)
        .unwrap();
    assert!(preview(&healthy, &RenderSettings::default(), 100).is_ok());
}

#[test]
fn png_cicp_has_priority_and_unknown_color_signals_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = RenderSettings::default();
    s.border_percent = 0.0;
    s.export.format = "png".into();
    for (name, cicp, expected) in [
        ("srgb", [1, 13, 0, 1], Some([180, 100, 80])),
        ("p3", [12, 13, 0, 1], Some([192, 94, 74])),
        ("pq", [9, 16, 0, 1], None),
        ("hlg", [9, 18, 0, 1], None),
        ("unknown", [255, 13, 0, 1], None),
        ("narrow", [1, 13, 0, 0], None),
    ] {
        let path = dir.path().join(format!("{name}.png"));
        let file = fs::File::create(&path).unwrap();
        let mut e = PngEncoder::new(file);
        e.set_icc_profile(Profile::new_srgb().icc().unwrap())
            .unwrap();
        e.write_image(
            &[180, 100, 80].repeat(48),
            8,
            6,
            image::ExtendedColorType::Rgb8,
        )
        .unwrap();
        with_cicp(&path, cicp);
        let output = dir.path().join(format!("{name}-out.png"));
        if let Some(expected) = expected {
            export_image(&path, &s, &output).unwrap();
            let pixels = image::open(output).unwrap().to_rgb8();
            let got = pixels.get_pixel(1, 1).0;
            assert!(
                got.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 2),
                "{name}: {got:?} != {expected:?}"
            );
        } else {
            assert!(export_image(&path, &s, &output).is_err(), "{name}");
            assert!(!output.exists());
        }
    }
}
