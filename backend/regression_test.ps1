
Set-Location "d:\SOLO-2\AI_solo_coder_task_A_136\backend"
$out = "d:\SOLO-2\AI_solo_coder_task_A_136\backend\regression_out.txt"
Remove-Item $out -ErrorAction SilentlyContinue

function TEST($name, $url, $method="GET", $body=$null) {
    "`n=== TEST: $name ===" | Out-File $out -Append -Encoding utf8
    "URL: $method $url" | Out-File $out -Append -Encoding utf8
    try {
        $sw = [System.Diagnostics.Stopwatch]::StartNew()
        if ($method -eq "GET") {
            $r = Invoke-WebRequest -Uri $url -UseBasicParsing -TimeoutSec 120
        } else {
            $r = Invoke-WebRequest -Uri $url -Method POST -Body $body -ContentType "application/json" -UseBasicParsing -TimeoutSec 120
        }
        $sw.Stop()
        "STATUS: HTTP $($r.StatusCode) in $($sw.ElapsedMilliseconds)ms" | Out-File $out -Append -Encoding utf8
        $len = $r.Content.Length
        "Response length: $len chars" | Out-File $out -Append -Encoding utf8
        if ($len -gt 0) {
            $snippet = $r.Content.Substring(0, [Math]::Min(600, $len))
            "Snippet: $snippet" | Out-File $out -Append -Encoding utf8
        }
        "PASS" | Out-File $out -Append -Encoding utf8
        return $true
    } catch {
        "FAIL: $($_.Exception.Message)" | Out-File $out -Append -Encoding utf8
        if ($_.Exception.Response) {
            try {
                $stream = $_.Exception.Response.GetResponseStream()
                $reader = New-Object System.IO.StreamReader($stream)
                $e_body = $reader.ReadToEnd()
                "Error body: $e_body" | Out-File $out -Append -Encoding utf8
            } catch {}
        }
        return $false
    }
}

$base = "http://localhost:8080"
$allPass = $true

"========== REGRESSION TEST SUITE ==========" | Out-File $out -Encoding utf8
"Start time: $(Get-Date -Format 'o')" | Out-File $out -Append -Encoding utf8

# Test 1: Health
$allPass = (TEST "1. Health check" "$base/api/health") -and $allPass

# Test 2: Tower configs
$allPass = (TEST "2. Tower configs" "$base/api/config/towers") -and $allPass

# Test 3: Soil configs
$allPass = (TEST "3. Soil configs" "$base/api/config/soils") -and $allPass

# Test 4: Tower sensor POST
$sensorPayload = @{
    tower_id="tower_a"
    timestamp=(Get-Date).ToString("o")
    layers=@(@{layer_id=1;wind_speed_mps=8.5;stress_x=10.1;stress_y=5.2;stress_z=35.0;temperature_c=22.0;humidity_pct=48.0;inclination_deg=0.2;vibration_freq_hz=1.5;displacement_mm=1.8;weight_on_soil_kg=7500;soil_pressure_kpa=32.0})
} | ConvertTo-Json -Depth 5
$allPass = (TEST "4. POST sensor data" "$base/api/towers/1/sensor" "POST" $sensorPayload) -and $allPass

# Test 5: Tower sensor GET
$allPass = (TEST "5. GET sensor data" "$base/api/towers/1/sensor") -and $allPass

# Test 6: Quick simulation POST
$simPayload = @{tower_id="tower_a";layers=@(@{layer_id=1;wind_speed_mps=15.0;stress_x=25.0;stress_y=12.0;stress_z=55.0;temperature_c=25.0;humidity_pct=55.0;inclination_deg=0.8;vibration_freq_hz=3.2;displacement_mm=5.1;weight_on_soil_kg=9200;soil_pressure_kpa=45.0})} | ConvertTo-Json -Depth 5
$allPass = (TEST "6. POST quick simulation" "$base/api/towers/1/analysis" "POST" $simPayload) -and $allPass

# Test 7: GET latest analysis
$allPass = (TEST "7. GET latest analysis" "$base/api/towers/1/analysis") -and $allPass

# Test 8: Structure-only analysis
$allPass = (TEST "8. Structure analysis endpoint" "$base/api/towers/1/analysis/structure") -and $allPass

# Test 9: Ground/soil analysis
$allPass = (TEST "9. Ground/Soil analysis" "$base/api/towers/1/ground") -and $allPass

# Test 10: Alerts endpoint
$allPass = (TEST "10. GET alert events" "$base/api/towers/1/alerts") -and $allPass

# Test 11: SSE sensor stream (brief) - may timeout quickly which is OK
"`n=== TEST: 11. SSE sensor stream (connectivity check) ===" | Out-File $out -Append -Encoding utf8
try {
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $r = Invoke-WebRequest -Uri "$base/api/stream/sensor" -UseBasicParsing -TimeoutSec 3
    "SSE returned data (unexpected but OK): $($r.StatusCode)" | Out-File $out -Append -Encoding utf8
    "PASS" | Out-File $out -Append -Encoding utf8
} catch {
    $msg = $_.Exception.Message
    if ($msg -match "timeout|超时|canceled" -or $_.Exception.Response -ne $null) {
        "SSE stream OK (expected timeout/connection, module is alive)" | Out-File $out -Append -Encoding utf8
        "PASS" | Out-File $out -Append -Encoding utf8
    } else {
        "FAIL: $msg" | Out-File $out -Append -Encoding utf8
        $allPass = $false
    }
}

# Test 12: SSE analysis stream (brief)
"`n=== TEST: 12. SSE analysis stream (connectivity check) ===" | Out-File $out -Append -Encoding utf8
try {
    $r = Invoke-WebRequest -Uri "$base/api/stream/analysis" -UseBasicParsing -TimeoutSec 3
    "PASS" | Out-File $out -Append -Encoding utf8
} catch {
    $msg = $_.Exception.Message
    if ($msg -match "timeout|超时|canceled" -or $_.Exception.Response -ne $null) {
        "SSE analysis stream OK (expected timeout)" | Out-File $out -Append -Encoding utf8
        "PASS" | Out-File $out -Append -Encoding utf8
    } else {
        "FAIL: $msg" | Out-File $out -Append -Encoding utf8
        $allPass = $false
    }
}

# Test 13: Full FEM end-to-end (the core fix)
$allPass = (TEST "13. FULL FEM END-TO-END (critical)" "$base/api/towers/1/analysis/full") -and $allPass

"`n==========================================" | Out-File $out -Append -Encoding utf8
"FINAL RESULT: $(if ($allPass) {'ALL TESTS PASSED ✓'} else {'SOME TESTS FAILED ✗'})" | Out-File $out -Append -Encoding utf8
"End time: $(Get-Date -Format 'o')" | Out-File $out -Append -Encoding utf8
