$baseUrl = "http://localhost:8080"
$results = @()

function Test-Endpoint {
    param($name, $method, $path, $body = $null)
    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    try {
        $headers = @{"Content-Type" = "application/json"}
        if ($method -eq "GET") {
            $response = Invoke-WebRequest -Uri "$baseUrl$path" -Method GET -UseBasicParsing -TimeoutSec 60
        } else {
            $bodyStr = $body | ConvertTo-Json -Depth 10 -Compress
            $response = Invoke-WebRequest -Uri "$baseUrl$path" -Method POST -Body $bodyStr -Headers $headers -UseBasicParsing -TimeoutSec 60
        }
        $stopwatch.Stop()
        $status = $response.StatusCode
        $content = $response.Content
        if ($content.Length -gt 500) { $content = $content.Substring(0, 500) }
        $pass = if ($status -ge 200 -and $status -lt 300) { "PASS" } else { "FAIL" }
        $results += [PSCustomObject]@{
            Name = $name
            Method = $method
            Path = $path
            Status = $status
            TimeMS = $stopwatch.ElapsedMilliseconds
            Result = $pass
            Response = $content
        }
        Write-Host "[$pass] $name | $status | $($stopwatch.ElapsedMilliseconds)ms"
    } catch {
        $stopwatch.Stop()
        $errMsg = $_.Exception.Message
        $status = 0
        if ($_.Exception.Response) {
            try { $status = [int]$_.Exception.Response.StatusCode } catch {}
        }
        if ($errMsg.Length -gt 500) { $errMsg = $errMsg.Substring(0, 500) }
        $results += [PSCustomObject]@{
            Name = $name
            Method = $method
            Path = $path
            Status = $status
            TimeMS = $stopwatch.ElapsedMilliseconds
            Result = "FAIL"
            Response = $errMsg
        }
        Write-Host "[FAIL] $name | $status | $($stopwatch.ElapsedMilliseconds)ms"
    }
}

$bodyObj = @{
    tower_id = "tower_a"
    layers = @(
        @{
            layer_id = 1
            wind_speed_mps = 8.5
            stress_x = 10.1
            stress_y = 5.2
            stress_z = 35.0
            temperature_c = 22.0
            humidity_pct = 48.0
            inclination_deg = 0.2
            vibration_freq_hz = 1.5
            displacement_mm = 1.8
            weight_on_soil_kg = 7500
            soil_pressure_kpa = 32.0
        }
    )
}

Write-Host "=== Starting API Tests ==="
Test-Endpoint "1.Health" "GET" "/api/health"
Test-Endpoint "2.ConfigTowers" "GET" "/api/config/towers"
Test-Endpoint "3.ConfigSoils" "GET" "/api/config/soils"
Test-Endpoint "4.PostSensor" "POST" "/api/towers/1/sensor" $bodyObj
Test-Endpoint "5.GetSensor" "GET" "/api/towers/1/sensor"
Test-Endpoint "6.PostAnalysis" "POST" "/api/towers/1/analysis" $bodyObj
Test-Endpoint "7.GetAnalysis" "GET" "/api/towers/1/analysis"
Test-Endpoint "8.GetAnalysisStructure" "GET" "/api/towers/1/analysis/structure"
Test-Endpoint "9.GetGround" "GET" "/api/towers/1/ground"
Test-Endpoint "10.GetAlerts" "GET" "/api/towers/1/alerts"
Test-Endpoint "11.GetFullAnalysis" "GET" "/api/towers/1/analysis/full"

Write-Host "`n=== FULL RESULTS ==="
$results | ForEach-Object {
    Write-Host "`n--- $($_.Name) ($($_.Method) $($_.Path)) ---"
    Write-Host "Result: $($_.Result) | Status: $($_.Status) | Time: $($_.TimeMS)ms"
    Write-Host "Response: $($_.Response)"
}

Write-Host "`n=== SUMMARY ==="
$passCount = ($results | Where-Object { $_.Result -eq "PASS" }).Count
$failCount = ($results | Where-Object { $_.Result -eq "FAIL" }).Count
Write-Host "Total: $($results.Count) | PASS: $passCount | FAIL: $failCount"

$results | ConvertTo-Json -Depth 5 | Out-File -FilePath "api_test_results.json" -Encoding UTF8
Write-Host "`nResults saved to api_test_results.json"
