#!/usr/bin/env python3
"""
Simple test to verify our enhanced tool parsing works.
"""

import json
import urllib.request
import urllib.parse


def test_enhanced_tool_parsing():
    """Test that our enhanced parser can handle multiple tool call formats."""

    hub_url = "http://localhost:3001"

    test_cases = [
        # Test 1: Natural language (new feature)
        {
            "model": "local-sidecar",
            "messages": [
                {"role": "user", "content": "Please read the file test.txt for me"}
            ],
            "tools": [
                {
                    "type": "function",
                    "function": {
                        "name": "read_file",
                        "description": "Read a file",
                        "parameters": {
                            "type": "object",
                            "properties": {"path": {"type": "string"}},
                        },
                    },
                }
            ],
        },
        # Test 2: OpenAI format (new feature)
        {
            "model": "local-sidecar",
            "messages": [
                {
                    "role": "user",
                    "content": 'I\'ll read the file now: {"name": "read_file", "arguments": {"path": "test.txt"}}',
                }
            ],
            "tools": [
                {
                    "type": "function",
                    "function": {
                        "name": "read_file",
                        "description": "Read a file",
                        "parameters": {
                            "type": "object",
                            "properties": {"path": {"type": "string"}},
                        },
                    },
                }
            ],
        },
        # Test 3: Anthropic format (new feature)
        {
            "model": "local-sidecar",
            "messages": [
                {
                    "role": "user",
                    "content": 'I need to check this file: {"name": "read_file", "input": {"path": "test.txt"}}',
                }
            ],
            "tools": [
                {
                    "type": "function",
                    "function": {
                        "name": "read_file",
                        "description": "Read a file",
                        "parameters": {
                            "type": "object",
                            "properties": {"path": {"type": "string"}},
                        },
                    },
                }
            ],
        },
        # Test 4: Indigo format (original)
        {
            "model": "local-sidecar",
            "messages": [
                {
                    "role": "user",
                    "content": 'Let me read the file: {"function_name": "read_file", "arguments": {"path": "test.txt"}}',
                }
            ],
            "tools": [
                {
                    "type": "function",
                    "function": {
                        "name": "read_file",
                        "description": "Read a file",
                        "parameters": {
                            "type": "object",
                            "properties": {"path": {"type": "string"}},
                        },
                    },
                }
            ],
        },
    ]

    for i, test_case in enumerate(test_cases, 1):
        print(f"\n=== Test {i}: Enhanced Tool Parsing ===")

        try:
            data = json.dumps(test_case).encode("utf-8")
            req = urllib.request.Request(
                f"{hub_url}/v1/chat/completions",
                data=data,
                headers={
                    "Content-Type": "application/json",
                    "Content-Length": str(len(data)),
                },
            )

            with urllib.request.urlopen(req, timeout=30) as response:
                if response.status == 200:
                    result = json.loads(response.read().decode("utf-8"))
                    if "choices" in result and len(result["choices"]) > 0:
                        message = result["choices"][0]["message"]
                        if "tool_calls" in message:
                            print(f"✅ SUCCESS: Tool call detected!")
                            for tool_call in message["tool_calls"]:
                                print(f"   Tool: {tool_call['function']['name']}")
                                print(f"   Args: {tool_call['function']['arguments']}")
                        else:
                            content = message.get("content", "")
                            print(f"📝 Regular response: {content[:200]}...")
                    else:
                        print(f"❌ No choices in response")
                else:
                    print(f"❌ HTTP {response.status}: {response.reason}")

        except Exception as e:
            print(f"❌ Error: {e}")


if __name__ == "__main__":
    print("Testing Enhanced Tool Call Parsing")
    print("Make sure to start hub with: cargo run --bin indigo-hub -- --with-sidecar")
    test_enhanced_tool_parsing()
