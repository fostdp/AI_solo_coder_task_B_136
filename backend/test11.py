import urllib.request
import urllib.error
import json
import time

start = time.time()
print('Starting Test11 FullAnalysis (FEM)...', flush=True)
try:
    url = 'http://localhost:8080/api/towers/1/analysis/full'
    with urllib.request.urlopen(url, timeout=600) as r:
        content = r.read().decode()
        status = r.status
    elapsed = int((time.time() - start) * 1000)
    print(f'Test11 Status: {status}, Time: {elapsed}ms', flush=True)
    resp_short = content[:500] if len(content) > 500 else content
    print(f'Response: {resp_short}', flush=True)
    result = {
        'name': 'Test11_FullAnalysis',
        'method': 'GET',
        'path': '/api/towers/1/analysis/full',
        'status': status,
        'time_ms': elapsed,
        'result': 'PASS' if 200 <= status < 300 else 'FAIL',
        'response': resp_short
    }
except Exception as e:
    elapsed = int((time.time() - start) * 1000)
    print(f'Test11 FAILED: {e}', flush=True)
    result = {
        'name': 'Test11_FullAnalysis',
        'method': 'GET',
        'path': '/api/towers/1/analysis/full',
        'status': 0,
        'time_ms': elapsed,
        'result': 'FAIL',
        'response': str(e)
    }

with open('D:/SOLO-2/AI_solo_coder_task_A_136/backend/test11_result.json', 'w', encoding='utf-8') as f:
    json.dump(result, f, ensure_ascii=False, indent=2)
print('Test11 saved to file.', flush=True)
