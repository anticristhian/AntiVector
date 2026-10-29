param([string]$CorpusDirectory='fuzz/corpus/decode_pipeline')
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path $PSScriptRoot -Parent
$directory = Join-Path $taskRoot $CorpusDirectory
New-Item -ItemType Directory -Force -Path $directory | Out-Null
[System.IO.File]::WriteAllBytes((Join-Path $directory 'rgba'),[byte[]](1,7,7,255,0,0,255,0,255,0,255,0,0,255,255))
foreach ($seed in @(
    @{name='png';prefix=0;path='corpus/images/logo_original_1.png'},
    @{name='project';prefix=2;path='corpus/baselines/session/f8/f8-native.avproj'},
    @{name='circle-optin';prefix=180;path='corpus/images/circle_r50.png'},
    @{name='logo-optin';prefix=180;path='corpus/images/logo_original_1.png'}
)) {
    $bytes = [System.IO.File]::ReadAllBytes((Join-Path $taskRoot $seed.path))
    [System.IO.File]::WriteAllBytes((Join-Path $directory $seed.name),[byte[]](@($seed.prefix)+$bytes))
}
$structured=New-Object byte[] 1027
$structured[0]=244;$structured[1]=15;$structured[2]=15
for($i=3;$i -lt $structured.Length;$i++){$structured[$i]=[byte](($i*73+19)%256)}
[System.IO.File]::WriteAllBytes((Join-Path $directory 'opaque-structured'),$structured)
