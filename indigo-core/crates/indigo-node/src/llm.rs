use anyhow::{anyhow, Result};
use hf_hub::{Repo, RepoType};
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaModel, Special};
use llama_cpp_2::sampling::LlamaSampler;
use std::fmt;
use std::num::NonZeroU32;
use std::sync::{Arc, Mutex};

use indigo_common::{ChatMessage, ToolDefinition};

pub struct InferenceEngine {
    model: Arc<LlamaModel>,
    backend: Arc<LlamaBackend>,
    model_name: String,
    _mmproj_path: Option<String>,
    n_ctx: u32,
    n_batch: u32,
}

// FIX: Manual Debug implementation because LlamaModel doesn't support it
impl fmt::Debug for InferenceEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InferenceEngine")
            .field("model_name", &self.model_name)
            .field("n_ctx", &self.n_ctx)
            .field("n_batch", &self.n_batch)
            .finish()
    }
}

impl InferenceEngine {
    pub fn new(
        model_repo: Option<String>,
        model_file: String,
        custom_path: Option<String>,
        n_gpu_layers: u32,
        mmproj: Option<String>,
        n_ctx: u32,
        n_batch: u32,
    ) -> Result<Self> {
        let backend = Arc::new(LlamaBackend::init()?);
        let model_params = LlamaModelParams::default().with_n_gpu_layers(n_gpu_layers);

        let name_for_logic = model_repo.clone().unwrap_or(model_file.clone());

        let model_path = if let Some(repo_name) = model_repo {
            let mut builder = hf_hub::api::sync::ApiBuilder::new();
            if let Some(path) = custom_path {
                builder = builder.with_cache_dir(std::path::PathBuf::from(path));
            }
            let api = builder.build()?;
            let repo = api.repo(Repo::new(repo_name, RepoType::Model));
            repo.get(&model_file)?
        } else {
            std::path::Path::new(&model_file).to_path_buf()
        };

        let model = LlamaModel::load_from_file(&backend, &model_path, &model_params)
            .map_err(|e| anyhow!("Failed to load model: {}", e))?;

        Ok(Self {
            model: Arc::new(model),
            backend,
            model_name: name_for_logic,
            _mmproj_path: mmproj,
            n_ctx,
            n_batch,
        })
    }

    pub fn generate_stream(
        engine: Arc<Mutex<Self>>,
        raw_prompt: String,
        _messages: Option<Vec<ChatMessage>>,
        _tools: Option<Vec<ToolDefinition>>, // Prefixed with _ to quiet warning
        max_tokens: usize,
        temperature: f32,
        _image_data: Option<Vec<u8>>,
        stop_tokens: Option<Vec<String>>,
    ) -> tokio::sync::mpsc::Receiver<Result<String>> {
        let (tx, rx) = tokio::sync::mpsc::channel(32);

        tokio::task::spawn_blocking(move || {
            let guard = engine.lock().unwrap();
            let model = guard.model.clone();
            let backend = guard.backend.clone();
            let _model_name = guard.model_name.clone(); // Prefixed with _
            let n_ctx = guard.n_ctx;
            let n_batch = guard.n_batch;

            // n_batch should be large enough to process the prompt in one go
            let ctx_params = LlamaContextParams::default()
                .with_n_ctx(NonZeroU32::new(n_ctx))
                .with_n_batch(n_batch)
                .with_n_ubatch(512)
                .with_n_seq_max(1)
                .with_flash_attention_policy(1)
                .with_rope_freq_base(1000000.0);

            let mut ctx = match model.new_context(&backend, ctx_params) {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.blocking_send(Err(anyhow!("Context error: {}", e)));
                    return;
                }
            };

            let mut formatted_prompt = String::new();
            
            // Try to parse the raw_prompt as a list of ChatMessages (JSON)
            if let Ok(messages) = serde_json::from_str::<Vec<ChatMessage>>(&raw_prompt) {
                formatted_prompt = apply_chat_template(&guard.model_name, &messages);
            } else {
                // Fallback: If it's not JSON messages, use it as raw text
                formatted_prompt = raw_prompt;
            }

            let tokens_list = model
                .str_to_token(&formatted_prompt, AddBos::Always)
                .unwrap();

            // Check context window
            if tokens_list.len() >= n_ctx as usize {
                let _ = tx.blocking_send(Err(anyhow!("Prompt too long: {} tokens, max context {}", tokens_list.len(), n_ctx)));
                return;
            }

            // Process prompt in batches to respect n_batch
            let batch_size = n_batch as usize;
            let mut batch = LlamaBatch::new(batch_size, 1);
            let mut n_processed = 0;

            while n_processed < tokens_list.len() {
                let remaining = tokens_list.len() - n_processed;
                let current_batch_size = remaining.min(batch_size);
                let chunk = &tokens_list[n_processed..n_processed + current_batch_size];
                
                batch.clear();
                
                for (i, token) in chunk.iter().enumerate() {
                    let global_idx = n_processed + i;
                    let is_last_token_of_prompt = global_idx == tokens_list.len() - 1;
                    
                    // We use global_idx as the position
                    batch.add(*token, global_idx as i32, &[0], is_last_token_of_prompt).unwrap();
                }

                if let Err(e) = ctx.decode(&mut batch) {
                    let _ = tx.blocking_send(Err(anyhow!("Prompt decode failed at offset {}: {}", n_processed, e)));
                    return;
                }

                n_processed += current_batch_size;
            }

            let mut n_cur = tokens_list.len() as i32;
            let mut sampler = LlamaSampler::chain_simple([
                LlamaSampler::temp(temperature.max(0.4)),
                LlamaSampler::top_p(0.5, 1),
                LlamaSampler::dist(512),
            ]);

            let mut sample_idx = batch.n_tokens() - 1;
            let mut pending_bytes = Vec::new();

            for _ in 0..max_tokens {
                let new_token_id = sampler.sample(&ctx, sample_idx);

                if new_token_id == model.token_eos() {
                    break;
                }

                if let Ok(bytes) = model.token_to_bytes(new_token_id, Special::Tokenize) {
                    pending_bytes.extend_from_slice(&bytes);

                    // Try to decode the pending bytes
                    // We only clear pending_bytes if we successfully decode everything
                    // If there's an error, it might be incomplete UTF-8, so we keep waiting
                    match String::from_utf8(pending_bytes.clone()) {
                        Ok(text) => {
                            pending_bytes.clear();
                            if text.contains("<|im_end|>")
                                || text.contains("<|im_start|>")
                                || text.contains("<|eot_id|>")
                                || text.contains("<end_of_turn>")
                                || text.contains("<|endoftext|>")
                            {
                                break;
                            }
                            
                            // Check stop tokens
                            let mut stopped = false;
                            if let Some(stops) = &stop_tokens {
                                for stop in stops {
                                    if text.contains(stop) {
                                        stopped = true;
                                        break;
                                    }
                                }
                            }
                            if stopped {
                                // Emit the token that caused the stop?
                                // Usually we emit it so the parser sees "}"
                                // But if it's "]" it depends.
                                // Let's emit it and break.
                                let _ = tx.blocking_send(Ok(text));
                                break;
                            }

                            if tx.blocking_send(Ok(text)).is_err() {
                                break;
                            }
                        }
                        Err(_e) => {
                            // Check if it's a valid incomplete sequence or actual garbage
                            // from_utf8 returns FromUtf8Error which contains the valid bytes so far
                            // but we want to stream chunks if possible.
                            // For simplicity, we just wait for more bytes unless it gets too large
                            if pending_bytes.len() > 16 {
                                // If we have a lot of bytes and still can't decode, it's likely garbage.
                                // Fallback to lossy for the whole chunk to clear it.
                                let text = String::from_utf8_lossy(&pending_bytes).to_string();
                                pending_bytes.clear();
                                if tx.blocking_send(Ok(text)).is_err() {
                                    break;
                                }
                            }
                        }
                    }
                }

                batch.clear();
                batch.add(new_token_id, n_cur, &[0], true).unwrap();
                if let Err(e) = ctx.decode(&mut batch) {
                    let _ = tx.blocking_send(Err(anyhow!("Loop decode failed: {}", e)));
                    break;
                }

                n_cur += 1;
                sample_idx = 0;
            }
        });

        rx
    }
}

fn apply_chat_template(model_name: &str, messages: &[ChatMessage]) -> String {
    let name = model_name.to_lowercase();
    let mut prompt = String::new();

    // Enhanced template support for different model families
    if name.contains("gemma") {
        // Gemma uses a turn-based format
        for msg in messages {
            let role = if msg.role == "assistant" { "model" } else { &msg.role };
            let content = extract_text_content(&msg.content);
            prompt.push_str(&format!("<start_of_turn>{}\n{}<end_of_turn>\n", role, content));
        }
        prompt.push_str("<start_of_turn>model\n");
    } else if name.contains("llama-3") || name.contains("llama 3") {
        // Llama 3 uses special tokens
        prompt.push_str("<|begin_of_text|>");
        for msg in messages {
            let content = extract_text_content(&msg.content);
            prompt.push_str(&format!("<|start_header_id|>{}<|end_header_id|>\n\n{}<|eot_id|>", msg.role, content));
        }
        prompt.push_str("<|start_header_id|>assistant<|end_header_id|>\n\n");
    } else if name.contains("qwen") {
        // Qwen models typically use ChatML format but sometimes have variations
        if name.contains("qwen2") || name.contains("qwen-2") {
            // Qwen2 uses ChatML
            for msg in messages {
                let content = extract_text_content(&msg.content);
                prompt.push_str(&format!("<|im_start|>{}\n{}<|im_end|>\n", msg.role, content));
            }
            prompt.push_str("<|im_start|>assistant\n");
        } else {
            // Older Qwen models might use different format
            for msg in messages {
                let content = extract_text_content(&msg.content);
                prompt.push_str(&format!("<|im_start|>{}\n{}<|im_end|>\n", msg.role, content));
            }
            prompt.push_str("<|im_start|>assistant\n");
        }
    } else if name.contains("phi-3") {
        // Phi-3 uses ChatML format
        for msg in messages {
            let content = extract_text_content(&msg.content);
            prompt.push_str(&format!("<|im_start|>{}\n{}<|im_end|>\n", msg.role, content));
        }
        prompt.push_str("<|im_start|>assistant\n");
    } else if name.contains("mistral") || name.contains("mixtral") {
        // Mistral models use ChatML
        for msg in messages {
            let content = extract_text_content(&msg.content);
            prompt.push_str(&format!("<|im_start|>{}\n{}<|im_end|>\n", msg.role, content));
        }
        prompt.push_str("<|im_start|>assistant\n");
    } else if name.contains("chatml") {
        // Explicit ChatML format
        for msg in messages {
            let content = extract_text_content(&msg.content);
            prompt.push_str(&format!("<|im_start|>{}\n{}<|im_end|>\n", msg.role, content));
        }
        prompt.push_str("<|im_start|>assistant\n");
    } else {
        // Default to ChatML as a robust default for modern models
        for msg in messages {
            let content = extract_text_content(&msg.content);
            prompt.push_str(&format!("<|im_start|>{}\n{}<|im_end|>\n", msg.role, content));
        }
        prompt.push_str("<|im_start|>assistant\n");
    }
    
    // Add debug info for template selection
    eprintln!("Applied template for model '{}': {} turns", model_name, messages.len());
    eprintln!("Prompt starts with: {}", &prompt[..prompt.len().min(100)]);
    
    prompt
}

fn extract_text_content(content: &serde_json::Value) -> String {
    if let Some(s) = content.as_str() {
        return s.to_string();
    }
    if let Some(arr) = content.as_array() {
        let mut text = String::new();
        for part in arr {
             if let Some(t) = part.get("type").and_then(|v| v.as_str()) {
                 if t == "text" {
                     if let Some(val) = part.get("text").and_then(|v| v.as_str()) {
                         text.push_str(val);
                     }
                 }
             }
        }
        return text;
    }
    String::new()
}
