
Set-Location "d:\SOLO-2\AI_solo_coder_task_A_136\backend"

Write-Host "=== Process check ==="
Get-Process | Where-Object {$_.Name -like "*siege*"} | Select-Object Id,Name,StartTime | Format-Table -AutoSize

Write-Host "`n=== Health Check ==="
try {
    $r = Invoke-WebRequest -Uri "http://localhost:8080/api/health" -UseBasicParsing -TimeoutSec 5
    Write-Host "OK: $($r.Content)"
} catch {
    Write-Host "FAIL: $($_.Exception.Message)"
}

Write-Host "`n=== Running Full FEM Analysis (GET /api/towers/1/analysis/full) ==="
try {
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $r = Invoke-WebRequest -Uri "http://localhost:8080/api/towers/1/analysis/full" -UseBasicParsing -TimeoutSec 600
    $sw.Stop()
    Write-Host "SUCCESS - STATUS: $($r.StatusCode) in $($sw.ElapsedMilliseconds)ms"
    Write-Host "`n=== Response (first 3000 chars) ==="
    $respLen = $r.Content.Length
    Write-Host "Total response length: $respLen"
    if ($respLen -gt 0) {
        $r.Content.Substring(0, [Math]::Min(3000, $respLen))
    }
    $resp = $r.Content | ConvertFrom-Json
    if ($resp.data) {
        Write-Host "`n=== Response data keys ==="
        $resp.data | Get-Member -MemberType NoteProperty | ForEach-Object { Write-Host "  - $($_.Name)" }
        if ($resp.data.structure) {
            Write-Host "`n  structure keys:"
            $resp.data.structure | Get-Member -MemberType NoteProperty | ForEach-Object { Write-Host "    - $($_.Name)" }
        }
        if ($resp.data.soil) {
            Write-Host "`n  soil keys:"
            $resp.data.soil | Get-Member -MemberType NoteProperty | ForEach-Object { Write-Host "    - $($_.Name)" }
        }
    }
} catch {
    Write-Host "FAIL: $($_.Exception.Message)"
    if ($_.Exception.Response) {
        try {
            $stream = $_.Exception.Response.GetResponseStream()
            $reader = New-Object System.IO.StreamReader($stream)
            $body = $reader.ReadToEnd()
            Write-Host "ERROR BODY: $body"
        } catch {}
    }
}

Write-Host "`n=== all.log tail 60 ==="
if (Test-Path all.log) {
    Get-Content all.log -Tail 60
} else {
    Write-Host "(log file not found)"
}
