# Windows native components

The Windows distribution dynamically links libheif 1.23.7 and libde265 1.1.3.
The shared libraries are beside the executable; users do not need vcpkg,
Rust, Node.js or Visual Studio. The installer supplies the Microsoft C++
runtime and downloads WebView2 only when required.

Build reproducibility: `scripts/build-windows.ps1` pins the Microsoft vcpkg
checkout to `93ad765c7b66ab9ce97469ec23a90b15c3a14ee5` and enables only
`libheif[core,libde265]:x64-windows` (no GPL x265 encoder). The exact
downloaded archives and all corresponding port build files and patches are in
`Licenses/NativeSources`. The script checks each archive's SHA-512 against the
pinned port declaration. The upstream notices and Windows-specific dependency
inventory are in the installed `Licenses` directory.

Compatible replacement DLLs can be placed beside the executable for local
development or debugging. Keep a backup of the original installation. The
upstream license terms apply to the native libraries; this does not assign a
new license to Margin's own code.
