#!/usr/bin/env python3
import subprocess, json, sys, time, os


def test_complete_flow():
    """Test complete flow from hub to sidecar"""

    print("Testing Complete Indigo Flow...")

    # Create test request
    test_request = {
        "prompt": "hi",
        "agent_id": "simple-test",
        "max_tokens": 100,
        "temperature": 0.7,
        "enable_tools": False,
    }

    # Prepare request data in correct protocol format
    request_json = json.dumps(test_request, indent=2)
    request_data = request_json.encode("utf-8")
    length_bytes = struct.pack(">I", len(request_data))

    print("Sending request:", request_json)
    print("Request length:", len(request_data), "bytes")

    try:
        # Start sidecar using correct path
        current_dir = os.path.dirname(os.path.abspath(__file__))
        sidecar_path = os.path.join(
            current_dir, "./indigo-core/target/debug/indigo-node.exe"
        )

        proc = subprocess.Popen(
            [
                sidecar_path,
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

        print("Sidecar started")

        # Wait for sidecar to be ready
        time.sleep(2)

        # Send to sidecar stdin
        proc.stdin.write(length_bytes + request_data)
        proc.stdin.flush()

        # Read response
        response_lines = []
        start_time = time.time()

        while True:
            line = proc.stdout.readline()
            if not line:
                break
            response_lines.append(line.strip())

            # Timeout after 30 seconds
            if time.time() - start_time > 30:
                print("Timeout reached")
                break

        # Get any error output
        try:
            _, stderr_output = proc.communicate(timeout=1)
            if stderr_output:
                print("Sidecar errors:", stderr_output)
        except:
            pass

        print("Response lines received:", len(response_lines))
        for i, line in enumerate(response_lines[:5]):  # First 5 lines
            print(f"  {i + 1}: {line}")

        print("Test completed!")

    except Exception as e:
        print("Error during test:", e)


if __name__ == "__main__":
    test_complete_flow()
