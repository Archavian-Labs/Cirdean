param([Parameter(Mandatory)][string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
. "$PSScriptRoot/camera-controls.ps1" | Out-Null
$taskCamera = [CameraControls]::new('UGREEN Camera')
$taskOriginal = $taskCamera.Read() | Where-Object Name -EQ Exposure
$taskCamera.Read() | ConvertTo-Json | Set-Content (Join-Path $OutputDirectory 'controls-before.json')
try {
    foreach ($taskExposure in @(-6, -5, -4, -3, -2)) {
        if ($taskExposure -lt $taskOriginal.Min -or $taskExposure -gt $taskOriginal.Max) { continue }
        if (($taskExposure - $taskOriginal.Min) % $taskOriginal.Step -ne 0) { continue }
        $taskTag = 'exposure-' + (-$taskExposure)
        $taskInfo = [System.Diagnostics.ProcessStartInfo]::new('ffmpeg')
        $taskInfo.UseShellExecute = $false
        $taskInfo.CreateNoWindow = $true
        $taskInfo.RedirectStandardError = $true
        foreach ($taskArgument in @('-hide_banner','-loglevel','warning','-f','dshow','-video_size','1920x1080','-framerate','5','-vcodec','mjpeg','-i','video=UGREEN Camera','-an','-t','7','-c:v','copy','-y',(Join-Path $OutputDirectory "$taskTag.avi"))) { $taskInfo.ArgumentList.Add($taskArgument) }
        $taskProcess = [System.Diagnostics.Process]::Start($taskInfo)
        $taskErrors = $taskProcess.StandardError.ReadToEndAsync()
        try {
            Start-Sleep -Seconds 1
            $taskCamera.Set('Exposure', $taskExposure, 2)
            if (-not $taskProcess.WaitForExit(20000)) { throw 'Capture timed out' }
            if ($taskProcess.ExitCode -ne 0) { throw "ffmpeg failed: $($taskErrors.GetAwaiter().GetResult())" }
            $taskCamera.Read() | ConvertTo-Json | Set-Content (Join-Path $OutputDirectory "$taskTag-controls.json")
            $taskErrors.GetAwaiter().GetResult() | Set-Content (Join-Path $OutputDirectory "$taskTag-ffmpeg.txt")
            Write-Output "Captured $taskTag as MJPEG stream copy"
        } finally {
            if (-not $taskProcess.HasExited) { $taskProcess.Kill(); $taskProcess.WaitForExit() }
            $taskProcess.Dispose()
        }
    }
} finally {
    # Restore the previous numeric setting, then its auto/manual mode.
    $taskCamera.Set('Exposure', $taskOriginal.Value, 2)
    if ($taskOriginal.Flags -ne 2) { $taskCamera.Set('Exposure', $taskOriginal.Value, $taskOriginal.Flags) }
    $taskCamera.Read() | ConvertTo-Json | Set-Content (Join-Path $OutputDirectory 'controls-restored.json')
    $taskCamera.Dispose()
}
