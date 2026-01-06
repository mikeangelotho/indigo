#!/usr/bin/env python3
import struct
import sys
import json


def send_sidecar_request(json_request):
    """Send request to sidecar in correct protocol format"""
    request_data = json.dumps(json_request).encode("utf-8")

    # Sidecar expects: [4-byte length][protobuf data]
    length_bytes = struct.pack(">I", len(request_data))

    # Write to stdin
    sys.stdout.buffer.write(length_bytes + request_data)
    sys.stdout.buffer.flush()


if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: python test_sidecar.py <json_request>")
        sys.exit(1)

    json_request = json.loads(sys.argv[1])
    send_sidecar_request(json_request)
