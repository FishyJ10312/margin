# Test images

`apple-synthetic.heic` is a small synthetic image encoded with the macOS HEIC encoder. It contains no personal photograph or account information. The separate `tests/example.heic` is the upstream libheif sample, documented in THIRD-PARTY.md.

Local personal test photos and generated acceptance images are excluded from Git.

`apple-10bit-ambiguous.heic` is a 1000-byte synthetic color-quadrant image produced
by `scripts/make-heic-fixtures.swift` using Apple's ImageIO encoder on 2026-11-17.
Its HEVC primary image is 10-bit but its nclx primaries and transfer are both
unspecified. It checks that high-depth input without reliable color signaling is
rejected; a copy with explicit sRGB signaling is the matching positive case.
It is not an iPhone capture and contains no personal photo or metadata.
