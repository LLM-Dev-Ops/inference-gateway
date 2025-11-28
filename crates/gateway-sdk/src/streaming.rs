//! Streaming support for the Gateway SDK.

use crate::error::{Error, Result};
use bytes::Bytes;
use futures::stream::Stream;
use pin_project_lite::pin_project;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use std::task::{Context, Poll};

/// A chunk from a streaming response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunk {
    /// Unique identifier for this completion.
    pub id: String,
    /// Object type (always "chat.completion.chunk").
    pub object: String,
    /// Unix timestamp of when the chunk was created.
    pub created: i64,
    /// Model used for the completion.
    pub model: String,
    /// List of completion choices.
    pub choices: Vec<StreamChoice>,
    /// System fingerprint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,
    /// Usage information (only in final chunk with stream_options).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<crate::response::Usage>,
}

impl StreamChunk {
    /// Get the content delta from the first choice.
    pub fn content(&self) -> &str {
        self.choices
            .first()
            .and_then(|c| c.delta.content.as_deref())
            .unwrap_or("")
    }

    /// Get the finish reason if present.
    pub fn finish_reason(&self) -> Option<&str> {
        self.choices
            .first()
            .and_then(|c| c.finish_reason.as_deref())
    }

    /// Check if this is the final chunk.
    pub fn is_final(&self) -> bool {
        self.choices
            .first()
            .map(|c| c.finish_reason.is_some())
            .unwrap_or(false)
    }

    /// Check if this chunk contains content.
    pub fn has_content(&self) -> bool {
        !self.content().is_empty()
    }
}

/// A choice in a streaming response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChoice {
    /// Index of this choice.
    pub index: u32,
    /// The delta content.
    pub delta: StreamDelta,
    /// Reason for completion (only in final chunk).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    /// Log probabilities (if requested).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<serde_json::Value>,
}

/// Delta content in a streaming response.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StreamDelta {
    /// Role of the message (only in first chunk).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Content fragment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Tool calls (streaming).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<StreamToolCall>>,
}

/// A tool call in a streaming response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamToolCall {
    /// Index of this tool call.
    pub index: u32,
    /// Tool call ID (only in first chunk for this tool call).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Type of tool.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub tool_type: Option<String>,
    /// Function details.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function: Option<StreamFunctionCall>,
}

/// A function call in a streaming response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamFunctionCall {
    /// Function name (only in first chunk).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Arguments fragment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}

pin_project! {
    /// A stream of chat completion chunks.
    pub struct ChatStream {
        #[pin]
        inner: Pin<Box<dyn Stream<Item = Result<StreamChunk>> + Send>>,
        buffer: String,
        done: bool,
    }
}

impl ChatStream {
    /// Create a new chat stream from a byte stream.
    pub fn new<S>(stream: S) -> Self
    where
        S: Stream<Item = std::result::Result<Bytes, reqwest::Error>> + Send + 'static,
    {
        let inner = Box::pin(parse_sse_stream(stream));
        Self {
            inner,
            buffer: String::new(),
            done: false,
        }
    }

    /// Collect all content from the stream.
    pub async fn collect_content(mut self) -> Result<String> {
        use futures::StreamExt;

        let mut content = String::new();
        while let Some(chunk) = self.next().await {
            match chunk {
                Ok(chunk) => content.push_str(chunk.content()),
                Err(e) => return Err(e),
            }
        }
        Ok(content)
    }

    /// Get the accumulated buffer content.
    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    /// Check if the stream is done.
    pub fn is_done(&self) -> bool {
        self.done
    }
}

impl Stream for ChatStream {
    type Item = Result<StreamChunk>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();

        if *this.done {
            return Poll::Ready(None);
        }

        match this.inner.poll_next(cx) {
            Poll::Ready(Some(Ok(chunk))) => {
                // Accumulate content in buffer
                this.buffer.push_str(chunk.content());

                if chunk.is_final() {
                    *this.done = true;
                }

                Poll::Ready(Some(Ok(chunk)))
            }
            Poll::Ready(Some(Err(e))) => {
                *this.done = true;
                Poll::Ready(Some(Err(e)))
            }
            Poll::Ready(None) => {
                *this.done = true;
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Parse an SSE stream into chunks.
fn parse_sse_stream<S>(stream: S) -> impl Stream<Item = Result<StreamChunk>>
where
    S: Stream<Item = std::result::Result<Bytes, reqwest::Error>> + Send,
{
    async_stream::stream! {
        use futures::StreamExt;

        let mut stream = std::pin::pin!(stream);
        let mut buffer = String::new();

        while let Some(result) = stream.next().await {
            let bytes = match result {
                Ok(bytes) => bytes,
                Err(e) => {
                    yield Err(Error::Http(e));
                    continue;
                }
            };

            let text = match std::str::from_utf8(&bytes) {
                Ok(text) => text,
                Err(e) => {
                    yield Err(Error::parse_error(format!("Invalid UTF-8: {}", e)));
                    continue;
                }
            };

            buffer.push_str(text);

            // Process complete SSE events
            while let Some(event_end) = buffer.find("\n\n") {
                let event = buffer[..event_end].to_string();
                buffer = buffer[event_end + 2..].to_string();

                // Parse SSE event
                for line in event.lines() {
                    if let Some(data) = line.strip_prefix("data: ") {
                        // Check for [DONE] marker
                        if data.trim() == "[DONE]" {
                            return;
                        }

                        // Parse JSON chunk
                        match serde_json::from_str::<StreamChunk>(data) {
                            Ok(chunk) => yield Ok(chunk),
                            Err(e) => {
                                // Log but don't fail on parse errors for individual chunks
                                tracing::debug!("Failed to parse chunk: {} - data: {}", e, data);
                            }
                        }
                    }
                }
            }
        }

        // Process any remaining data in buffer
        if !buffer.is_empty() {
            for line in buffer.lines() {
                if let Some(data) = line.strip_prefix("data: ") {
                    if data.trim() != "[DONE]" {
                        if let Ok(chunk) = serde_json::from_str::<StreamChunk>(data) {
                            yield Ok(chunk);
                        }
                    }
                }
            }
        }
    }
}

/// Collected result from a streaming response.
#[derive(Debug, Clone)]
pub struct StreamResult {
    /// Full accumulated content.
    pub content: String,
    /// Model used.
    pub model: String,
    /// Finish reason.
    pub finish_reason: Option<String>,
    /// Token usage (if available).
    pub usage: Option<crate::response::Usage>,
    /// Number of chunks received.
    pub chunk_count: usize,
}

impl StreamResult {
    /// Create a new stream result.
    pub fn new() -> Self {
        Self {
            content: String::new(),
            model: String::new(),
            finish_reason: None,
            usage: None,
            chunk_count: 0,
        }
    }

    /// Add a chunk to the result.
    pub fn add_chunk(&mut self, chunk: &StreamChunk) {
        self.content.push_str(chunk.content());
        self.chunk_count += 1;

        if self.model.is_empty() {
            self.model.clone_from(&chunk.model);
        }

        if let Some(reason) = chunk.finish_reason() {
            self.finish_reason = Some(reason.to_string());
        }

        if let Some(usage) = &chunk.usage {
            self.usage = Some(usage.clone());
        }
    }
}

impl Default for StreamResult {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_chunk_content() {
        let chunk = StreamChunk {
            id: "test".to_string(),
            object: "chat.completion.chunk".to_string(),
            created: 1234567890,
            model: "gpt-4o".to_string(),
            choices: vec![StreamChoice {
                index: 0,
                delta: StreamDelta {
                    role: None,
                    content: Some("Hello".to_string()),
                    tool_calls: None,
                },
                finish_reason: None,
                logprobs: None,
            }],
            system_fingerprint: None,
            usage: None,
        };

        assert_eq!(chunk.content(), "Hello");
        assert!(!chunk.is_final());
        assert!(chunk.has_content());
    }

    #[test]
    fn test_stream_chunk_final() {
        let chunk = StreamChunk {
            id: "test".to_string(),
            object: "chat.completion.chunk".to_string(),
            created: 1234567890,
            model: "gpt-4o".to_string(),
            choices: vec![StreamChoice {
                index: 0,
                delta: StreamDelta::default(),
                finish_reason: Some("stop".to_string()),
                logprobs: None,
            }],
            system_fingerprint: None,
            usage: None,
        };

        assert!(chunk.is_final());
        assert_eq!(chunk.finish_reason(), Some("stop"));
    }

    #[test]
    fn test_stream_result() {
        let mut result = StreamResult::new();

        let chunk1 = StreamChunk {
            id: "test".to_string(),
            object: "chat.completion.chunk".to_string(),
            created: 1234567890,
            model: "gpt-4o".to_string(),
            choices: vec![StreamChoice {
                index: 0,
                delta: StreamDelta {
                    role: Some("assistant".to_string()),
                    content: Some("Hello".to_string()),
                    tool_calls: None,
                },
                finish_reason: None,
                logprobs: None,
            }],
            system_fingerprint: None,
            usage: None,
        };

        let chunk2 = StreamChunk {
            id: "test".to_string(),
            object: "chat.completion.chunk".to_string(),
            created: 1234567890,
            model: "gpt-4o".to_string(),
            choices: vec![StreamChoice {
                index: 0,
                delta: StreamDelta {
                    role: None,
                    content: Some(", world!".to_string()),
                    tool_calls: None,
                },
                finish_reason: Some("stop".to_string()),
                logprobs: None,
            }],
            system_fingerprint: None,
            usage: None,
        };

        result.add_chunk(&chunk1);
        result.add_chunk(&chunk2);

        assert_eq!(result.content, "Hello, world!");
        assert_eq!(result.model, "gpt-4o");
        assert_eq!(result.finish_reason, Some("stop".to_string()));
        assert_eq!(result.chunk_count, 2);
    }

    #[test]
    fn test_chunk_deserialization() {
        let json = r#"{
            "id": "chatcmpl-123",
            "object": "chat.completion.chunk",
            "created": 1677652288,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "delta": {
                    "content": "Hello"
                },
                "finish_reason": null
            }]
        }"#;

        let chunk: StreamChunk = serde_json::from_str(json).unwrap();
        assert_eq!(chunk.content(), "Hello");
        assert!(!chunk.is_final());
    }
}
