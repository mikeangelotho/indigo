#!/bin/bash

# Test the tool format conversion functionality

echo "Testing Indigo Hub with OpenCode AI format..."

# Test tool list endpoint
curl -s http://localhost:3001/v1/tools | jq '.[] | .name'

echo "Setting INDIGO_TOOL_FORMAT=opencode and restarting..."

# Set environment variable for OpenCode format
export INDIGO_TOOL_FORMAT=opencode

echo "Testing with OpenCode format..."
# This should now convert tool calls to OpenCode format automatically