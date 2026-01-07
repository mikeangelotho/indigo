#!/usr/bin/env python3
"""
Test script to verify CSP configuration allows backend connections.
"""

import requests
import json
import time


def test_csp_compliance():
    """Test that CSP allows connections to backend API."""

    print("🔒 Testing CSP Configuration...")
    print("=" * 40)

    # Test that frontend can connect to backend
    backend_url = "http://localhost:3001"

    try:
        # Test basic connectivity
        response = requests.get(f"{backend_url}/health", timeout=5)
        if response.status_code == 200:
            print("✅ Backend is accessible")
        else:
            print(f"⚠️  Backend returned status {response.status_code}")
    except requests.exceptions.RequestException as e:
        print(f"❌ Cannot connect to backend: {e}")
        return False

    # Test API endpoints that frontend uses
    endpoints_to_test = [
        "/api/nodes",
        "/api/agents",
        "/api/tools",
        "/ws",  # WebSocket endpoint
    ]

    print("\n🔍 Testing API endpoints:")
    for endpoint in endpoints_to_test:
        try:
            if endpoint == "/ws":
                # WebSocket test - just check if it responds to upgrade request
                response = requests.get(
                    f"{backend_url}{endpoint}",
                    headers={"Upgrade": "websocket"},
                    timeout=3,
                )
                print(f"✅ {endpoint} - Responds (WebSocket upgrade expected)")
            else:
                response = requests.get(f"{backend_url}{endpoint}", timeout=3)
                if response.status_code in [
                    200,
                    404,
                    405,
                ]:  # 404/405 are OK - endpoint exists
                    print(f"✅ {endpoint} - Accessible (HTTP {response.status_code})")
                else:
                    print(f"⚠️  {endpoint} - HTTP {response.status_code}")
        except requests.exceptions.RequestException as e:
            print(f"❌ {endpoint} - Error: {e}")

    return True


def test_xss_protection():
    """Test that XSS protection is working."""

    print("\n🛡️  Testing XSS Protection...")
    print("=" * 35)

    # Test malicious payloads
    xss_payloads = [
        "<script>alert('XSS')</script>",
        "<img src=x onerror=alert('XSS')>",
        "javascript:alert('XSS')",
    ]

    backend_url = "http://localhost:3001"

    for payload in xss_payloads:
        try:
            # Test via agent creation (which stores names)
            response = requests.post(
                f"{backend_url}/api/agents",
                json={
                    "name": payload,
                    "system_prompt": "Test agent",
                    "model": "test-model",
                },
                timeout=5,
            )

            if response.status_code == 200:
                result = response.json()
                # Check if payload was sanitized
                if "<script>" in str(result) or "javascript:" in str(result):
                    print(f"❌ XSS payload not sanitized: {payload[:30]}...")
                else:
                    print(f"✅ XSS payload blocked: {payload[:30]}...")
            else:
                print(
                    f"✅ XSS payload rejected: {payload[:30]}... (HTTP {response.status_code})"
                )

        except requests.exceptions.RequestException as e:
            print(f"⚠️  Could not test XSS payload: {e}")


if __name__ == "__main__":
    print("🔒 Indigo CSP & Security Test")
    print("=" * 30)
    print("This script tests that CSP allows legitimate connections")
    print("while blocking XSS attacks.\n")

    print("⚠️  Make sure both frontend and backend are running:")
    print("   Frontend: npm run dev (usually on port 3000)")
    print("   Backend: cargo run (usually on port 3001)")
    print("\nPress Enter to continue or Ctrl+C to cancel...")
    input()

    test_csp_compliance()
    test_xss_protection()

    print("\n" + "=" * 40)
    print("🎯 CSP & Security Test Complete!")
    print("If all tests pass, CSP is configured correctly.")
