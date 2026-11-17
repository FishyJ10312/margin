# Native image components

The Mac app dynamically links unmodified libheif 1.23.7 and libde265 1.1.3.
Their upstream source archives, SHA256 checksums, LGPL texts and this build script
are included in `Margin.app/Contents/Resources/Licenses/NativeSources`.
The original project source is delivered separately with the same version number.

- libheif source: https://github.com/strukturag/libheif/releases/tag/v1.23.7
- libde265 source: https://github.com/strukturag/libde265/releases/tag/v1.1.3
- Full notices and exact dependency inventory: the parent `Licenses` directory.

`scripts/build-macos.sh` in the project verifies the source archives before
compiling the shared libraries. The HEVC decoder is libde265; plugin loading,
encoders and unused decoder backends are disabled. No native source modifications
are applied. CMake, a C/C++ compiler, pkg-config, Rust and Node are development
requirements; end users do not need them to run the app.

Shared libraries live in `Contents/Frameworks`, with executable-relative lookup.
For local development, compatible replacement dylibs can be placed there and the
app ad-hoc signed again with `codesign --force --deep --sign - Margin.app`.
Keep the original copy before replacing libraries. Modifying a distributed app
invalidates its existing signature; a local development signature is not a
Developer ID signature or Apple notarization.

The upstream license texts describe the native components' terms. This document
records the supplied materials and build process; it does not assign a new license
to Margin's own source code or promise a completed distribution-rights review.
