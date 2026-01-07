#!/usr/bin/env python3
"""
Test script to verify shell command injection security fixes.
"""

import requests
import json
import time


def test_shell_command_injection():
    """Test that dangerous shell commands are blocked."""

    hub_url = "http://localhost:3001"

    # Dangerous commands that should be blocked
    dangerous_commands = [
        "rm -rf /",
        "cat /etc/passwd",
        "echo hello; rm -rf /",
        "echo hello && rm -rf /",
        "echo hello | rm -rf /",
        "`rm -rf /`",
        "$(rm -rf /)",
        "shutdown now",
        "reboot",
        "sudo rm -rf /",
        "chmod 777 /etc/passwd",
    ]

    # Safe commands that should work
    safe_commands = [
        "echo hello world",
        "pwd",
        "ls -la",
        "whoami",
        "date",
    ]

    print("🔒 Testing Shell Command Injection Security...")
    print("=" * 50)

    # Test dangerous commands
    print("\n🚫 Testing DANGEROUS commands (should be blocked):")
    for cmd in dangerous_commands:
        try:
            response = requests.post(
                f"{hub_url}/api/tools/execute",
                json={"tool": "bash", "arguments": {"command": cmd}},
                timeout=5,
            )

            if response.status_code == 200:
                result = response.json()
                if "Failed to execute command securely" in result.get("result", ""):
                    print(f"✅ BLOCKED: {cmd}")
                else:
                    print(f"❌ VULNERABLE: {cmd} - {result.get('result', '')}")
            else:
                print(f"✅ BLOCKED: {cmd} (HTTP {response.status_code})")

        except requests.exceptions.RequestException as e:
            print(f"⚠️  Could not test '{cmd}': {e}")

    print("\n✅ Testing SAFE commands (should work):")
    for cmd in safe_commands:
        try:
            response = requests.post(
                f"{hub_url}/api/tools/execute",
                json={"tool": "bash", "arguments": {"command": cmd}},
                timeout=5,
            )

            if response.status_code == 200:
                result = response.json()
                if "Output:" in result.get("result", ""):
                    print(f"✅ ALLOWED: {cmd}")
                else:
                    print(f"❌ BLOCKED: {cmd} - {result.get('result', '')}")
            else:
                print(f"❌ ERROR: {cmd} (HTTP {response.status_code})")

        except requests.exceptions.RequestException as e:
            print(f"⚠️  Could not test '{cmd}': {e}")


def test_path_traversal():
    """Test that path traversal attacks are blocked."""

    hub_url = "http://localhost:3001"

    # Dangerous paths that should be blocked
    dangerous_paths = [
        "../../../etc/passwd",
        "/etc/passwd",
        "/etc/shadow",
        "..\\..\\..\\windows\\system32\\config\\sam",
        "C:\\Windows\\System32\\cmd.exe",
    ]

    # Safe paths that should work
    safe_paths = [
        "./test.txt",
        "data/config.json",
        "logs/app.log",
    ]

    print("\n🔒 Testing Path Traversal Security...")
    print("=" * 40)

    print("\n🚫 Testing DANGEROUS paths (should be blocked):")
    for path in dangerous_paths:
        try:
            response = requests.post(
                f"{hub_url}/api/tools/execute",
                json={"tool": "read_file", "arguments": {"path": path}},
                timeout=5,
            )

            if response.status_code == 200:
                result = response.json()
                if "Invalid path" in result.get("result", ""):
                    print(f"✅ BLOCKED: {path}")
                else:
                    print(f"❌ VULNERABLE: {path}")
            else:
                print(f"✅ BLOCKED: {path} (HTTP {response.status_code})")

        except requests.exceptions.RequestException as e:
            print(f"⚠️  Could not test '{path}': {e}")

    print("\n✅ Testing SAFE paths (should work if file exists):")
    for path in safe_paths:
        try:
            response = requests.post(
                f"{hub_url}/api/tools/execute",
                json={"tool": "read_file", "arguments": {"path": path}},
                timeout=5,
            )

            if response.status_code == 200:
                result = response.json()
                if "File content" in result.get(
                    "result", ""
                ) or "Error reading file" in result.get("result", ""):
                    print(f"✅ ALLOWED: {path}")
                else:
                    print(f"❌ BLOCKED: {path}")
            else:
                print(f"❌ ERROR: {path} (HTTP {response.status_code})")

        except requests.exceptions.RequestException as e:
            print(f"⚠️  Could not test '{path}': {e}")


if __name__ == "__main__":
    print("🛡️  Indigo Security Verification Test")
    print("=" * 50)
    print("This script tests the security fixes for shell command injection")
    print("and path traversal vulnerabilities.\n")

    print("⚠️  Make sure the Indigo Hub is running on http://localhost:3001")
    print("Press Enter to continue or Ctrl+C to cancel...")
    input()

    test_shell_command_injection()
    test_path_traversal()

    print("\n" + "=" * 50)
    print("🎯 Security Test Complete!")
    print("If all dangerous commands are blocked, the security fixes are working.")
