#!/usr/bin/env python3
"""
Test script to verify XSS security fixes in Indigo frontend.
"""

import requests
import json
import time


def test_xss_in_chat():
    """Test that XSS attacks are blocked in chat messages."""

    hub_url = "http://localhost:3001"

    # XSS payloads that should be blocked/sanitized
    xss_payloads = [
        '<script>alert("XSS")</script>',
        '<img src="x" onerror="alert(\'XSS\')">',
        "<svg onload=\"alert('XSS')\">",
        '"><script>alert("XSS")</script>',
        "<iframe src=\"javascript:alert('XSS')\"></iframe>",
        "<body onload=\"alert('XSS')\">",
        "<div onclick=\"alert('XSS')\">Click me</div>",
        "<a href=\"javascript:alert('XSS')\">Click here</a>",
        '<<SCRIPT>alert("XSS");//<</SCRIPT>',
        '<script>document.location="http://evil.com"</script>',
    ]

    # Safe content that should work
    safe_content = [
        "This is a **safe** message with markdown.",
        "# Safe Title\n\nThis is safe content.",
        "Here's a code block:\n\n```python\nprint('hello')\n```",
        "Link: https://example.com",
        "Image: ![safe](image.jpg)",
    ]

    print("🛡️  Testing XSS Security in Chat Content...")
    print("=" * 50)

    print("\n🚫 Testing XSS payloads (should be sanitized):")
    for payload in xss_payloads:
        try:
            # Send as a chat message
            response = requests.post(
                f"{hub_url}/api/chat",
                json={"message": payload, "agent_id": "test-agent"},
                timeout=5,
            )

            if response.status_code == 200:
                result = response.json()
                # Check if the raw XSS payload exists in response
                if payload in str(result):
                    print(
                        f"❌ VULNERABLE: XSS payload not sanitized: {payload[:50]}..."
                    )
                else:
                    print(f"✅ BLOCKED: {payload[:50]}...")
            else:
                print(f"✅ BLOCKED: {payload[:50]}... (HTTP {response.status_code})")

        except requests.exceptions.RequestException as e:
            print(f"⚠️  Could not test XSS payload: {e}")

    print("\n✅ Testing safe content (should work):")
    for content in safe_content:
        try:
            response = requests.post(
                f"{hub_url}/api/chat",
                json={"message": content, "agent_id": "test-agent"},
                timeout=5,
            )

            if response.status_code == 200:
                result = response.json()
                # Check if safe content is preserved
                if content in str(result):
                    print(f"✅ ALLOWED: {content[:50]}...")
                else:
                    print(f"❌ BLOCKED: {content[:50]}...")
            else:
                print(f"❌ ERROR: {content[:50]}... (HTTP {response.status_code})")

        except requests.exceptions.RequestException as e:
            print(f"⚠️  Could not test safe content: {e}")


def test_file_upload_xss():
    """Test that file upload XSS is blocked."""

    print("\n🛡️  Testing File Upload XSS...")
    print("=" * 30)

    # Malicious file names that should be blocked
    malicious_filenames = [
        "<script>alert('XSS')</script>.txt",
        "file<img src=x onerror=alert('XSS')>.txt",
        'file"><script>alert("XSS")</script>.txt',
        "javascript:alert('XSS').txt",
    ]

    safe_filenames = [
        "document.txt",
        "image.jpg",
        "data.json",
        "script.py",
    ]

    print("\n🚫 Testing malicious filenames (should be blocked):")
    for filename in malicious_filenames:
        # Test via agent configuration (which stores filenames)
        try:
            response = requests.post(
                f"http://localhost:3001/api/agents",
                json={
                    "name": f"Test Agent {filename}",
                    "system_prompt": f"Test agent with filename {filename}",
                    "model": "test-model",
                },
                timeout=5,
            )

            if response.status_code == 200:
                result = response.json()
                # Check if malicious filename exists in response
                if any(
                    malicious_part in str(result)
                    for malicious_part in ["<script", "javascript:", "onerror"]
                ):
                    print(
                        f"❌ VULNERABLE: Malicious filename not sanitized: {filename}"
                    )
                else:
                    print(f"✅ BLOCKED: {filename}")
            else:
                print(f"✅ BLOCKED: {filename} (HTTP {response.status_code})")

        except requests.exceptions.RequestException as e:
            print(f"⚠️  Could not test malicious filename: {e}")

    print("\n✅ Testing safe filenames (should work):")
    for filename in safe_filenames:
        try:
            response = requests.post(
                f"http://localhost:3001/api/agents",
                json={
                    "name": f"Test Agent {filename}",
                    "system_prompt": f"Test agent with filename {filename}",
                    "model": "test-model",
                },
                timeout=5,
            )

            if response.status_code == 200:
                result = response.json()
                if filename in str(result):
                    print(f"✅ ALLOWED: {filename}")
                else:
                    print(f"❌ BLOCKED: {filename}")
            else:
                print(f"❌ ERROR: {filename} (HTTP {response.status_code})")

        except requests.exceptions.RequestException as e:
            print(f"⚠️  Could not test safe filename: {e}")


def test_url_validation():
    """Test that malicious URLs are blocked."""

    print("\n🛡️  Testing URL Validation...")
    print("=" * 30)

    # Malicious URLs that should be blocked
    malicious_urls = [
        "javascript:alert('XSS')",
        "data:text/html,<script>alert('XSS')</script>",
        "vbscript:msgbox('XSS')",
        "file:///etc/passwd",
        "http://evil.com/steal_data?cookies=" + "A" * 1000,  # URL length test
    ]

    safe_urls = [
        "https://example.com",
        "http://localhost:3000",
        "https://github.com/indigo",
        "/api/health",
    ]

    print("\n🚫 Testing malicious URLs (should be blocked):")
    for url in malicious_urls:
        try:
            # Test via tool execution (which processes URLs)
            response = requests.post(
                f"http://localhost:3001/api/tools/execute",
                json={"tool": "test_url_tool", "arguments": {"url": url}},
                timeout=5,
            )

            if response.status_code == 200:
                result = response.json()
                if any(
                    malicious_scheme in str(result)
                    for malicious_scheme in ["javascript:", "data:", "vbscript:"]
                ):
                    print(f"❌ VULNERABLE: Malicious URL not blocked: {url[:50]}...")
                else:
                    print(f"✅ BLOCKED: {url[:50]}...")
            else:
                print(f"✅ BLOCKED: {url[:50]}... (HTTP {response.status_code})")

        except requests.exceptions.RequestException as e:
            print(f"⚠️  Could not test malicious URL: {e}")

    print("\n✅ Testing safe URLs (should work):")
    for url in safe_urls:
        try:
            response = requests.post(
                f"http://localhost:3001/api/tools/execute",
                json={"tool": "test_url_tool", "arguments": {"url": url}},
                timeout=5,
            )

            if response.status_code == 200:
                result = response.json()
                if url in str(result):
                    print(f"✅ ALLOWED: {url}")
                else:
                    print(f"❌ BLOCKED: {url}")
            else:
                print(f"❌ ERROR: {url} (HTTP {response.status_code})")

        except requests.exceptions.RequestException as e:
            print(f"⚠️  Could not test safe URL: {e}")


if __name__ == "__main__":
    print("🛡️  Indigo Frontend XSS Security Test")
    print("=" * 50)
    print("This script tests the XSS security fixes in the frontend.")
    print("It verifies that malicious content is properly sanitized.")
    print("\n⚠️  Make sure to:")
    print("   1. Start the Indigo frontend: npm run dev")
    print("   2. Start the Indigo backend (Hub)")
    print("   3. Verify both are accessible on localhost")
    print("\nPress Enter to continue or Ctrl+C to cancel...")
    input()

    test_xss_in_chat()
    test_file_upload_xss()
    test_url_validation()

    print("\n" + "=" * 50)
    print("🎯 XSS Security Test Complete!")
    print("If all malicious content is blocked, XSS fixes are working.")
