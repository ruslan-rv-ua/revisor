<#
.SYNOPSIS
    Regenerates crates/revisor/resources/app.ico from crates/revisor/resources/icon.svg.

.DESCRIPTION
    The icon is **two assets from one source design**: the SVG the main window embeds, and
    the multi-resolution .ico the exe carries as a resource. This script is the second half
    of that sentence — the .ico is a build input and is committed, but it is never hand-drawn,
    so the two assets cannot drift into two designs.

    Sizes are 16 / 24 / 32 / 48 / 256, 32-bit BGRA with alpha, the 256 layer PNG-compressed
    per Microsoft's app-icon guidance (Windows scales *down* from the next size up, which is
    why 256 has to be there).

    The ICO is assembled here rather than by ImageMagick because IM's ICO writer rejects a
    256 px layer outright (`InvalidDimensions`, icon.c) — it will write 255 and no more. The
    format is small enough to write correctly: a 6-byte ICONDIR, one 16-byte ICONDIRENTRY per
    layer, then either a headerless BMP (BITMAPINFOHEADER with a doubled height, bottom-up
    BGRA, and an all-zero AND mask — alpha carries transparency) or, for 256, the PNG file
    verbatim with its width and height fields written as 0, which is how 256 is spelled in a
    single byte.

.NOTES
    Needs ImageMagick 7 on PATH (`magick`), built with librsvg — `magick -list format` must
    show SVG as RSVG rather than MSVG, or the rasterisation is ImageMagick's own renderer and
    the result will not match what the frame's nanosvg draws.
#>
[CmdletBinding()]
param(
    [string]$Svg = (Join-Path $PSScriptRoot '..\crates\revisor\resources\icon.svg'),
    [string]$Ico = (Join-Path $PSScriptRoot '..\crates\revisor\resources\app.ico')
)

$ErrorActionPreference = 'Stop'
$sizes = 16, 24, 32, 48, 256

if (-not (Get-Command magick -ErrorAction SilentlyContinue)) {
    throw 'ImageMagick 7 (magick) is not on PATH'
}
$svgPath = (Resolve-Path $Svg).Path
$staging = Join-Path ([System.IO.Path]::GetTempPath()) ("revisor-icon-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $staging | Out-Null

try {
    # One rasterisation per size, from the SVG each time rather than by downscaling a big
    # PNG: 16 px wants its own pass over the vectors, not a resample of 256.
    $layers = foreach ($size in $sizes) {
        $png = Join-Path $staging "$size.png"
        # -density 384 renders the 256-unit viewBox at 1024 px before the resize, so small
        # sizes come off a supersampled render instead of a 1:1 one.
        & magick -background none -density 384 $svgPath -resize "${size}x${size}" PNG32:$png
        if ($LASTEXITCODE -ne 0) { throw "magick failed to rasterise $size px" }

        if ($size -eq 256) {
            # Kept as PNG, bytes and all: this is the layer the format compresses.
            [pscustomobject]@{ Size = $size; Png = $true; Bytes = [IO.File]::ReadAllBytes($png) }
        }
        else {
            # -flip because a BMP inside an ICO is stored bottom-up and raw BGRA comes out
            # of ImageMagick top-down.
            $raw = Join-Path $staging "$size.bgra"
            & magick $png -depth 8 -alpha on -flip BGRA:$raw
            if ($LASTEXITCODE -ne 0) { throw "magick failed to write BGRA for $size px" }
            $pixels = [IO.File]::ReadAllBytes($raw)
            if ($pixels.Length -ne $size * $size * 4) {
                throw "$size px: expected $($size * $size * 4) BGRA bytes, got $($pixels.Length)"
            }

            # The AND mask is legacy and all-zero — every pixel "opaque", with the real
            # transparency in the alpha channel — but its rows are still padded to 4 bytes
            # and its height still has to be there, or the layer is silently misread.
            $maskStride = [math]::Ceiling($size / 8.0)
            $maskStride = [int]([math]::Ceiling($maskStride / 4.0) * 4)
            $mask = New-Object byte[] ($maskStride * $size)

            # One stream, not three arrays joined with `+`: joining byte[] in PowerShell
            # yields an Object[], and BinaryWriter.Write then binds the *bool* overload and
            # writes a single 0x01 byte per layer — a malformed icon and a green run.
            $bmp = New-Object System.IO.MemoryStream
            $w = New-Object System.IO.BinaryWriter($bmp)
            $w.Write([uint32]40)          # biSize
            $w.Write([int32]$size)        # biWidth
            $w.Write([int32]($size * 2))  # biHeight: XOR image and AND mask stacked
            $w.Write([uint16]1)           # biPlanes
            $w.Write([uint16]32)          # biBitCount
            $w.Write([uint32]0)           # biCompression = BI_RGB
            $w.Write([uint32]($pixels.Length + $mask.Length)) # biSizeImage
            $w.Write([int32]0); $w.Write([int32]0)            # pixels-per-metre
            $w.Write([uint32]0); $w.Write([uint32]0)          # palette counts
            $w.Write($pixels, 0, $pixels.Length)
            $w.Write($mask, 0, $mask.Length)
            $w.Flush()

            [pscustomobject]@{ Size = $size; Png = $false; Bytes = $bmp.ToArray() }
        }
    }

    $out = New-Object System.IO.MemoryStream
    $writer = New-Object System.IO.BinaryWriter($out)
    $writer.Write([uint16]0)             # reserved
    $writer.Write([uint16]1)             # type 1 = icon
    $writer.Write([uint16]$layers.Count)

    $offset = 6 + 16 * $layers.Count
    foreach ($layer in $layers) {
        # 0 is how 256 is written in a byte-wide field.
        $dimension = if ($layer.Size -ge 256) { 0 } else { $layer.Size }
        $writer.Write([byte]$dimension)  # bWidth
        $writer.Write([byte]$dimension)  # bHeight
        $writer.Write([byte]0)           # bColorCount: 0 for anything past 8 bpp
        $writer.Write([byte]0)           # bReserved
        $writer.Write([uint16]1)         # wPlanes
        $writer.Write([uint16]32)        # wBitCount
        $writer.Write([uint32]$layer.Bytes.Length)
        $writer.Write([uint32]$offset)
        $offset += $layer.Bytes.Length
    }
    foreach ($layer in $layers) { $writer.Write($layer.Bytes, 0, $layer.Bytes.Length) }
    $writer.Flush()

    [IO.File]::WriteAllBytes((New-Item -ItemType File -Path $Ico -Force).FullName, $out.ToArray())
    $written = Get-Item $Ico
    Write-Host "$($written.FullName): $($layers.Count) layers ($($sizes -join '/') px), $($written.Length) bytes"
}
finally {
    Remove-Item -Recurse -Force $staging -ErrorAction SilentlyContinue
}
