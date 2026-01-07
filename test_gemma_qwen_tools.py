#!/usr/bin/env python3
"""
Test script for enhanced tool calling with Gemma and Qwen models
"""

import json
import requests
import time
import sys


def test_tool_calling(model_name, hub_url="http://localhost:3001", test_files=True):
    """Test tool calling capabilities with different models"""

    print(f"\n{'=' * 60}")
    print(f"Testing Tool Calling with: {model_name}")
    print(f"{'=' * 60}")

    # Define test tools
    tools = [
        {
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Read the contents of a file",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file to read",
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
                            "description": "Directory path to list files from",
                        }
                    },
                    "required": [],
                },
            },
        },
        {
            "type": "function",
            "function": {
                "name": "write_file",
                "description": "Write content to a file",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string", "description": "File path"},
                        "content": {
                            "type": "string",
                            "description": "Content to write",
                        },
                    },
                    "required": ["path", "content"],
                },
            },
        },
    ]

    # Test cases for different tool calling scenarios
    test_cases = [
        {
            "name": "Natural Language - Read File",
            "messages": [
                {"role": "user", "content": "Please read the file test.txt for me"}
            ],
            "expect_tool": "read_file",
        },
        {
            "name": "Explicit Tool Call",
            "messages": [
                {
                    "role": "user",
                    "content": "I need to see what files are in the current directory",
                }
            ],
            "expect_tool": "list_files",
        },
        {
            "name": "JSON Format Test",
            "messages": [
                {
                    "role": "user",
                    "content": "Create a file called hello.txt with the content 'Hello World'",
                }
            ],
            "expect_tool": "write_file",
        },
    ]

    results = []

    for test_case in test_cases:
        print(f"\n--- {test_case['name']} ---")

        # Prepare request
        payload = {
            "model": model_name,
            "messages": test_case["messages"],
            "tools": tools,
            "tool_choice": "auto",
            "stream": False,
            "max_tokens": 500,
            "temperature": 0.7,
        }

        try:
            # Make request
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
                finish_reason = result.get("choices", [{}])[0].get("finish_reason", "")

                print(f"Status: ✅ Success")
                print(f"Finish Reason: {finish_reason}")
                print(f"Content: {content[:200]}{'...' if len(content) > 200 else ''}")
                print(f"Tool Calls: {len(tool_calls)}")

                for tc in tool_calls:
                    func = tc.get("function", {})
                    print(f"  - {func.get('name')}: {func.get('arguments')}")

                # Evaluate result
                success = (
                    finish_reason == "tool_calls"
                    and len(tool_calls) > 0
                    and (
                        not test_case.get("expect_tool")
                        or any(
                            tc.get("function", {}).get("name")
                            == test_case["expect_tool"]
                            for tc in tool_calls
                        )
                    )
                )

                results.append(
                    {
                        "test": test_case["name"],
                        "success": success,
                        "finish_reason": finish_reason,
                        "tool_calls": len(tool_calls),
                        "expected_tool": test_case.get("expect_tool"),
                        "actual_tools": [
                            tc.get("function", {}).get("name") for tc in tool_calls
                        ],
                    }
                )

            else:
                print(f"Status: ❌ Failed with HTTP {response.status_code}")
                print(f"Error: {response.text}")
                results.append(
                    {
                        "test": test_case["name"],
                        "success": False,
                        "error": f"HTTP {response.status_code}: {response.text}",
                    }
                )

        except requests.exceptions.Timeout:
            print("Status: ⏰ Timeout")
            results.append(
                {
                    "test": test_case["name"],
                    "success": False,
                    "error": "Request timeout",
                }
            )
        except Exception as e:
            print(f"Status: ❌ Exception: {e}")
            results.append(
                {"test": test_case["name"], "success": False, "error": str(e)}
            )

        time.sleep(2)  # Brief pause between tests

    # Summary
    print(f"\n{'=' * 60}")
    print(f"Test Summary for {model_name}")
    print(f"{'=' * 60}")

    successful = sum(1 for r in results if r.get("success", False))
    total = len(results)

    print(
        f"Overall Success Rate: {successful}/{total} ({successful / total * 100:.1f}%)"
    )

    for result in results:
        status = "✅" if result.get("success", False) else "❌"
        print(f"{status} {result['test']}")
        if not result.get("success", False):
            if "error" in result:
                print(f"   Error: {result['error']}")
            else:
                print(f"   Expected: {result.get('expected_tool', 'any tool')}")
                print(f"   Got: {result.get('actual_tools', [])}")

    return results


def test_model_availability(hub_url="http://localhost:3001"):
    """Check what models are available"""
    print("Checking available models...")

    try:
        response = requests.get(f"{hub_url}/v1/models")
        if response.status_code == 200:
            models = response.json().get("data", [])
            model_names = [m.get("id") for m in models]
            print(f"Available models: {model_names}")
            return model_names
        else:
            print(f"Failed to get models: HTTP {response.status_code}")
            return []
    except Exception as e:
        print(f"Error getting models: {e}")
        return []


def main():
    hub_url = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:3001"
    target_models = sys.argv[2:] if len(sys.argv) > 2 else []

    # Check available models
    available_models = test_model_availability(hub_url)

    # Determine which models to test
    if target_models:
        models_to_test = [m for m in target_models if m in available_models]
        if not models_to_test:
            print(f"None of the specified models {target_models} are available!")
            return
    else:
        # Auto-detect Gemma and Qwen models
        models_to_test = []
        for model in available_models:
            name_lower = model.lower()
            if "gemma" in name_lower or "qwen" in name_lower:
                models_to_test.append(model)

        if not models_to_test:
            print("No Gemma or Qwen models found. Testing with first available model.")
            models_to_test = available_models[:1] if available_models else []

    if not models_to_test:
        print("No models available for testing!")
        return

    print(f"Testing with models: {models_to_test}")

    # Run tests
    all_results = {}
    for model in models_to_test:
        results = test_tool_calling(model, hub_url)
        all_results[model] = results

        print("\n" + "=" * 60 + "\n")

    # Final summary
    print("FINAL SUMMARY")
    print("=" * 60)
    for model, results in all_results.items():
        successful = sum(1 for r in results if r.get("success", False))
        total = len(results)
        print(f"{model}: {successful}/{total} tests passed")


if __name__ == "__main__":
    main()
