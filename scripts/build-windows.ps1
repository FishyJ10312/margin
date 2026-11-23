param([string]$VcpkgRoot, [string]$Destination = '')
$ErrorActionPreference = 'Stop'
$project = Split-Path -Parent $PSScriptRoot
Set-Location $project
foreach ($tool in @('cargo', 'node', 'npm', 'git', 'cmake', 'python')) {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
        throw "Missing build tool: $tool. Install Rust MSVC, Node.js LTS, Git, CMake, and Visual Studio C++ Build Tools first."
    }
}
# Pin the ports and their patches, not just the names of the dependencies.
$revision = '93ad765c7b66ab9ce97469ec23a90b15c3a14ee5'
if (-not $VcpkgRoot) { $VcpkgRoot = Join-Path $project '.native/vcpkg-release' }
if (-not (Test-Path $VcpkgRoot)) {
    New-Item -ItemType Directory -Force $VcpkgRoot | Out-Null
    git -C $VcpkgRoot init
    if ($LASTEXITCODE -ne 0) { throw 'Cannot initialize the pinned vcpkg checkout.' }
    git -C $VcpkgRoot remote add origin https://github.com/microsoft/vcpkg.git
    if ($LASTEXITCODE -ne 0) { throw 'Cannot set vcpkg origin.' }
    git -C $VcpkgRoot fetch --depth 1 origin $revision
    if ($LASTEXITCODE -ne 0) { throw 'Cannot download pinned vcpkg.' }
    git -C $VcpkgRoot checkout --detach $revision
    if ($LASTEXITCODE -ne 0) { throw 'Cannot checkout pinned vcpkg.' }
}
$actualRevision = git -C $VcpkgRoot rev-parse HEAD
if ($LASTEXITCODE -ne 0 -or $actualRevision -ne $revision) {
    throw 'vcpkg checkout does not match the pinned release revision. Use a fresh project-local checkout.'
}
if (-not (Test-Path (Join-Path $VcpkgRoot 'vcpkg.exe'))) {
    & (Join-Path $VcpkgRoot 'bootstrap-vcpkg.bat') -disableMetrics
    if ($LASTEXITCODE -ne 0) { throw 'Cannot bootstrap vcpkg.' }
}
$env:VCPKG_ROOT = $VcpkgRoot
$env:VCPKGRS_DYNAMIC = '1'
$env:VCPKGRS_TRIPLET = 'x64-windows'
$env:VCPKG_DOWNLOADS = Join-Path $project '.native/vcpkg-downloads'
New-Item -ItemType Directory -Force $env:VCPKG_DOWNLOADS | Out-Null
& (Join-Path $VcpkgRoot 'vcpkg.exe') install 'libheif[core,libde265]:x64-windows' --disable-metrics --binarysource=clear
if ($LASTEXITCODE -ne 0) { throw 'HEIC decoder build failed.' }
$versions = & (Join-Path $VcpkgRoot 'vcpkg.exe') list
if ($LASTEXITCODE -ne 0 -or -not ($versions -match '^libheif:x64-windows\s+1\.23\.7\s') -or -not ($versions -match '^libde265:x64-windows\s+1\.1\.3\s')) {
    throw 'The installed native versions do not match the corresponding source archives.'
}
$native = Join-Path $project '.native/windows'
New-Item -ItemType Directory -Force $native | Out-Null
$resources = @{}
$dlls = @(Get-ChildItem (Join-Path $VcpkgRoot 'installed/x64-windows/bin') -Filter '*.dll')
if (-not ($dlls.Name -match 'heif') -or -not ($dlls.Name -match 'de265')) { throw 'Expected shared HEIC decoder DLLs.' }
$dlls | ForEach-Object {
    Copy-Item $_.FullName (Join-Path $native $_.Name) -Force
    $resources["../.native/windows/$($_.Name)"] = $_.Name
}
$resources['resources/FONT-LICENSE.txt'] = 'Licenses/FONT-LICENSE.txt'
$resources['../THIRD-PARTY.md'] = 'Licenses/THIRD-PARTY.md'
$env:PATH = (Join-Path $VcpkgRoot 'installed/x64-windows/bin') + ';' + $env:PATH
npm ci
if ($LASTEXITCODE -ne 0) { throw 'npm ci failed.' }
npm run check
if ($LASTEXITCODE -ne 0) { throw 'UI type check failed.' }
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib
if ($LASTEXITCODE -ne 0) { throw 'Rust tests failed.' }
# Collect notices for the actual Windows dependency graph, not the Mac graph.
$metadata = cargo metadata --manifest-path src-tauri/Cargo.toml --locked --format-version 1 --filter-platform x86_64-pc-windows-msvc
if ($LASTEXITCODE -ne 0) { throw 'Could not collect Windows dependency inventory.' }
$metadataPath = Join-Path $native 'cargo-metadata.json'
[System.IO.File]::WriteAllText($metadataPath, ($metadata -join "`n"), (New-Object System.Text.UTF8Encoding($false)))
python scripts/collect-notices.py $metadataPath
if ($LASTEXITCODE -ne 0) { throw 'Dependency notice collection failed.' }
Get-ChildItem (Join-Path $project 'licenses') -Filter '*.txt' | ForEach-Object {
    $resources["../licenses/$($_.Name)"] = "Licenses/$($_.Name)"
}
$resources['../licenses/dependency-inventory.json'] = 'Licenses/dependency-inventory.json'
# Include exact downloaded source archives (and all port patch/build files).
$sourceDir = Join-Path $native 'NativeSources'
New-Item -ItemType Directory -Force $sourceDir | Out-Null
$nativeSources = @(
    @{ Port='libheif'; Version='1.23.7'; Sha512='e3b59c0ab6da9a86273d3f6de693d5d1fc955c84d97ca5514c58c62b6ff8ece44e81e215a126d6dbb4248b1e344f60251443c520058033fd71a7282263492180' },
    @{ Port='libde265'; Version='1.1.3'; Sha512='58eabdac466c0b59e0bc82e4d2b6d4fc9cbf6ee0af9562e36d63c0601c92c2fe21bd3dfca2a119934ee4a9dcc73e7207f6ea25dba78382752e901374896c4471' }
)
foreach ($source in $nativeSources) {
    $archive = Get-ChildItem $env:VCPKG_DOWNLOADS -File -Filter '*.tar.gz' |
        Where-Object { $_.Name -match $source.Port -and (Get-FileHash $_.FullName -Algorithm SHA512).Hash -eq $source.Sha512 } |
        Select-Object -First 1
    if (-not $archive) { throw "Missing exact downloaded source archive for $($source.Port)." }
    $archiveName = "$($source.Port)-$($source.Version)-vcpkg-source.tar.gz"
    Copy-Item $archive.FullName (Join-Path $sourceDir $archiveName) -Force
    $resources["../.native/windows/NativeSources/$archiveName"] = "Licenses/NativeSources/$archiveName"
    $portDirectory = Join-Path $VcpkgRoot "ports/$($source.Port)"
    foreach ($portFile in (Get-ChildItem $portDirectory -File -Recurse)) {
        $relativePortFile = $portFile.FullName.Substring($portDirectory.Length + 1).Replace('\','/')
        $target = Join-Path $sourceDir "vcpkg-ports/$($source.Port)/$relativePortFile"
        New-Item -ItemType Directory -Force (Split-Path -Parent $target) | Out-Null
        Copy-Item $portFile.FullName $target -Force
        $resources["../.native/windows/NativeSources/vcpkg-ports/$($source.Port)/$relativePortFile"] = "Licenses/NativeSources/vcpkg-ports/$($source.Port)/$relativePortFile"
    }
    $copyright = Join-Path $VcpkgRoot "installed/x64-windows/share/$($source.Port)/copyright"
    if (-not (Test-Path $copyright)) { throw 'Missing native license notice.' }
    Copy-Item $copyright (Join-Path $sourceDir "$($source.Port)-copyright.txt") -Force
    $resources["../.native/windows/NativeSources/$($source.Port)-copyright.txt"] = "Licenses/NativeSources/$($source.Port)-copyright.txt"
}
$resources['../docs/native-components-windows.md'] = 'Licenses/NativeSources/README.md'
$resources['../scripts/build-windows.ps1'] = 'Licenses/NativeSources/build-windows.ps1'
$config = @{ bundle = @{ targets = @('nsis'); resources = $resources; windows = @{ bundleVCRuntime = $true; webviewInstallMode = @{ type = 'downloadBootstrapper'; silent = $true } } } }
$configPath = Join-Path $project 'src-tauri/tauri.windows.generated.json'
$configJson = $config | ConvertTo-Json -Depth 8
[System.IO.File]::WriteAllText($configPath, $configJson, (New-Object System.Text.UTF8Encoding($false)))
node scripts/desktop-build.mjs --config src-tauri/tauri.windows.generated.json
if ($LASTEXITCODE -ne 0) { throw 'Windows installer build failed.' }
if ($Destination) {
    $targetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $project 'src-tauri/target' }
    $installerFiles = @(Get-ChildItem (Join-Path $targetDir 'release/bundle/nsis') -File -Filter '*.exe')
    if ($installerFiles.Count -ne 1) { throw 'Expected exactly one NSIS installer in a clean target directory.' }
    New-Item -ItemType Directory -Force $Destination | Out-Null
    $version = (Get-Content -Raw (Join-Path $project 'package.json') | ConvertFrom-Json).version
    Copy-Item $installerFiles[0].FullName (Join-Path $Destination "Margin-$version-windows-x64-setup.exe")
}
Write-Output 'Windows installer: src-tauri/target/release/bundle/nsis/'
