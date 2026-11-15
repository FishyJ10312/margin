param([string]$VcpkgRoot = $env:VCPKG_ROOT)
$ErrorActionPreference = 'Stop'
$project = Split-Path -Parent $PSScriptRoot
Set-Location $project
foreach ($tool in @('cargo', 'node', 'npm', 'git', 'cmake')) {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
        throw "Missing build tool: $tool. Install Rust MSVC, Node.js LTS, Git, CMake, and Visual Studio C++ Build Tools first."
    }
}
if (-not $VcpkgRoot) { $VcpkgRoot = Join-Path $project '.native/vcpkg' }
if (-not (Test-Path (Join-Path $VcpkgRoot 'vcpkg.exe'))) {
    if (-not (Test-Path (Join-Path $VcpkgRoot '.git'))) {
        git clone https://github.com/microsoft/vcpkg.git $VcpkgRoot
        if ($LASTEXITCODE -ne 0) { throw 'Cannot download vcpkg.' }
    }
    & (Join-Path $VcpkgRoot 'bootstrap-vcpkg.bat') -disableMetrics
    if ($LASTEXITCODE -ne 0) { throw 'Cannot bootstrap vcpkg.' }
}
$env:VCPKG_ROOT = $VcpkgRoot
$env:VCPKGRS_DYNAMIC = '1'
$env:VCPKGRS_TRIPLET = 'x64-windows'
& (Join-Path $VcpkgRoot 'vcpkg.exe') install 'libheif[core,libde265]:x64-windows' --disable-metrics
if ($LASTEXITCODE -ne 0) { throw 'HEIC decoder build failed.' }
$native = Join-Path $project '.native/windows'
New-Item -ItemType Directory -Force $native | Out-Null
$resources = @{}
Get-ChildItem (Join-Path $VcpkgRoot 'installed/x64-windows/bin') -Filter '*.dll' | ForEach-Object {
    Copy-Item $_.FullName (Join-Path $native $_.Name) -Force
    $resources["../.native/windows/$($_.Name)"] = $_.Name
}
$resources['resources/FONT-LICENSE.txt'] = 'Licenses/FONT-LICENSE.txt'
$resources['../THIRD-PARTY.md'] = 'Licenses/THIRD-PARTY.md'
Get-ChildItem (Join-Path $project 'licenses') -Filter '*.txt' | ForEach-Object {
    $resources["../licenses/$($_.Name)"] = "Licenses/$($_.Name)"
}
$config = @{ bundle = @{ targets = @('nsis'); resources = $resources; windows = @{ bundleVCRuntime = $true } } }
$configPath = Join-Path $project 'src-tauri/tauri.windows.generated.json'
$configJson = $config | ConvertTo-Json -Depth 8
[System.IO.File]::WriteAllText($configPath, $configJson, (New-Object System.Text.UTF8Encoding($false)))
$env:PATH = (Join-Path $VcpkgRoot 'installed/x64-windows/bin') + ';' + $env:PATH
npm ci
if ($LASTEXITCODE -ne 0) { throw 'npm ci failed.' }
npm run check
if ($LASTEXITCODE -ne 0) { throw 'UI type check failed.' }
cargo test --manifest-path src-tauri/Cargo.toml --lib
if ($LASTEXITCODE -ne 0) { throw 'Rust tests failed.' }
npm run tauri -- build --config src-tauri/tauri.windows.generated.json
if ($LASTEXITCODE -ne 0) { throw 'Windows installer build failed.' }
Write-Output 'Windows installer: src-tauri/target/release/bundle/nsis/'
