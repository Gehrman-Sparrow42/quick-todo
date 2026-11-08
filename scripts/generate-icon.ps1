# Rebuild resources/quick-todo.ico from the geometric artwork, using only Windows.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$root = Split-Path -Parent $PSScriptRoot
$sizes = @(16, 20, 24, 32, 48, 64, 128, 256)
$images = @()
foreach ($size in $sizes) {
    $bitmap = New-Object System.Drawing.Bitmap $size, $size
    $g = [System.Drawing.Graphics]::FromImage($bitmap)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.ScaleTransform($size / 256.0, $size / 256.0)
    $teal = New-Object System.Drawing.SolidBrush ([System.Drawing.ColorTranslator]::FromHtml('#155f55'))
    $paper = New-Object System.Drawing.SolidBrush ([System.Drawing.ColorTranslator]::FromHtml('#fffdf8'))
    $fold = New-Object System.Drawing.SolidBrush ([System.Drawing.ColorTranslator]::FromHtml('#c7e1d7'))
    $shape = New-Object System.Drawing.Drawing2D.GraphicsPath
    $shape.AddArc(12,12,108,108,180,90); $shape.AddArc(136,12,108,108,270,90); $shape.AddArc(136,136,108,108,0,90); $shape.AddArc(12,136,108,108,90,90); $shape.CloseFigure()
    $g.FillPath($teal, $shape)
    $g.FillPolygon($paper, [System.Drawing.Point[]]@([System.Drawing.Point]::new(68,47),[System.Drawing.Point]::new(162,47),[System.Drawing.Point]::new(189,74),[System.Drawing.Point]::new(189,206),[System.Drawing.Point]::new(68,206)))
    $g.FillPolygon($fold, [System.Drawing.Point[]]@([System.Drawing.Point]::new(162,47),[System.Drawing.Point]::new(162,74),[System.Drawing.Point]::new(189,74)))
    $pen = New-Object System.Drawing.Pen ([System.Drawing.ColorTranslator]::FromHtml('#155f55')),15
    $pen.StartCap = $pen.EndCap = [System.Drawing.Drawing2D.LineCap]::Round; $pen.LineJoin = [System.Drawing.Drawing2D.LineJoin]::Round
    $g.DrawLines($pen, [System.Drawing.Point[]]@([System.Drawing.Point]::new(91,131),[System.Drawing.Point]::new(114,154),[System.Drawing.Point]::new(161,103)))
    $stream = New-Object System.IO.MemoryStream
    $bitmap.Save($stream, [System.Drawing.Imaging.ImageFormat]::Png)
    $images += ,$stream.ToArray()
    if ($size -eq 256) { $bitmap.Save((Join-Path $root 'resources\quick-todo.png'), [System.Drawing.Imaging.ImageFormat]::Png) }
    $stream.Dispose(); $pen.Dispose(); $shape.Dispose(); $teal.Dispose(); $paper.Dispose(); $fold.Dispose(); $g.Dispose(); $bitmap.Dispose()
}
$out = [System.IO.File]::Create((Join-Path $root 'resources\quick-todo.ico'))
$writer = New-Object System.IO.BinaryWriter $out
$writer.Write([uint16]0); $writer.Write([uint16]1); $writer.Write([uint16]$sizes.Count)
$offset = 6 + 16 * $sizes.Count
for ($i=0; $i -lt $sizes.Count; $i++) {
    $dimension = if ($sizes[$i] -eq 256) { 0 } else { $sizes[$i] }
    $writer.Write([byte]$dimension); $writer.Write([byte]$dimension); $writer.Write([byte]0); $writer.Write([byte]0)
    $writer.Write([uint16]1); $writer.Write([uint16]32); $writer.Write([uint32]$images[$i].Length); $writer.Write([uint32]$offset)
    $offset += $images[$i].Length
}
foreach ($bytes in $images) { $writer.Write([byte[]]$bytes) }
$writer.Dispose(); $out.Dispose()
