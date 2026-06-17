
Set-Location "d:\SOLO-2\AI_solo_coder_task_A_136\backend"
$ErrorActionPreference = "Continue"
$out = "d:\SOLO-2\AI_solo_coder_task_A_136\backend\full_test_out.txt"

"=== Step 1: KILL existing processes ===" | Out-File $out -Encoding utf8
Get-Process | Where-Object {$_.Name -like "*siege*"} | ForEach-Object {
    "Killing PID $($_.Id)" | Out-File $out -Append -Encoding utf8
    Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
}
Start-Sleep -Seconds 3

"" | Out-File $out -Append -Encoding utf8
"=== Step 2: WAIT for cargo build to finish ===" | Out-File $out -Append -Encoding utf8
$buildDone = $false
for ($i=0; $i -lt 30; $i++) {
    $cargo = Get-Process cargo -ErrorAction SilentlyContinue
    $rustc = Get-Process rustc -ErrorAction SilentlyContinue
    if (-not $cargo -and -not $rustc) {
        "Build appears complete (no cargo/rustc running), iter=$i" | Out-File $out -Append -Encoding utf8
        $buildDone = $true
        break
    }
    Start-Sleep -Seconds 3
}
if (-not $buildDone) {
    "WARNING: cargo/rustc still running after 90s" | Out-File $out -Append -Encoding utf8
}

"" | Out-File $out -Append -Encoding utf8
"=== Step 3: CHECK binary timestamp ===" | Out-File $out -Append -Encoding utf8
$binPath = ".\target\debug\siege-tower-server.exe"
if (Test-Path $binPath) {
    $t = (Get-Item $binPath).LastWriteTime
    ("Binary exists: " + $t.ToString()) | Out-File $out -Append -Encoding utf8
    $age = [DateTime]::Now - $t
    ("Binary age: " + $age.TotalMinutes.ToString("F2") + " minutes") | Out-File $out -Append -Encoding utf8
    if ($age.TotalMinutes -lt 10) {
        "BINARY IS RECENT! (<10 min) - GOOD" | Out-File $out -Append -Encoding utf8
    } else {
        "WARNING: BINARY IS OLD! (>10 min) - BAD" | Out-File $out -Append -Encoding utf8
    }
} else {
    "FATAL: BINARY DOES NOT EXIST!" | Out-File $out -Append -Encoding utf8
}

"" | Out-File $out -Append -Encoding utf8
"=== Step 4: START server ===" | Out-File $out -Append -Encoding utf8
$env:RUST_BACKTRACE = "1"
Remove-Item all.log -ErrorAction SilentlyContinue
$argList = "-NoExit", "-Command", "cd 'd:\SOLO-2\AI_solo_coder_task_A_136\backend'; `$env:RUST_BACKTRACE='1'; & .\target\debug\siege-tower-server.exe *> all.log; pause"
Start-Process -FilePath "powershell.exe" -ArgumentList $argList -WindowStyle Hidden
"Server launch command issued" | Out-File $out -Append -Encoding utf8

"" | Out-File $out -Append -Encoding utf8
"=== Step 5: WAIT for server ===" | Out-File $out -Append -Encoding utf8
$ok = $false
for ($i=0; $i -lt 20; $i++) {
    Start-Sleep -Seconds 2
    try {
        $r = Invoke-WebRequest -Uri "http://localhost:8080/api/health" -UseBasicParsing -TimeoutSec 3
        ("Health OK after $($i*2)s: " + $r.Content) | Out-File $out -Append -Encoding utf8
        $ok = $true
        break
    } catch {
        ("Waiting... iter=$i status=FAIL" ) | Out-File $out -Append -Encoding utf8
    }
}
if (-not $ok) {
    "FATAL: Server did not start" | Out-File $out -Append -Encoding utf8
    if (Test-Path all.log) {
        "=== all.log (first 50 lines) ===" | Out-File $out -Append -Encoding utf8
        Get-Content all.log -Head 50 | Out-File $out -Append -Encoding utf8
    }
    exit 1
}

"" | Out-File $out -Append -Encoding utf8
"=== Step 6: RUN Full FEM Analysis TEST ===" | Out-File $out -Append -Encoding utf8
try {
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $r = Invoke-WebRequest -Uri "http://localhost:8080/api/towers/1/analysis/full" -UseBasicParsing -TimeoutSec 600
    $sw.Stop()
    ("SUCCESS - HTTP $($r.StatusCode) in $($sw.ElapsedMilliseconds)ms" ) | Out-File $out -Append -Encoding utf8
    ("Total length: $($r.Content.Length)" ) | Out-File $out -Append -Encoding utf8
    "" | Out-File $out -Append -Encoding utf8
    "=== Response (first 3000 chars) ===" | Out-File $out -Append -Encoding utf8
    $r.Content.Substring(0, [Math]::Min(3000, $r.Content.Length)) | Out-File $out -Append -Encoding utf8
} catch {
    ("FAIL: " + $_.Exception.Message) | Out-File $out -Append -Encoding utf8
    if ($_.Exception.Response) {
        try {
            $stream = $_.Exception.Response.GetResponseStream()
            $reader = New-Object System.IO.StreamReader($stream)
            $body = $reader.ReadToEnd()
            ("ERROR BODY: " + $body) | Out-File $out -Append -Encoding utf8
        } catch {}
    }
}

"" | Out-File $out -Append -Encoding utf8
"=== Step 7: POST-MORTEM log analysis ===" | Out-File $out -Append -Encoding utf8
if (Test-Path all.log) {
    $log = Get-Content all.log -Raw
    $idx = $log.IndexOf("panicked at")
    if ($idx -ge 0) {
        "PANIC FOUND! Position=$idx" | Out-File $out -Append -Encoding utf8
        $start = [Math]::Max(0, $idx - 50)
        $len = [Math]::Min(4000, $log.Length - $start)
        "=== Panic context ===" | Out-File $out -Append -Encoding utf8
        $log.Substring($start, $len) | Out-File $out -Append -Encoding utf8
    } else {
        "NO PANIC DETECTED IN LOG - GOOD!" | Out-File $out -Append -Encoding utf8
    }
    "" | Out-File $out -Append -Encoding utf8
    "=== Last 30 lines of log ===" | Out-File $out -Append -Encoding utf8
    Get-Content all.log -Tail 30 | Out-File $out -Append -Encoding utf8
} else {
    "all.log not found" | Out-File $out -Append -Encoding utf8
}

"" | Out-File $out -Append -Encoding utf8
"=== TEST DONE ===" | Out-File $out -Append -Encoding utf8
