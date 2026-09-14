param(
    [string]$Destination = "target/camscan-fixtures"
)

$ErrorActionPreference = "Stop"
$sha = "8c06d742dc77ad2c6768fdc873a0755764f1bb16"
$base = "https://raw.githubusercontent.com/suhren/camscan/$sha"
$files = @{
    "LICENSE.md" = "74266B1FDF03F422903DCD953CA4F74CB92C2E49B107687445CEF3B74E222286"
    "test_scanner.py" = "213EA2196EBD1D2395040A1BD0E9DB0277C8C73EA5320F04F081A032F89FED48"
    "images/IMG_1842.jpg" = "190BD4DB84BA68C8EDAAA69E7A896D53BD167A9B17129B684367E1F802958245"
    "images/IMG_1843.jpg" = "D63FDE4E4F722CE2048474AB0A5797AD0563EE24BF0BDCF6516FE5F31322B334"
    "images/IMG_1844.jpg" = "5CF0EC62AFFD30098F1BCD928AE90B6CF4FD51BB1BF758014E4DAF69BB7AC094"
    "images/IMG_1845.jpg" = "8290E60AC0986EE70F4914B94BDF7ED149FA3695DF4F4FBAF7CAD0C555B2E445"
    "images/IMG_1846.jpg" = "6B45D6643F1E90BE4DB2115DA2279F5AFEF78E62375A8ADE90D9F33EB5D47F42"
}

foreach ($relative in $files.Keys) {
    $path = Join-Path $Destination $relative
    New-Item -ItemType Directory -Force -Path (Split-Path $path) | Out-Null
    $remoteRelative = if ($relative -eq "LICENSE.md") { $relative } else { "tests/$relative" }
    Invoke-WebRequest -Uri "$base/$remoteRelative" -OutFile $path
    $actual = (Get-FileHash $path -Algorithm SHA256).Hash
    if ($actual -ne $files[$relative]) {
        throw "SHA256 mismatch for ${relative}: expected $($files[$relative]), got $actual"
    }
}

Write-Output "Fetched Camscan fixtures at commit $sha into $Destination and verified SHA256."
