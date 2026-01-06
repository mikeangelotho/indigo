// Temporary simple fix for sidecar communication
// This is a workaround for the complex borrowing issues

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use indigo_common::inference::{InferenceRequest, InferenceResponse};
use prost::Message;

pub async fn simple_sidecar_request(
    stdin: &mut tokio::process::ChildStdin,
    stdout: &mut tokio::process::ChildStdout,
    req: &InferenceRequest,
) -> Result<Vec<InferenceResponse>, Box<dyn std::error::Error>> {
    let req_bytes = req.encode_to_vec();
    let len_bytes = (req_bytes.len() as u32).to_be_bytes();

    // Send request
    stdin.write_all(&len_bytes).await?;
    stdin.write_all(&req_bytes).await?;
    stdin.flush().await?;

    // Read all responses
    let mut responses = Vec::new();
    loop {
        let mut len_buf = [0u8; 4];
        stdout.read_exact(&mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut msg_buf = vec![0u8; len];
        stdout.read_exact(&mut msg_buf).await?;
        
        let resp = InferenceResponse::decode(std::io::Cursor::new(msg_buf))?;
        responses.push(resp.clone());
        
        if resp.status == 1 || resp.status == 2 { // SUCCESS or ERROR
            break;
        }
    }

    Ok(responses)
}