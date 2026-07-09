$ErrorActionPreference = "Stop"

$repo = if ($env:SENTRA_REPO) { $env:SENTRA_REPO } else { "flash-dev-ctrl/sentra-cli" }
$version = if ($env:SENTRA_VERSION) { $env:SENTRA_VERSION } else { "latest" }
$installDir = if ($env:SENTRA_INSTALL_DIR) { $env:SENTRA_INSTALL_DIR } else { Join-Path $env:USERPROFILE ".sentra\bin" }

function ConvertTo-PowerShellLiteral {
    param([string]$Value)

    "'" + $Value.Replace("'", "''") + "'"
}

function Start-SentraDeferredReplace {
    param(
        [string]$Source,
        [string]$Target,
        [int]$ParentPid
    )

    $deferDir = Join-Path $env:TEMP ("sentra-replace-" + [guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Force -Path $deferDir | Out-Null

    $pending = Join-Path $deferDir "sentra.exe"
    Copy-Item -Force -LiteralPath $Source -Destination $pending

    $sourceLiteral = ConvertTo-PowerShellLiteral $pending
    $targetLiteral = ConvertTo-PowerShellLiteral $Target
    $deferDirLiteral = ConvertTo-PowerShellLiteral $deferDir
    $script = @"
`$ErrorActionPreference = "Stop"
`$source = $sourceLiteral
`$target = $targetLiteral
`$deferDir = $deferDirLiteral
try {
    try {
        Wait-Process -Id $ParentPid -ErrorAction SilentlyContinue
    } catch {
    }

    `$installed = `$false
    for (`$attempt = 0; `$attempt -lt 120; `$attempt++) {
        try {
            Copy-Item -Force -LiteralPath `$source -Destination `$target
            & `$target --help | Out-Null
            `$installed = `$true
            break
        } catch {
            Start-Sleep -Milliseconds 500
        }
    }

    if (!`$installed) {
        throw "timed out replacing `$target"
    }
} finally {
    Remove-Item -Force -LiteralPath `$source -ErrorAction SilentlyContinue
    Remove-Item -Recurse -Force -LiteralPath `$deferDir -ErrorAction SilentlyContinue
}
"@
    $encoded = [Convert]::ToBase64String([System.Text.Encoding]::Unicode.GetBytes($script))
    Start-Process -FilePath "powershell" -ArgumentList @("-NoProfile", "-ExecutionPolicy", "Bypass", "-EncodedCommand", $encoded) -WindowStyle Hidden
}

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
    $deferred = $false
    try {
        Copy-Item -Force -LiteralPath $exe.FullName -Destination $target
    } catch [System.IO.IOException] {
        if (!$env:SENTRA_PARENT_PID) {
            throw
        }

        Start-SentraDeferredReplace -Source $exe.FullName -Target $target -ParentPid ([int]$env:SENTRA_PARENT_PID)
        $deferred = $true
    }

    $path = [Environment]::GetEnvironmentVariable("Path", "User")
    if (($path -split ";") -notcontains $installDir) {
        $newPath = if ([string]::IsNullOrWhiteSpace($path)) { $installDir } else { "$path;$installDir" }
        [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
        $env:Path = "$env:Path;$installDir"
        Write-Host "Added to user PATH: $installDir"
    }

    if ($deferred) {
        Write-Host "sentra update scheduled; it will complete after this command exits: $target"
    } else {
        Write-Host "sentra installed to $target"
        & $target --help | Out-Null
    }
} finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
