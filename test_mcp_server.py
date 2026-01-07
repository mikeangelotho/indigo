#!/usr/bin/env python3
"""
Test script for MCP server functionality
"""

import json
import subprocess
import sys
import time
import requests
from typing import Dict, Any


def test_mcp_server_stdio():
    """Test MCP server via stdio transport"""
    print("Testing MCP server via stdio transport...")

    # Start the indigo-hub in background
    hub_process = subprocess.Popen(
        ["cargo", "run", "--bin", "indigo-hub"],
        cwd="indigo-core",
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    try:
        # Wait a bit for server to start
        time.sleep(2)

        # Test initialize request
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

        hub_process.stdin.write(json.dumps(init_request) + "\n")
        hub_process.stdin.flush()

        # Read response
        response_line = hub_process.stdout.readline()
        if response_line:
            response = json.loads(response_line.strip())
            print("Initialize response:", json.dumps(response, indent=2))

            if "result" in response:
                print("✅ MCP server initialized successfully!")

                # Test tools/list
                tools_request = {
                    "jsonrpc": "2.0",
                    "id": 2,
                    "method": "tools/list",
                    "params": {},
                }

                hub_process.stdin.write(json.dumps(tools_request) + "\n")
                hub_process.stdin.flush()

                tools_response = hub_process.stdout.readline()
                if tools_response:
                    tools_data = json.loads(tools_response.strip())
                    print("Tools list response:", json.dumps(tools_data, indent=2))
                    print("✅ MCP server tools listing works!")
            else:
                print("❌ MCP server initialization failed")
        else:
            print("❌ No response from MCP server")

    except Exception as e:
        print(f"❌ Error testing MCP server: {e}")
    finally:
        hub_process.terminate()
        hub_process.wait()


def test_http_endpoints():
    """Test HTTP endpoints for MCP functionality"""
    print("\nTesting HTTP endpoints...")

    try:
        # Test list MCP servers
        response = requests.get("http://localhost:3001/v1/mcp-servers", timeout=5)
        if response.status_code == 200:
            data = response.json()
            print("✅ MCP servers endpoint works:", data)
        else:
            print("❌ MCP servers endpoint failed:", response.status_code)

        # Test register MCP server
        mcp_config = {
            "server_name": "test-server",
            "server_type": "stdio",
            "command": "echo",
            "args": ["hello"],
            "environment": {"TEST": "true"},
        }

        response = requests.post(
            "http://localhost:3001/v1/mcp-servers", json=mcp_config, timeout=5
        )
        if response.status_code == 200:
            data = response.json()
            print("✅ MCP server registration works:", data)
        else:
            print(
                "❌ MCP server registration failed:",
                response.status_code,
                response.text,
            )

    except requests.exceptions.ConnectionError:
        print("❌ Could not connect to HTTP server - make sure indigo-hub is running")
    except Exception as e:
        print(f"❌ Error testing HTTP endpoints: {e}")


if __name__ == "__main__":
    print("🧪 Testing Indigo MCP Server Implementation")
    print("=" * 50)

    # Test 1: HTTP endpoints (requires running hub)
    test_http_endpoints()

    # Test 2: MCP server stdio (starts new hub process)
    test_mcp_server_stdio()

    print("\n🎉 MCP server testing completed!")
