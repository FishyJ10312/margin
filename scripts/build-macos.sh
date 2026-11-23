#!/bin/bash
set -euo pipefail
project_dir="$(cd "$(dirname "$0")/.." && pwd)"
native_dir="$project_dir/.native/macos"
prefix_dir="$native_dir/install"
cd "$project_dir"
mkdir -p "$native_dir"
if [ ! -f "$prefix_dir/lib/pkgconfig/libheif.pc" ]; then
  cp "$project_dir/native-sources/libde265-1.1.3.tar.gz" "$native_dir/libde265.tar.gz"
  cp "$project_dir/native-sources/libheif-1.23.7.tar.gz" "$native_dir/libheif.tar.gz"
  echo "554228bd17788c99a7e63b37ab5634722190e6e2bf60c1dcb01cef328e133905  $native_dir/libde265.tar.gz" | shasum -a 256 -c -
  echo "f7483e818503ea6224c8863e4a56ac02c7089021a98472c811c12aa19faf2475  $native_dir/libheif.tar.gz" | shasum -a 256 -c -
  tar -xzf "$native_dir/libde265.tar.gz" -C "$native_dir"
  tar -xzf "$native_dir/libheif.tar.gz" -C "$native_dir"
  cmake -S "$native_dir/libde265-1.1.3" -B "$native_dir/de265-build" -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX="$prefix_dir" -DCMAKE_OSX_DEPLOYMENT_TARGET=12.0 -DENABLE_DECODER=OFF -DENABLE_ENCODER=OFF -DENABLE_SDL=OFF
  cmake --build "$native_dir/de265-build" -j "${MARGIN_NATIVE_JOBS:-4}"
  cmake --install "$native_dir/de265-build"
  PKG_CONFIG_PATH="$prefix_dir/lib/pkgconfig" cmake -S "$native_dir/libheif-1.23.7" -B "$native_dir/heif-build" -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX="$prefix_dir" -DCMAKE_PREFIX_PATH="$prefix_dir" -DCMAKE_OSX_DEPLOYMENT_TARGET=12.0 -DWITH_LIBDE265=ON -DWITH_LIBDE265_PLUGIN=OFF -DWITH_X265=OFF -DWITH_AOM_DECODER=OFF -DWITH_AOM_ENCODER=OFF -DWITH_X264=OFF -DWITH_OpenH264_DECODER=OFF -DWITH_LIBSHARPYUV=OFF -DWITH_DAV1D=OFF -DWITH_FFMPEG_DECODER=OFF -DWITH_JPEG_DECODER=OFF -DWITH_JPEG_ENCODER=OFF -DWITH_KVAZAAR=OFF -DWITH_OPENJPH_ENCODER=OFF -DWITH_OpenJPEG_DECODER=OFF -DWITH_OpenJPEG_ENCODER=OFF -DWITH_RAV1E=OFF -DWITH_SvtEnc=OFF -DWITH_UVG266=OFF -DWITH_VVDEC=OFF -DWITH_VVENC=OFF -DWITH_WEBCODECS=OFF -DWITH_UNCOMPRESSED_CODEC=OFF -DWITH_HEADER_COMPRESSION=OFF -DENABLE_PLUGIN_LOADING=OFF -DWITH_EXAMPLES=OFF -DWITH_GDK_PIXBUF=OFF -DBUILD_TESTING=OFF
  cmake --build "$native_dir/heif-build" -j "${MARGIN_NATIVE_JOBS:-4}"
  cmake --install "$native_dir/heif-build"
fi
export PKG_CONFIG_PATH="$prefix_dir/lib/pkgconfig:${PKG_CONFIG_PATH:-}"
export MACOSX_DEPLOYMENT_TARGET=12.0
export DYLD_FALLBACK_LIBRARY_PATH="$prefix_dir/lib:${DYLD_FALLBACK_LIBRARY_PATH:-}"
npm ci
npm run check
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib
target_triple="$(rustc -vV | awk '/^host:/{print $2}')"
cargo metadata --manifest-path src-tauri/Cargo.toml --locked --format-version 1 --filter-platform "$target_triple" > "$native_dir/cargo-metadata.json"
python3 scripts/collect-notices.py "$native_dir/cargo-metadata.json"
node scripts/desktop-build.mjs --bundles app
product_name="$(python3 -c 'import json; print(json.load(open("src-tauri/tauri.conf.json"))["productName"])')"
python3 scripts/package-macos.py "src-tauri/target/release/bundle/macos/$product_name.app" --native-dir "$prefix_dir/lib" --native-sources "$native_dir" --destination "${1:-$project_dir/deliverables}"
