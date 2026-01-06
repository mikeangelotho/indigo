// Fix for proxy function error handling
pub fn proxy_chat_completions_fixed(
    // ... existing parameters ...
    mut state: ProxyState,
) -> Result<Response, ResponseError> {
    // ... existing code until error handling ...

    match client.run_inference(Request::new(grpc_req)).await {
        Ok(resp) => {
            let mut grpc_stream = resp.into_inner();
            let mut parser = ToolParser::new();
            let mut tool_emitted = false;
            let mut tool_index = 0;
            let mut stream_had_error = false;

            while let Some(Ok(item)) = grpc_stream.next().await {
                stream_had_error = true;
                if item.status == 1 {
                    // Flush parser
                    if let Some(s) = parser.flush() {
                        yield Event::default()
                            .json_data(OpenAIStreamResponse {
                                id: id.clone(),
                                object: "chat.completion.chunk".to_string(),
                                created,
                                model: model_name.clone(),
                                choices: vec![StreamChoice {
                                    index: 0,
                                    delta: Delta {
                                        content: Some(s),
                                        role: None,
                                        tool_calls: None,
                                    },
                                    finish_reason: None,
                                }],
                            })
                            .map_err(axum::Error::new)?;
                    }
                    break;
                }

                if !item.token.is_empty() {
                    let (text, tool) = parser.push(&item.token);

                    if let Some(t) = text {
                        yield Event::default()
                            .json_data(OpenAIStreamResponse {
                                id: id.clone(),
                                object: "chat.completion.chunk".to_string(),
                                created,
                                model: model_name.clone(),
                                choices: vec![StreamChoice {
                                    index: 0,
                                    delta: Delta {
                                        content: Some(t),
                                        role: None,
                                        tool_calls: None,
                                    },
                                    finish_reason: None,
                                }],
                            })
                            .map_err(axum::Error::new)?;
                    }

                    if let Some(mut tc) = tool {
                        tc.index = tool_index;
                        tool_index += 1;
                        tool_emitted = true;
                        yield Event::default()
                            .json_data(OpenAIStreamResponse {
                                id: id.clone(),
                                object: "chat.completion.chunk".to_string(),
                                created,
                                model: model_name.clone(),
                                choices: vec![StreamChoice {
                                    index: 0,
                                    delta: Delta {
                                        content: None,
                                        role: None,
                                        tool_calls: Some(vec![tc]),
                                    },
                                    finish_reason: None,
                                }],
                            })
                            .map_err(axum::Error::new)?;
                    }
                }
            }

            if !stream_had_error {
                yield Event::default().data("[DONE]");
            }

            Sse::new(stream)
                .keep_alive(axum::response::sse::KeepAlive::default())
                .into_response()
        }
        Err(e) => {
            eprintln!("Proxy gRPC Stream Error: {}", e);
            // FIX: Return error response instead of just logging
            yield Event::default()
                .json_data(OpenAIErrorResponse::new(
                    format!("Proxy inference failed: {}", e),
                    Some("proxy_inference_error".into()),
                ))
                .map_err(axum::Error::new)?;
            return;
        }
    }
}
