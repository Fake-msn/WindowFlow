# Create a minimal valid ICO file (1x1 pixel)
# ICO header: 0-1 reserved, 2-3 type (1=icon), 4-5 count
$icoHeader = [byte[]]@(0, 0, 1, 0, 1, 0)

# ICO directory entry: 0 width, 1 height, 2 colors, 3 reserved, 4-5 planes, 6-7 bpp, 8-11 size, 12-15 offset
$icoEntry = [byte[]]@(1, 1, 0, 0, 1, 0, 32, 0, 40, 0, 0, 0, 22, 0, 0, 0)

# BMP info header (40 bytes)
$bmpHeader = [byte[]]@(
    40, 0, 0, 0,  # header size
    1, 0, 0, 0,   # width
    2, 0, 0, 0,   # height (2x for XOR+AND masks)
    1, 0,         # planes
    32, 0,        # bits per pixel
    0, 0, 0, 0,   # compression
    0, 0, 0, 0,   # image size
    0, 0, 0, 0,   # X pixels per meter
    0, 0, 0, 0,   # Y pixels per meter
    0, 0, 0, 0,   # colors used
    0, 0, 0, 0    # important colors
)

# Pixel data (1 pixel, BGRA) + AND mask (padded to 4 bytes)
$pixelData = [byte[]]@(0, 0, 255, 255, 0, 0, 0, 0)

# Combine all parts
$icoData = $icoHeader + $icoEntry + $bmpHeader + $pixelData

# Write to file
[System.IO.File]::WriteAllBytes("src-tauri\icons\icon.ico", $icoData)
Write-Host "Created icon.ico"
