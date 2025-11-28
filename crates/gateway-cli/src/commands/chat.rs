//! Chat command - send chat completion requests.

use anyhow::Result;
use clap::Args;
use futures::StreamExt;
use serde::Serialize;
use std::io::{self, BufRead, Write};

use crate::output::{self, CommandResult, OutputFormat};

/// Arguments for the chat command.
#[derive(Args, Debug)]
pub struct ChatArgs {
    /// Message to send (if not provided, reads from stdin)
    #[arg(short, long)]
    pub message: Option<String>,

    /// Model to use
    #[arg(short = 'M', long, default_value = "gpt-4o-mini")]
    pub model: String,

    /// System prompt
    #[arg(short, long)]
    pub system: Option<String>,

    /// Enable streaming output
    #[arg(long)]
    pub stream: bool,

    /// Temperature (0.0 to 2.0)
    #[arg(short, long)]
    pub temperature: Option<f32>,

    /// Maximum tokens to generate
    #[arg(long)]
    pub max_tokens: Option<u32>,

    /// Top-p sampling parameter
    #[arg(long)]
    pub top_p: Option<f32>,

    /// User identifier for tracking
    #[arg(long)]
    pub user: Option<String>,

    /// Seed for deterministic outputs
    #[arg(long)]
    pub seed: Option<i64>,

    /// Interactive chat mode
    #[arg(short, long)]
    pub interactive: bool,

    /// Show token usage
    #[arg(long)]
    pub show_usage: bool,
}

/// Chat response for output.
#[derive(Debug, Serialize)]
pub struct ChatOutput {
    pub model: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<UsageOutput>,
}

/// Token usage output.
#[derive(Debug, Serialize)]
pub struct UsageOutput {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Execute the chat command.
pub async fn execute(
    args: ChatArgs,
    base_url: &str,
    api_key: Option<&str>,
    json: bool,
) -> Result<()> {
    let format = OutputFormat::from_json_flag(json);

    // Build client
    let client = build_client(base_url, api_key)?;

    if args.interactive {
        run_interactive_mode(&client, &args, format).await
    } else {
        run_single_message(&client, &args, format).await
    }
}

/// Build the SDK client.
fn build_client(base_url: &str, api_key: Option<&str>) -> Result<gateway_sdk::Client> {
    let builder = gateway_sdk::Client::builder()
        .base_url(base_url)
        .timeout(std::time::Duration::from_secs(120));

    let builder = if let Some(key) = api_key {
        builder.api_key(key)
    } else {
        builder
    };

    Ok(builder.build()?)
}

/// Run a single message chat.
async fn run_single_message(
    client: &gateway_sdk::Client,
    args: &ChatArgs,
    format: OutputFormat,
) -> Result<()> {
    // Get the message
    let message = if let Some(ref msg) = args.message {
        msg.clone()
    } else {
        // Read from stdin
        let mut input = String::new();
        io::stdin().lock().read_line(&mut input)?;
        input.trim().to_string()
    };

    if message.is_empty() {
        let result: CommandResult<()> = CommandResult::failure("No message provided");
        result.print(format)?;
        return Ok(());
    }

    // Build request
    let mut chat = client.chat().model(&args.model).user_message(&message);

    if let Some(ref system) = args.system {
        chat = chat.system_message(system);
    }

    if let Some(temp) = args.temperature {
        chat = chat.temperature(temp);
    }

    if let Some(max) = args.max_tokens {
        chat = chat.max_tokens(max);
    }

    if let Some(top_p) = args.top_p {
        chat = chat.top_p(top_p);
    }

    if let Some(ref user) = args.user {
        chat = chat.user(user);
    }

    if let Some(seed) = args.seed {
        chat = chat.seed(seed);
    }

    if args.stream && !matches!(format, OutputFormat::Json) {
        // Streaming output
        let mut stream = chat.stream().await?;

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(chunk) => {
                    output::stream_text(chunk.content());
                }
                Err(e) => {
                    output::stream_newline();
                    output::error(&format!("Stream error: {}", e));
                    break;
                }
            }
        }
        output::stream_newline();
    } else {
        // Non-streaming output
        if !matches!(format, OutputFormat::Json) {
            let spinner = output::spinner("Generating response...");
            let result = chat.send().await;
            spinner.finish_and_clear();

            match result {
                Ok(response) => {
                    println!("{}", response.content());

                    if args.show_usage {
                        if let Some(usage) = &response.usage {
                            output::section("Token Usage");
                            output::key_value("Prompt", &usage.prompt_tokens.to_string());
                            output::key_value("Completion", &usage.completion_tokens.to_string());
                            output::key_value("Total", &usage.total_tokens.to_string());
                        }
                    }
                }
                Err(e) => {
                    output::error(&format!("Request failed: {}", e));
                }
            }
        } else {
            let result = chat.send().await;

            match result {
                Ok(response) => {
                    let chat_output = ChatOutput {
                        model: response.model.clone(),
                        content: response.content().to_string(),
                        finish_reason: response.finish_reason().map(String::from),
                        usage: response.usage.as_ref().map(|u| UsageOutput {
                            prompt_tokens: u.prompt_tokens,
                            completion_tokens: u.completion_tokens,
                            total_tokens: u.total_tokens,
                        }),
                    };
                    let result = CommandResult::success(chat_output);
                    result.print(format)?;
                }
                Err(e) => {
                    let result: CommandResult<ChatOutput> =
                        CommandResult::failure(format!("{}", e));
                    result.print(format)?;
                }
            }
        }
    }

    Ok(())
}

/// Run interactive chat mode.
async fn run_interactive_mode(
    client: &gateway_sdk::Client,
    args: &ChatArgs,
    _format: OutputFormat,
) -> Result<()> {
    output::info(&format!("Interactive chat with {} (type 'exit' to quit)", args.model));

    if let Some(ref system) = args.system {
        output::info(&format!("System: {}", system));
    }

    println!();

    let mut messages: Vec<gateway_sdk::Message> = Vec::new();

    // Add system message if provided
    if let Some(ref system) = args.system {
        messages.push(gateway_sdk::Message::system(system));
    }

    loop {
        // Prompt
        print!("{} ", "You:".to_string());
        io::stdout().flush()?;

        // Read input
        let mut input = String::new();
        io::stdin().lock().read_line(&mut input)?;
        let input = input.trim();

        // Check for exit
        if input.eq_ignore_ascii_case("exit") || input.eq_ignore_ascii_case("quit") {
            output::info("Goodbye!");
            break;
        }

        if input.is_empty() {
            continue;
        }

        // Add user message
        messages.push(gateway_sdk::Message::user(input));

        // Build request with full conversation history
        let mut chat = client.chat().model(&args.model);

        for msg in &messages {
            chat = chat.message(msg.clone());
        }

        if let Some(temp) = args.temperature {
            chat = chat.temperature(temp);
        }

        if let Some(max) = args.max_tokens {
            chat = chat.max_tokens(max);
        }

        // Get response
        if args.stream {
            print!("{} ", "Assistant:".to_string());
            io::stdout().flush()?;

            let mut stream = match chat.stream().await {
                Ok(s) => s,
                Err(e) => {
                    output::error(&format!("Error: {}", e));
                    messages.pop(); // Remove the user message
                    continue;
                }
            };

            let mut response_content = String::new();
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(chunk) => {
                        let content = chunk.content();
                        response_content.push_str(content);
                        print!("{}", content);
                        io::stdout().flush()?;
                    }
                    Err(e) => {
                        output::error(&format!("Stream error: {}", e));
                        break;
                    }
                }
            }
            println!();

            // Add assistant response to history
            if !response_content.is_empty() {
                messages.push(gateway_sdk::Message::assistant(&response_content));
            }
        } else {
            let spinner = output::spinner("Thinking...");
            let result = chat.send().await;
            spinner.finish_and_clear();

            match result {
                Ok(response) => {
                    println!("{} {}", "Assistant:".to_string(), response.content());

                    // Add assistant response to history
                    messages.push(gateway_sdk::Message::assistant(response.content()));

                    if args.show_usage {
                        if let Some(usage) = &response.usage {
                            output::info(&format!(
                                "Tokens: {} prompt, {} completion, {} total",
                                usage.prompt_tokens,
                                usage.completion_tokens,
                                usage.total_tokens
                            ));
                        }
                    }
                }
                Err(e) => {
                    output::error(&format!("Error: {}", e));
                    messages.pop(); // Remove the user message
                }
            }
        }

        println!();
    }

    Ok(())
}
