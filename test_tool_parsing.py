#!/usr/bin/env python3
"""
Test script to diagnose tool call parsing issues with local models.
This simulates what happens when models return different formats.
"""

import json
import re


def test_tool_call_parsing():
    """Test various tool call formats that local models might produce."""

    # Current parser regex from openai.rs
    parser_regex = re.compile(r"(?s)\{.*\}")

    test_cases = [
        # Expected format (should work)
        '{"function_name": "read_file", "arguments": {"path": "test.txt"}}',
        # Common variations that might fail
        '{"function_name": "read_file", "arguments": "{\\"path\\": \\"test.txt\\"}"}',
        '{"function_name": "read_file", "arguments_json": "{\\"path\\": \\"test.txt\\"}"}',
        '{"function_name": "read_file", "arguments": "{\n  \\"path\\": \\"test.txt\\"\n}"}',
        # Models that echo instructions
        'To read a file, I need to use the read_file tool. Here\'s the JSON: {"function_name": "read_file", "arguments": {"path": "test.txt"}}',
        # Models with extra text
        'I will read the file for you.\n\n{"function_name": "read_file", "arguments": {"path": "test.txt"}}\n\nDone.',
        # Malformed JSON
        '{"function_name": "read_file", "arguments": {"path": "test.txt"',
        '{"functionName": "read_file", "arguments": {"path": "test.txt"}}',  # camelCase variation
        # OpenAI-style format (should be adapted)
        '{"name": "read_file", "arguments": {"path": "test.txt"}}',
        # Anthropic-style format
        '{"name": "read_file", "input": {"path": "test.txt"}}',
        # Natural language instead of JSON
        "I need to read the file test.txt to see its contents.",
    ]

    print("=== Tool Call Parsing Test ===\n")

    for i, test_input in enumerate(test_cases, 1):
        print(f"Test {i}: {test_input[:80]}{'...' if len(test_input) > 80 else ''}")

        # Test current parser logic
        match = parser_regex.search(test_input)
        if match:
            json_str = match.group(0)
            print(f"  Found JSON block: {json_str}")

            try:
                json_value = json.loads(json_str)
                print(f"  Parsed JSON: {json_value}")

                # Check for function_name
                if "function_name" in json_value:
                    func_name = json_value["function_name"]
                    print(f"  [OK] Function name: {func_name}")

                    # Extract arguments
                    args = json_value.get("arguments", "{}")
                    if isinstance(args, dict):
                        args_str = json.dumps(args)
                    elif isinstance(args, str):
                        args_str = args
                    else:
                        args_str = json.dumps(args)

                    print(f"  Arguments: {args_str}")
                else:
                    print(f"  [FAIL] No 'function_name' field found")

                    # Try alternative field names
                    alt_fields = ["name", "tool", "function"]
                    for field in alt_fields:
                        if field in json_value:
                            print(
                                f"  [HINT] Found alternative field '{field}': {json_value[field]}"
                            )

            except json.JSONDecodeError as e:
                print(f"  [FAIL] JSON parsing failed: {e}")
        else:
            print(f"  [FAIL] No JSON block found")

            # Try to find potential tool keywords
            tool_keywords = ["read_file", "write_file", "bash", "web_search"]
            found_keywords = [kw for kw in tool_keywords if kw in test_input.lower()]
            if found_keywords:
                print(f"  [HINT] Found tool keywords: {found_keywords}")

        print()


def enhanced_tool_parser(content: str) -> tuple[str, list[dict] | None, str]:
    """Enhanced tool parser that handles multiple formats."""

    # Strategy 1: Look for exact format first
    exact_pattern = re.compile(
        r'(?:^|\n)\s*\{\s*"function_name"\s*:\s*"[^"]+"\s*,\s*"arguments"'
    )
    match = exact_pattern.search(content)
    if match:
        json_str = extract_full_json(content, match.start())
        if json_str:
            try:
                data = json.loads(json_str)
                if "function_name" in data:
                    return extract_tool_call(content, json_str, data)
            except:
                pass

    # Strategy 2: Look for OpenAI-style format
    openai_pattern = re.compile(
        r'(?:^|\n)\s*\{\s*"name"\s*:\s*"[^"]+"\s*,\s*"arguments"'
    )
    match = openai_pattern.search(content)
    if match:
        json_str = extract_full_json(content, match.start())
        if json_str:
            try:
                data = json.loads(json_str)
                if "name" in data:
                    # Convert to standard format
                    data["function_name"] = data.pop("name")
                    return extract_tool_call(content, json_str, data)
            except:
                pass

    # Strategy 3: Look for any JSON with tool-like content
    json_pattern = re.compile(r"\{[^{}]*(?:\{[^{}]*\}[^{}]*)*\}")
    matches = json_pattern.finditer(content)

    for match in matches:
        json_str = match.group(0)
        try:
            data = json.loads(json_str)

            # Check for various function name fields
            func_name = None
            for field in ["function_name", "name", "tool", "function"]:
                if field in data:
                    func_name = data[field]
                    if field != "function_name":
                        data["function_name"] = func_name
                    break

            if func_name:
                # Handle different argument field names
                args_field = None
                for field in ["arguments", "args", "input", "parameters"]:
                    if field in data:
                        args_field = data[field]
                        if field != "arguments":
                            data["arguments"] = args_field
                        break

                if "arguments" not in data:
                    data["arguments"] = {}

                return extract_tool_call(content, json_str, data)

        except:
            continue

    # Strategy 4: Natural language extraction (basic)
    tool_keywords = ["read file", "write file", "search", "execute", "run command"]
    content_lower = content.lower()

    for keyword in tool_keywords:
        if keyword in content_lower:
            print(f"  [NATURAL] Natural language tool detected: '{keyword}'")
            print(f"     Full content: {content}")
            # This would require LLM-based extraction in a real implementation
            return (content, None, "natural_language")

    return (content, None, "stop")


def extract_full_json(content: str, start_pos: int) -> str | None:
    """Extract complete JSON object starting from position."""
    bracket_count = 0
    in_string = False
    escape_next = False
    i = start_pos

    while i < len(content):
        char = content[i]

        if escape_next:
            escape_next = False
        elif char == "\\":
            escape_next = True
        elif char == '"' and not escape_next:
            in_string = not in_string
        elif not in_string:
            if char == "{":
                if bracket_count == 0:
                    start_pos = i
                bracket_count += 1
            elif char == "}":
                bracket_count -= 1
                if bracket_count == 0:
                    return content[start_pos : i + 1]

        i += 1

    return None


def extract_tool_call(
    content: str, json_str: str, data: dict
) -> tuple[str, list[dict], str]:
    """Extract tool call from parsed JSON."""
    import uuid

    func_name = data["function_name"]
    args = data.get("arguments", "{}")

    if isinstance(args, dict):
        args_str = json.dumps(args)
    else:
        args_str = str(args)

    tool_call = {
        "index": 0,
        "id": f"call_{uuid.uuid4().hex[:8]}",
        "type": "function",
        "function": {"name": func_name, "arguments": args_str},
    }

    # Extract content before JSON
    json_start = content.find(json_str)
    content_before = content[:json_start].strip()

    return (content_before, [tool_call], "tool_calls")


if __name__ == "__main__":
    test_tool_call_parsing()

    print("\n=== Testing Enhanced Parser ===\n")

    test_cases = [
        '{"function_name": "read_file", "arguments": {"path": "test.txt"}}',
        'I will read the file: {"name": "read_file", "arguments": {"path": "test.txt"}}',
        '{"name": "read_file", "input": {"path": "test.txt"}}',
        "I need to search the web for information.",
    ]

    for i, test_input in enumerate(test_cases, 1):
        print(f"Enhanced Test {i}: {test_input}")
        content, tool_calls, finish_reason = enhanced_tool_parser(test_input)

        if tool_calls:
            print(f"  [OK] Tool calls detected: {len(tool_calls)}")
            for tc in tool_calls:
                print(f"     - {tc['function']['name']}: {tc['function']['arguments']}")
        else:
            print(f"  [FAIL] No tool calls detected (finish_reason: {finish_reason})")

        print()
