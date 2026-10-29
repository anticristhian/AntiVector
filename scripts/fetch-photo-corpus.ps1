$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path $PSScriptRoot -Parent
$destination = Join-Path $taskRoot 'corpus/sources/scikit-image-v0.25.2'
New-Item -ItemType Directory -Force -Path $destination | Out-Null
$assets = @(
    @{name='camera'; hash='b0793d2adda0fa6ae899c03989482bff9a42d3d5690fc7e3648f2795d730c23a'; author='Lav Varshney'; license='CC0-1.0'},
    @{name='coffee'; hash='cc02f8ca188b167c775a7101b5d767d1e71792cf762c33d6fa15a4599b5a8de7'; author='Rachel Michetti'; license='CC0-1.0'},
    @{name='chelsea'; hash='596aa1e7cb875eb79f437e310381d26b338a81c2da23439704a73c4651e8c4bb'; author='Stefan van der Walt'; license='CC0-1.0'},
    @{name='brick'; hash='7966caf324f6ba843118d98f7a07746d22f6a343430add0233eca5f6eaaa8fcf'; author='CC0Textures, Bricks25'; license='CC0-1.0'},
    @{name='astronaut'; hash='88431cd9653ccd539741b555fb0a46b61558b301d4110412b5bc28b5e3ea6cb5'; author='NASA'; license='Public domain'}
)
foreach ($asset in $assets) {
    $url = 'https://raw.githubusercontent.com/scikit-image/scikit-image/v0.25.2/skimage/data/' + $asset.name + '.png'
    $path = Join-Path $destination ($asset.name + '.png')
    if (!(Test-Path -LiteralPath $path)) { Invoke-WebRequest -Uri $url -OutFile $path }
    $actual = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $asset.hash) { throw ('SHA256 mismatch: ' + $asset.name) }
    $asset.url = $url
    $asset.license_source = 'https://github.com/scikit-image/scikit-image/blob/v0.25.2/skimage/data/_fetchers.py'
    Write-Output ($asset.name + ': SHA256 verified')
}
$assets | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $destination 'manifest.json') -Encoding utf8
