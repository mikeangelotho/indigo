import subprocess
import json
import time

data = {
    "prompt": "hi",
    "agent_id": "simple-test",
    "max_tokens": 100,
    "temperature": 0.7,
    "enable_tools": False,
}
request_json = json.dumps(data, indent=2)
request_data = request_json.encode("utf-8")
length_bytes = len(request_data).to_bytes(4, byteorder="big")

print("Request:", request_json)
print("Length:", len(request_data), "bytes")

proc = subprocess.Popen(
    [
        "./indigo-core/target/debug/indigo-node.exe",
        "--model-file",
        "crates/indigo-hub/models/gemma-3-1b-it-q4_0.gguf",
        "--n-ctx",
        "4096",
        "--n-batch",
        "512",
        "--gpu-layers",
        "32",
        "--stdio",
        "--node-id",
        "test-standalone",
    ],
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    stderr=subprocess.PIPE,
    text=True,
)

time.sleep(2)
data_to_write = length_bytes + request_data.encode("utf-8")
proc.stdin.write(data_to_write)
proc.stdin.flush()

for line in proc.stdout:
    print("Response:", line.strip())
