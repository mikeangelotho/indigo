#!/usr/bin/env python3
"""
Simple test to verify MCP server is working
"""

import subprocess
import json
import time
import threading
import queue
import sys


def test_mcp_stdio_communication():
    """Test basic MCP server communication via stdio"""
    print("🧪 Testing MCP Server Communication")
    print("=" * 40)

    # Start the hub process in background
    hub_process = subprocess.Popen(
        ["cargo", "run", "--bin", "indigo-hub"],
        cwd="indigo-core/crates/indigo-hub",
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    try:
        # Wait for server to start
        time.sleep(2)

        # Send initialize request
        init_request = {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocol_version": "2024-11-05",
                "capabilities": {"tools": {}},
                "client_info": {"name": "test-client", "version": "1.0.0"},
            },
        }

        print("Sending initialize request...")
        hub_process.stdin.write(json.dumps(init_request) + "\n")
        hub_process.stdin.flush()

        # Read response
        response_line = hub_process.stdout.readline()
        if response_line:
            response = json.loads(response_line.strip())
            print("✅ Initialize response received")

            if "result" in response:
                print("✅ Server capabilities:", response["result"]["capabilities"])

                # Test tools/list
                tools_request = {
                    "jsonrpc": "2.0",
                    "id": 2,
                    "method": "tools/list",
                    "params": {},
                }

                print("\nSending tools/list request...")
                hub_process.stdin.write(json.dumps(tools_request) + "\n")
                hub_process.stdin.flush()

                tools_response = hub_process.stdout.readline()
                if tools_response:
                    tools_data = json.loads(tools_response.strip())
                    if "result" in tools_data:
                        tools = tools_data["result"].get("tools", [])
                        print(f"✅ Found {len(tools)} available tools:")
                        for tool in tools[:3]:  # Show first 3 tools
                            print(
                                f"  - {tool.get('name', 'Unknown')}: {tool.get('description', 'No description')}"
                            )

                        if len(tools) > 3:
                            print(f"  ... and {len(tools) - 3} more tools")

                        print("\n🎉 MCP server is working correctly!")
                        return True
                    else:
                        print("❌ Tools list response missing result")
            else:
                print("❌ Initialize response missing result")
        else:
            print("❌ No response to initialize request")

    except Exception as e:
        print(f"❌ Error during test: {e}")
        return False
    finally:
        # Clean up
        if "hub_process" in locals():
            hub_process.terminate()
            hub_process.wait()

    return False


if __name__ == "__main__":
    success = test_mcp_stdio_communication()
    if success:
        print("\n✅ MCP server test PASSED")
        sys.exit(0)
    else:
        print("\n❌ MCP server test FAILED")
        sys.exit(1)
