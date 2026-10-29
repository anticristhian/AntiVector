$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path $PSScriptRoot -Parent
$directory = Join-Path $taskRoot 'fuzz/corpus/decode_pipeline'
New-Item -ItemType Directory -Force -Path $directory | Out-Null
[System.IO.File]::WriteAllBytes((Join-Path $directory 'rgba'),[byte[]](1,7,7,255,0,0,255,0,255,0,255,0,0,255,255))
foreach ($seed in @(
    @{name='png';prefix=0;path='corpus/images/logo_original_1.png'},
    @{name='project';prefix=2;path='corpus/baselines/session/f8/f8-native.avproj'}
)) {
    $bytes = [System.IO.File]::ReadAllBytes((Join-Path $taskRoot $seed.path))
    [System.IO.File]::WriteAllBytes((Join-Path $directory $seed.name),[byte[]](@($seed.prefix)+$bytes))
}
