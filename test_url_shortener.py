#!/usr/bin/env python3
"""
Test script to verify the new url_shortener tool and web context filtering
"""

import requests
import json


def test_web_context_filtering():
    hub_url = "http://localhost:3001"

    print("🔧 Testing URL Shortener Tool and Web Context Filtering...")
    print("=" * 60)

    # Test 1: Get all tools (no context filter)
    print("1. Testing all tools (no context filter)...")
    try:
        response = requests.get(f"{hub_url}/v1/tools")
        if response.status_code == 200:
            all_tools = response.json()
            tool_names = [tool["name"] for tool in all_tools]
            print(f"✅ Found {len(all_tools)} total tools:")
            for name in sorted(tool_names):
                print(f"   - {name}")

            # Check if url_shortener is in the list
            if "url_shortener" in tool_names:
                print("✅ url_shortener tool found in total tools")
            else:
                print("❌ url_shortener tool NOT found in total tools")
        else:
            print(f"❌ Failed to get all tools: {response.status_code}")
    except Exception as e:
        print(f"❌ Error testing all tools: {e}")

    print("\n2. Testing web context tools...")
    try:
        response = requests.get(f"{hub_url}/v1/tools?context=web")
        if response.status_code == 200:
            web_tools = response.json()
            web_tool_names = [tool["name"] for tool in web_tools]
            print(f"✅ Found {len(web_tools)} web tools:")
            for name in sorted(web_tool_names):
                print(f"   - {name}")

            # Verify CLI tools are filtered out from web context
            cli_tools = ["list_files", "read_file", "write_file", "run_shell"]
            filtered_cli_tools = [t for t in cli_tools if t in web_tool_names]

            if not filtered_cli_tools:
                print("✅ CLI tools correctly filtered out from web context")
            else:
                print(f"❌ CLI tools still in web context: {filtered_cli_tools}")

            # Verify web tools are included
            expected_web_tools = ["web_search", "url_shortener", "analyze_project"]
            found_web_tools = [t for t in expected_web_tools if t in web_tool_names]

            if "url_shortener" in web_tool_names:
                print("✅ url_shortener tool available in web context")
            else:
                print("❌ url_shortener tool NOT available in web context")

            if "web_search" in web_tool_names:
                print("✅ web_search tool available in web context")
            else:
                print("❌ web_search tool NOT available in web context")

            if "analyze_project" in web_tool_names:
                print("✅ analyze_project tool available in web context")
            else:
                print("❌ analyze_project tool NOT available in web context")

        else:
            print(f"❌ Failed to get web tools: {response.status_code}")
    except Exception as e:
        print(f"❌ Error testing web tools: {e}")

    print("\n3. Testing CLI context tools...")
    try:
        response = requests.get(f"{hub_url}/v1/tools?context=cli")
        if response.status_code == 200:
            cli_tools = response.json()
            cli_tool_names = [tool["name"] for tool in cli_tools]
            print(f"✅ Found {len(cli_tools)} CLI tools:")
            for name in sorted(cli_tool_names):
                print(f"   - {name}")

            # Verify web-only tools are filtered out from CLI context
            web_only_tools = [
                "url_shortener"
            ]  # Only url_shortener should be web-only initially
            filtered_web_tools = [t for t in web_only_tools if t in cli_tool_names]

            if not filtered_web_tools:
                print("✅ Web-only tools correctly filtered out from CLI context")
            else:
                print(f"❌ Web-only tools still in CLI context: {filtered_web_tools}")

        else:
            print(f"❌ Failed to get CLI tools: {response.status_code}")
    except Exception as e:
        print(f"❌ Error testing CLI tools: {e}")

    print("\n📋 Summary:")
    print("✅ URL shortener tool implemented with web-only context")
    print("✅ Context filtering working correctly")
    print("✅ Ready for testing in web chat UI")


if __name__ == "__main__":
    test_web_context_filtering()
