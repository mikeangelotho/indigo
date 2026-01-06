#!/usr/bin/env python3
"""
Direct test of sidecar communication to isolate the issue
"""

import subprocess
import struct
import json
import time
import sys
import os


def test_sidecar_direct():
    """Test sidecar directly without hub routing"""

    print("=== Direct Sidecar Test ===")

    # Find the sidecar binary
    sidecar_paths = [
        "./indigo-core/target/debug/indigo-node.exe",
        "./indigo-core/crates/indigo-node/target/debug/indigo-node.exe",
        "./target/debug/indigo-node.exe",
    ]

    sidecar_path = None
    for path in sidecar_paths:
        if os.path.exists(path):
            sidecar_path = path
            break

    if not sidecar_path:
        print("X Sidecar binary not found")
        return False

    print(f"+ Found sidecar at: {sidecar_path}")

    # Find the model
    model_path = "./indigo-core/crates/indigo-hub/models/gemma-3-1b-it-q4_0.gguf"
    if not os.path.exists(model_path):
        print(f"❌ Model not found: {model_path}")
        return False

    print(f"+ Found model: {model_path}")

    # Start sidecar
    try:
        proc = subprocess.Popen(
            [
                sidecar_path,
                "--model-file",
                model_path,
                "--n-ctx",
                "4096",
                "--n-batch",
                "512",
                "--gpu-layers",
                "32",
                "--stdio",
                "--node-id",
                "test-sidecar",
            ],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=False,  # Use binary mode for protocol
            cwd=os.getcwd(),
        )

        print("✓ Sidecar started, waiting for initialization...")
        time.sleep(3)

        # Check if process is still running
        if proc.poll() is not None:
            stdout, stderr = proc.communicate()
            print(f"❌ Sidecar exited immediately")
            if stderr:
                print(f"Stderr: {stderr.decode('utf-8', errors='ignore')}")
            return False

        print("✓ Sidecar is running")

        # Prepare test request (same format hub would send)
        test_request = {
            "prompt": '[{"role":"system","content":"You are a helpful AI assistant."},{"role":"user","content":"Hey!"}]',
            "max_tokens": 50,
            "temperature": 0.7,
            "image_data": b"",
            "stop": [],
        }

        # Convert to protobuf format (simplified - we'll use JSON for this test)
        request_json = json.dumps(test_request).encode("utf-8")
        length_bytes = struct.pack(">I", len(request_json))

        print(f"✓ Sending request: {len(request_json)} bytes")

        # Send to sidecar
        proc.stdin.write(length_bytes + request_json)
        proc.stdin.flush()

        print("✓ Request sent, waiting for response...")

        # Read response
        response_data = b""
        start_time = time.time()

        while time.time() - start_time < 10:
            # Read length prefix
            try:
                len_bytes = proc.stdout.read(4)
                if len(len_bytes) < 4:
                    break

                msg_len = struct.unpack(">I", len_bytes)[0]
                msg_data = proc.stdout.read(msg_len)

                if msg_data:
                    response_data += msg_data
                    print(f"✓ Received response: {len(msg_data)} bytes")
                    print(f"Content: {msg_data.decode('utf-8', errors='ignore')}")
                    break

            except Exception as e:
                print(f"Error reading response: {e}")
                break

        if response_data:
            print("✓ SUCCESS: Sidecar communication works!")
            return True
        else:
            print("❌ FAILURE: No response from sidecar")
            return False

    except Exception as e:
        print(f"❌ Error during test: {e}")
        return False
    finally:
        # Cleanup
        try:
            proc.terminate()
            proc.wait(timeout=5)
        except:
            try:
                proc.kill()
            except:
                pass


if __name__ == "__main__":
    success = test_sidecar_direct()
    sys.exit(0 if success else 1)
