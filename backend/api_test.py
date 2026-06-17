import urllib.request
import urllib.error
import json
import time
import sys

BASE_URL = "http://localhost:8080"
results = []

def test_endpoint(name, method, path, body=None):
    start = time.time()
    url = f"{BASE_URL}{path}"
    try:
        if method == "GET":
            req = urllib.request.Request(url, method="GET")
        else:
            data = json.dumps(body).encode("utf-8")
            req = urllib.request.Request(url, data=data, method="POST")
            req.add_header("Content-Type", "application/json")
        
        with urllib.request.urlopen(req, timeout=120) as resp:
            status = resp.status
            content = resp.read().decode("utf-8")
    except urllib.error.HTTPError as e:
        status = e.code
        content = str(e)
    except Exception as e:
        status = 0
        content = str(e)
    
    elapsed_ms = int((time.time() - start) * 1000)
    if len(content) > 500:
        content = content[:500]
    
    passed = "PASS" if 200 <= status < 300 else "FAIL"
    results.append({
        "name": name,
        "method": method,
        "path": path,
        "status": status,
        "time_ms": elapsed_ms,
        "result": passed,
        "response": content
    })
    print(f"[{passed}] {name} | {method} {path} | {status} | {elapsed_ms}ms")
    print(f"  Response: {content}\n")
    sys.stdout.flush()

sensor_body = {
    "tower_id": "tower_a",
    "layers": [{
        "layer_id": 1,
        "wind_speed_mps": 8.5,
        "stress_x": 10.1,
        "stress_y": 5.2,
        "stress_z": 35.0,
        "temperature_c": 22.0,
        "humidity_pct": 48.0,
        "inclination_deg": 0.2,
        "vibration_freq_hz": 1.5,
        "displacement_mm": 1.8,
        "weight_on_soil_kg": 7500,
        "soil_pressure_kpa": 32.0
    }]
}

print("=" * 60)
print("STARTING API REGRESSION TESTS")
print("=" * 60)
sys.stdout.flush()

test_endpoint("1.Health", "GET", "/api/health")
test_endpoint("2.ConfigTowers", "GET", "/api/config/towers")
test_endpoint("3.ConfigSoils", "GET", "/api/config/soils")
test_endpoint("4.PostSensor", "POST", "/api/towers/1/sensor", sensor_body)
test_endpoint("5.GetSensor", "GET", "/api/towers/1/sensor")
test_endpoint("6.PostAnalysis", "POST", "/api/towers/1/analysis", sensor_body)
test_endpoint("7.GetAnalysis", "GET", "/api/towers/1/analysis")
test_endpoint("8.AnalysisStructure", "GET", "/api/towers/1/analysis/structure")
test_endpoint("9.Ground", "GET", "/api/towers/1/ground")
test_endpoint("10.Alerts", "GET", "/api/towers/1/alerts")
test_endpoint("11.FullAnalysis", "GET", "/api/towers/1/analysis/full")

print("=" * 60)
print("TEST SUMMARY")
print("=" * 60)
pass_count = sum(1 for r in results if r["result"] == "PASS")
fail_count = sum(1 for r in results if r["result"] == "FAIL")
print(f"Total: {len(results)} | PASS: {pass_count} | FAIL: {fail_count}")
print()
print(f"{'#':<3} {'Name':<25} {'Method':<6} {'Path':<35} {'Status':<7} {'Time(ms)':<9} {'Result'}")
print("-" * 95)
for i, r in enumerate(results):
    print(f"{i+1:<3} {r['name']:<25} {r['method']:<6} {r['path']:<35} {r['status']:<7} {r['time_ms']:<9} {r['result']}")

with open("api_test_results_py.json", "w", encoding="utf-8") as f:
    json.dump(results, f, ensure_ascii=False, indent=2)
print(f"\nResults saved to api_test_results_py.json")
