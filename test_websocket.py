#!/usr/bin/env python3
import asyncio
import websockets
import json
import time


async def test_agent_d():
    """Test WebSocket request to agent 'd' with debug logging"""

    uri = "ws://localhost:3001/ws"

    test_request = {
        "agent_id": "d",
        "messages": [{"role": "user", "content": "Hey!"}],
        "max_tokens": 50,
        "temperature": 0.7,
        "enable_tools": False,
    }

    print("=== Testing WebSocket request to agent 'd' ===")
    print(f"Request: {json.dumps(test_request, indent=2)}")
    print()

    try:
        async with websockets.connect(uri) as websocket:
            print("Connected to WebSocket")

            # Send the request
            await websocket.send(json.dumps(test_request))
            print("Request sent")

            # Listen for responses with timeout
            response_count = 0
            start_time = time.time()

            while time.time() - start_time < 10:  # 10 second timeout
                try:
                    message = await asyncio.wait_for(websocket.recv(), timeout=1.0)
                    response_count += 1
                    print(f"Response {response_count}: {message}")

                    # Parse the response to see status
                    try:
                        resp_data = json.loads(message)
                        if resp_data.get("status") == "Success":
                            print("✓ Success response received")
                            break
                        elif resp_data.get("status") == "Error":
                            print(f"✗ Error response: {resp_data}")
                            break
                    except:
                        pass

                except asyncio.TimeoutError:
                    print("Waiting for response...")
                    continue
                except websockets.exceptions.ConnectionClosed:
                    print("WebSocket connection closed")
                    break

            if response_count == 0:
                print("❌ No responses received at all")

    except Exception as e:
        print(f"X Connection error: {e}")
        return False

    return response_count > 0


if __name__ == "__main__":
    asyncio.run(test_agent_d())
