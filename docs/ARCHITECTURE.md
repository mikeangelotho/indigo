System Topography
The Hub (Backend/Orchestrator): The Central Authority. Written in Rust (Axum). Holds the SQLite state for chat history and the Model Registry.

The Node (Splitter/Worker): The Math Engine. Written in Rust (Candle). Communicates via gRPC. Can be many nodes or just one (local).

The Agent (Tool Caller): The Hands. A module within the Hub that executes local OS commands when the LLM emits a JSON tool call.

The Client (UI): The Window. SolidStart using WebSockets to stream tokens from the Hub.