$ErrorActionPreference = "Stop"

$repo = if ($env:SENTRA_REPO) { $env:SENTRA_REPO } else { "flash-dev-ctrl/sentra-cli" }
$version = if ($env:SENTRA_VERSION) { $env:SENTRA_VERSION } else { "latest" }
$installDir = if ($env:SENTRA_INSTALL_DIR) { $env:SENTRA_INSTALL_DIR } else { Join-Path $env:USERPROFILE ".sentra\bin" }

$arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString().ToLowerInvariant()
switch ($arch) {
    "x64" { $asset = "sentra-windows-x86_64-static.zip" }
    "x86" { throw "unsupported Windows architecture: x86" }
    "arm64" { throw "unsupported Windows architecture: arm64" }
    default { throw "unsupported Windows architecture: $arch" }
}

if ($version -eq "latest") {
    $url = "https://github.com/$repo/releases/latest/download/$asset"
} else {
    $url = "https://github.com/$repo/releases/download/$version/$asset"
}

$tmp = Join-Path $env:TEMP ("sentra-install-" + [guid]::NewGuid().ToString("N"))
$extract = Join-Path $tmp "extract"
$zip = Join-Path $tmp $asset
New-Item -ItemType Directory -Force -Path $tmp, $extract, $installDir | Out-Null

try {
    Invoke-WebRequest -Uri $url -OutFile $zip
    Expand-Archive -Force -Path $zip -DestinationPath $extract

    $exe = Get-ChildItem -Path $extract -Recurse -Filter sentra.exe | Select-Object -First 1
    if (!$exe) {
        throw "sentra.exe not found in $asset"
    }

    $target = Join-Path $installDir "sentra.exe"
    Copy-Item -Force $exe.FullName $target

    $path = [Environment]::GetEnvironmentVariable("Path", "User")
    if (($path -split ";") -notcontains $installDir) {
        $newPath = if ([string]::IsNullOrWhiteSpace($path)) { $installDir } else { "$path;$installDir" }
        [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
        $env:Path = "$env:Path;$installDir"
        Write-Host "Added to user PATH: $installDir"
    }

    Write-Host "sentra installed to $target"
    & $target --help | Out-Null
} finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
