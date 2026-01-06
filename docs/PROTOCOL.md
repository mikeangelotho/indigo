The "Life of a Prompt"
Ingress: Client sends PromptRequest via WebSocket.

Scheduling: Hub checks the "Node Pool." If nodes are available, it sends a TaskAllocation to the Splitter.

Inference: Nodes compute. Tokens are streamed back to the Hub.

Interception: Hub scans tokens for "Tool Call" patterns.

Execution: If a tool is detected, the Hub pauses streaming, runs the Rust Agent code, appends the result to the prompt, and restarts inference.

Egress: Final tokens streamed to UI; status set to Complete.