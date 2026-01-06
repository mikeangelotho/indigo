# Fix for OpenAI Proxy Error Handling

## Issue
The proxy_chat_completions function in openai.rs only logs gRPC errors but doesn't return error responses, causing client hangs.

## Files to Modify
- `indigo-core/crates/indigo-hub/src/openai.rs`

## Specific Changes

### 1. Add error tracking variable (around line 445)
Add this line after variable declarations:
```rust
let mut stream_had_error = false;
```

### 2. Fix error handling (around line 516)
Replace:
```rust
Err(e) => {
    eprintln!("Proxy gRPC Stream Error: {}", e);
}
```

With:
```rust
Err(e) => {
    eprintln!("Proxy gRPC Stream Error: {}", e);
    // FIX: Return error response instead of just logging
    yield Event::default().json_data(OpenAIErrorResponse::new(
        format!("Proxy inference failed: {}", e), 
        Some("proxy_inference_error".into())
    )).map_err(axum::Error::new)?;
    break;
}
```

### 3. Add success guard (around line 525)
Replace:
```rust
yield Event::default().data("[DONE]"));
```

With:
```rust
if !stream_had_error {
    yield Event::default().data("[DONE]");
}
```

## Result
This will prevent the hanging behavior by properly returning error responses when gRPC calls fail, instead of just logging and continuing.