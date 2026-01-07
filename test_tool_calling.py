#!/usr/bin/env python3
"""
Comprehensive test for tool calling with Gemma and Qwen models
"""

import json
import requests
import time
import sys
import os


def test_model_tool_calling(hub_url="http://localhost:3001"):
    """Test tool calling with available models"""

    # Check available models first
    try:
        response = requests.get(f"{hub_url}/v1/models", timeout=5)
        if response.status_code != 200:
            print("❌ Failed to get models list")
            return
        models = response.json().get("data", [])
        model_names = [m.get("id") for m in models]
        print(f"Available models: {model_names}")
    except Exception as e:
        print(f"❌ Error getting models: {e}")
        return

    # Find Gemma and Qwen models
    target_models = []
    for model_name in model_names:
        name_lower = model_name.lower()
        if "gemma" in name_lower or "qwen" in name_lower:
            target_models.append(model_name)

    if not target_models:
        print("⚠️ No Gemma or Qwen models found. Testing with first available model.")
        if model_names:
            target_models = [model_names[0]]
        else:
            print("❌ No models available!")
            return

    print(f"\n🧪 Testing with models: {target_models}")

    # Test tools
    tools = [
        {
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Read contents of a file",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to file to read",
                        }
                    },
                    "required": ["path"],
                },
            },
        },
        {
            "type": "function",
            "function": {
                "name": "list_files",
                "description": "List files in a directory",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Directory path (optional, defaults to current)",
                        }
                    },
                    "required": [],
                },
            },
        },
    ]

    # Test scenarios
    test_scenarios = [
        {
            "name": "Natural Language Request",
            "message": "Please read the test.txt file for me",
            "expected_tool": "read_file",
        },
        {
            "name": "Direct File Request",
            "message": "I need to see what's in the current directory",
            "expected_tool": "list_files",
        },
        {
            "name": "Command-style Request",
            "message": "Can you list files in the current directory?",
            "expected_tool": "list_files",
        },
    ]

    for model in target_models:
        print(f"\n{'=' * 60}")
        print(f"Testing Model: {model}")
        print(f"{'=' * 60}")

        model_success = 0
        model_total = 0

        for scenario in test_scenarios:
            print(f"\n--- {scenario['name']} ---")
            model_total += 1

            # Prepare request
            payload = {
                "model": model,
                "messages": [{"role": "user", "content": scenario["message"]}],
                "tools": tools,
                "tool_choice": "auto",
                "stream": False,
                "max_tokens": 500,
                "temperature": 0.7,
            }

            try:
                response = requests.post(
                    f"{hub_url}/v1/chat/completions",
                    headers={"Content-Type": "application/json"},
                    json=payload,
                    timeout=60,
                )

                if response.status_code == 200:
                    result = response.json()
                    message = result.get("choices", [{}])[0].get("message", {})
                    content = message.get("content", "")
                    tool_calls = message.get("tool_calls", [])
                    finish_reason = result.get("choices", [{}])[0].get(
                        "finish_reason", ""
                    )

                    print(f"✅ Success")
                    print(f"Finish Reason: {finish_reason}")
                    if content:
                        print(
                            f"Content: {content[:150]}{'...' if len(content) > 150 else ''}"
                        )
                    print(f"Tool Calls: {len(tool_calls)}")

                    for tc in tool_calls:
                        func = tc.get("function", {})
                        print(f"  🛠️ {func.get('name')}: {func.get('arguments')}")

                        if func.get("name") == scenario["expected_tool"]:
                            model_success += 1
                            print("  ✅ Correct tool called!")
                        else:
                            print(
                                f"  ⚠️ Expected {scenario['expected_tool']}, got {func.get('name')}"
                            )

                    if len(tool_calls) == 0 and scenario.get("expected_tool"):
                        print(f"  ❌ Expected tool call but got none")
                else:
                    print(f"❌ HTTP {response.status_code}: {response.text}")

            except requests.exceptions.Timeout:
                print("⏰ Request timeout")
            except Exception as e:
                print(f"❌ Error: {e}")

        # Model summary
        if model_total > 0:
            success_rate = (model_success / model_total) * 100
            print(
                f"\n📊 {model} Summary: {model_success}/{model_total} ({success_rate:.1f}% success)"
            )

    print(f"\n{'=' * 60}")
    print("🎯 Testing Complete!")
    print(f"{'=' * 60}")


def check_hub_status(hub_url="http://localhost:3001"):
    """Check if hub is running and basic status"""
    try:
        response = requests.get(f"{hub_url}/v1/models", timeout=3)
        if response.status_code == 200:
            models = response.json().get("data", [])
            print(f"✅ Hub is running with {len(models)} models available")
            return True
        else:
            print(f"❌ Hub returned HTTP {response.status_code}")
            return False
    except requests.exceptions.ConnectionError:
        print("❌ Hub is not running or not accessible")
        return False
    except Exception as e:
        print(f"❌ Error checking hub: {e}")
        return False


def main():
    hub_url = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:3001"

    print("🚀 Enhanced Tool Calling Test Suite")
    print("====================================")
    print(f"Hub URL: {hub_url}")

    if not check_hub_status(hub_url):
        print("\n💡 Please start the hub:")
        print("   cargo run --release --bin indigo-hub -- --with-sidecar")
        print("   or")
        print("   cargo run --release --bin indigo-hub -- --port 3001")
        return

    test_model_tool_calling(hub_url)


if __name__ == "__main__":
    main()
