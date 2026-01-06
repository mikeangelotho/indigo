#!/usr/bin/env python3
import subprocess
import time
import os


def test_sidecar_simple():
    """Simple test to see if sidecar starts correctly"""

    print("=== Simple Sidecar Test ===")

    # Find model
    model_path = "./indigo-core/crates/indigo-hub/models/gemma-3-1b-it-q4_0.gguf"
    if not os.path.exists(model_path):
        print(f"Model not found: {model_path}")
        return False

    print(f"Found model: {model_path}")

    # Find sidecar binary
    sidecar_path = "./indigo-core/target/debug/indigo-node.exe"
    if not os.path.exists(sidecar_path):
        print("Sidecar binary not found")
        return False

    print(f"Found sidecar: {sidecar_path}")

    try:
        # Start sidecar with basic args
        proc = subprocess.Popen(
            [
                sidecar_path,
                "--model-file",
                model_path,
                "--n-ctx",
                "4096",
                "--gpu-layers",
                "32",
                "--stdio",
                "--node-id",
                "test",
            ],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            cwd=os.getcwd(),
        )

        print("Sidecar process started")
        time.sleep(3)

        # Check if process is still running
        if proc.poll() is not None:
            stdout, stderr = proc.communicate()
            print("Sidecar exited immediately")
            print(f"Return code: {proc.returncode}")
            if stderr:
                print(f"Error output: {stderr.decode('utf-8', errors='ignore')}")
            return False

        print("SUCCESS: Sidecar is running!")
        print("Waiting 2 more seconds for full initialization...")
        time.sleep(2)

        # Clean up
        proc.terminate()
        try:
            proc.wait(timeout=3)
        except subprocess.TimeoutExpired:
            proc.kill()

        print("Sidecar stopped cleanly")
        return True

    except Exception as e:
        print(f"Error: {e}")
        return False


if __name__ == "__main__":
    success = test_sidecar_simple()
    print(f"Test result: {'PASS' if success else 'FAIL'}")
