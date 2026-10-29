$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path $PSScriptRoot -Parent
$metadata = Get-Content (Join-Path $taskRoot 'target/f9-metadata.json') -Raw | ConvertFrom-Json
$notices = Get-Content (Join-Path $taskRoot 'app/src-tauri/notices/manifest.json') -Raw | ConvertFrom-Json
$missing = @($notices.missing_texts) + @($notices.packages | Where-Object { $_.texts.file -like 'upstream/*' } | ForEach-Object { $_.name+'@'+$_.version })
$missing = $missing | Sort-Object -Unique
$cache = @{}
$provenance = @()
foreach ($id in $missing) {
    $name, $version = $id.Substring(6).Split('@',2)
    $pkg = $metadata.packages | Where-Object { $_.name -eq $name -and $_.version -eq $version }
    if ($name -eq 'selectors') {
        $url = 'https://www.mozilla.org/media/MPL/2.0/index.txt'
        $relative = 'third-party/licenses/selectors-' + $version + '/LICENSE-MPL-2.0.txt'
        $path = Join-Path $taskRoot $relative
        New-Item -ItemType Directory -Force -Path (Split-Path $path) | Out-Null
        Invoke-WebRequest -Uri $url -OutFile $path
        $provenance += @{package=$id;url=$url;file=$relative;sha256=(Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()}
        $provenance | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $taskRoot 'third-party/license-sources.json') -Encoding utf8
        continue
    }
    if ($name -like 'winapi-*-pc-windows-gnu') {
        # These old packages omit VCS metadata and license files. The companion
        # winapi0.3.9 revision still declares both target subpackages as0.4.0.
        $vcs = @{git=@{sha1='796a8e6c2971dc2ff1bcff166e6671284f9b5b6b'};path_in_vcs=''}
    } else {
        $vcs = Get-Content (Join-Path (Split-Path $pkg.manifest_path) '.cargo_vcs_info.json') -Raw | ConvertFrom-Json
    }
    $repo = $pkg.repository
    if ($name -eq 'libappindicator-sys') { $repo = 'https://github.com/tauri-apps/libappindicator-rs' }
    if ($repo -notmatch '^https://github.com/([^/]+/[^/]+?)(\.git)?/?$') { throw ('Unsupported repository: ' + $repo) }
    $repository = $Matches[1]
    $sha = $vcs.git.sha1
    $key = $repository + '/' + $sha
    if (!$cache.ContainsKey($key)) {
        $tree = Invoke-RestMethod -Uri ('https://api.github.com/repos/' + $repository + '/git/trees/' + $sha + '?recursive=1') -Headers @{'User-Agent'='AntiVector-license-audit'}
        if ($tree.truncated) { throw ('Truncated repository tree: ' + $repository) }
        $cache[$key] = $tree.tree
    }
    $ancestors = @('')
    $prefix = $vcs.path_in_vcs
    while ($prefix) { $ancestors += $prefix.TrimEnd('/') + '/'; if ($prefix -notmatch '/') { break }; $prefix=$prefix.Substring(0,$prefix.LastIndexOf('/')) }
    $files = $cache[$key] | Where-Object { $_.type -eq 'blob' -and $_.mode -ne '120000' -and $_.path -match '(^|/)(LICENSE|LICENCE|COPYING|NOTICE|AUTHORS)([._-].*)?$' -and $ancestors.Contains(($_.path -replace '[^/]+$','')) }
    if (!$files) { throw ('No authoritative license text found: ' + $id) }
    $destination = Join-Path $taskRoot ('third-party/licenses/' + $name + '-' + $version)
    New-Item -ItemType Directory -Force -Path $destination | Out-Null
    foreach ($file in $files) {
        $url = 'https://raw.githubusercontent.com/' + $repository + '/' + $sha + '/' + $file.path
        $filename = $file.path.Replace('/','__')
        $path = Join-Path $destination $filename
        if (!(Test-Path -LiteralPath $path)) { Invoke-WebRequest -Uri $url -OutFile $path }
        $provenance += @{package=$id; url=$url; file=('third-party/licenses/'+$name+'-'+$version+'/'+$filename); sha256=(Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()}
        $provenance | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $taskRoot 'third-party/license-sources.json') -Encoding utf8
    }
    Write-Output ($id + ': ' + $files.Count + ' license files')
}
$provenance | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $taskRoot 'third-party/license-sources.json') -Encoding utf8
