$baseUrl = "http://localhost:8080"
$allResults = @()

function Run-Test($id, $method, $path, $bodyJson = $null) {
    Write-Host "`n=== Test $id : $method $path ==="
    $sw = [Diagnostics.Stopwatch]::StartNew()
    try {
        if ($method -eq "GET") {
            $response = Invoke-WebRequest -Uri "$baseUrl$path" -Method GET -TimeoutSec 120 -UseBasicParsing
        } else {
            $response = Invoke-WebRequest -Uri "$baseUrl$path" -Method POST -Body $bodyJson -ContentType "application/json" -TimeoutSec 120 -UseBasicParsing
        }
        $sw.Stop()
        $status = [int]$response.StatusCode
        $content = $response.Content
        if ($content.Length -gt 500) { $content = $content.Substring(0,500) }
        $pass = if ($status -ge 200 -and $status -lt 300) { "PASS" } else { "FAIL" }
        Write-Host "$pass | $status | $($sw.ElapsedMilliseconds)ms"
        Write-Host "Response: $content"
        $script:allResults += [PSCustomObject]@{ID=$id;Method=$method;Path=$path;Status=$status;TimeMS=$sw.ElapsedMilliseconds;Result=$pass;Response=$content}
    } catch {
        $sw.Stop()
        $status = if ($_.Exception.Response) { [int]$_.Exception.Response.StatusCode } else { 0 }
        $err = $_.Exception.Message
        if ($err.Length -gt 500) { $err = $err.Substring(0,500) }
        Write-Host "FAIL | $status | $($sw.ElapsedMilliseconds)ms"
        Write-Host "Error: $err"
        $script:allResults += [PSCustomObject]@{ID=$id;Method=$method;Path=$path;Status=$status;TimeMS=$sw.ElapsedMilliseconds;Result="FAIL";Response=$err}
    }
}

$correctSensorBody = @{
    tower_id = 1
    tower_name = "Tower Alpha"
    timestamp = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ss.fffZ")
    layers = @(
        @{
            layer_id = 1
            layer_name = "Layer 1"
            stress_x = 10.1
            stress_y = 5.2
            stress_z = 35.0
            stress_von_mises = 38.5
            tilt_x = 0.1
            tilt_y = 0.17
            tilt_total = 0.2
            vibration_freq_hz = 1.5
            vibration_amplitude = 0.05
            battery_voltage = 3.7
            signal_strength = -65.0
        }
    )
    environment = @{
        wind_speed_mps = 8.5
        wind_direction_deg = 180.0
        ground_pressure_kpa = 32.0
        temperature_c = 22.0
        humidity_pct = 48.0
    }
} | ConvertTo-Json -Depth 10 -Compress

$originalBody = '{"tower_id":"tower_a","layers":[{"layer_id":1,"wind_speed_mps":8.5,"stress_x":10.1,"stress_y":5.2,"stress_z":35.0,"temperature_c":22.0,"humidity_pct":48.0,"inclination_deg":0.2,"vibration_freq_hz":1.5,"displacement_mm":1.8,"weight_on_soil_kg":7500,"soil_pressure_kpa":32.0}]}'

Write-Host "========== Re-testing with corrected endpoints =========="

Run-Test "2(corrected)" "GET" "/api/config/towers/1"
Run-Test "4(original)" "POST" "/api/towers/1/sensor" $originalBody
Run-Test "4(corrected)" "POST" "/api/towers/1/sensor" $correctSensorBody
Run-Test "5" "GET" "/api/towers/1/sensor"
Run-Test "7" "GET" "/api/towers/1/analysis"

Write-Host "`n`n========== SUMMARY OF RETESTS =========="
$allResults | Format-Table -AutoSize ID, Method, Path, Status, TimeMS, Result
