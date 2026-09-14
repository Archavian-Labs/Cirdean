param([Parameter(Mandatory)][string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force $OutputDirectory | Out-Null
. "$PSScriptRoot/camera-controls.ps1" | Out-Null
$taskCamera = [CameraControls]::new('UGREEN Camera')
$taskOriginal = @($taskCamera.Read())
$taskOriginal | ConvertTo-Json | Set-Content (Join-Path $OutputDirectory 'controls-before.json')
$taskSettings = @{Exposure=-4;WhiteBalance=6200;Gain=0}
try {
    foreach ($taskIndex in 0..3) {
        $taskFormat = @('mjpeg','yuy2','yuy2','mjpeg')[$taskIndex]
        $taskDirectory = Join-Path $OutputDirectory $taskFormat
        New-Item -ItemType Directory -Force $taskDirectory | Out-Null
        $taskStem = Join-Path $taskDirectory "take-$taskIndex"
        $taskInfo = [System.Diagnostics.ProcessStartInfo]::new('ffmpeg')
        $taskInfo.UseShellExecute=$false
        $taskInfo.CreateNoWindow=$true
        $taskInfo.RedirectStandardError=$true
        $taskArguments = @('-hide_banner','-loglevel','info','-f','dshow','-rtbufsize','256M','-video_size','1920x1080','-framerate','5')
        $taskArguments += if ($taskFormat -eq 'mjpeg') { @('-vcodec','mjpeg') } else { @('-pixel_format','yuyv422') }
        $taskArguments += @('-i','video=UGREEN Camera','-an','-t','7','-c:v','copy','-y',"$taskStem.avi")
        foreach($taskArgument in $taskArguments) { $taskInfo.ArgumentList.Add($taskArgument) }
        $taskProcess = [System.Diagnostics.Process]::Start($taskInfo)
        $taskErrors = $taskProcess.StandardError.ReadToEndAsync()
        try {
            Start-Sleep -Seconds 1
            foreach ($taskName in $taskSettings.Keys) { $taskCamera.Set($taskName,$taskSettings[$taskName],2) }
            $taskDuring = @($taskCamera.Read())
            foreach($taskName in $taskSettings.Keys) {
                $taskRead = $taskDuring | Where-Object Name -EQ $taskName
                if($taskRead.Value -ne $taskSettings[$taskName] -or $taskRead.Flags -ne 2) { throw "Control did not lock: $taskName" }
            }
            $taskDuring | ConvertTo-Json | Set-Content "$taskStem-controls.json"
            if(-not $taskProcess.WaitForExit(20000)) { throw 'Capture timed out' }
            $taskErrors.GetAwaiter().GetResult() | Set-Content "$taskStem-ffmpeg.txt"
            if($taskProcess.ExitCode -ne 0) { throw "Capture failed: $taskStem" }
            Write-Output "Captured $taskFormat take $taskIndex, unchanged scene, fixed exposure/WB/gain"
        } finally {
            if(-not $taskProcess.HasExited) { $taskProcess.Kill(); $taskProcess.WaitForExit() }
            $taskProcess.Dispose()
        }
    }
} finally {
    try {
        foreach ($taskName in $taskSettings.Keys) {
            $taskOld = $taskOriginal | Where-Object Name -EQ $taskName
            $taskCamera.Set($taskName,$taskOld.Value,2)
            if($taskOld.Flags -ne 2) { $taskCamera.Set($taskName,$taskOld.Value,$taskOld.Flags) }
        }
        $taskCamera.Read() | ConvertTo-Json | Set-Content (Join-Path $OutputDirectory 'controls-restored.json')
    } finally { $taskCamera.Dispose() }
}
