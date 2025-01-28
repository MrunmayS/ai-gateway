# LangDB AI Gateway

A Rust-based gateway service for interacting with various LLM providers (OpenAI, Anthropic, etc.) with unified API interface.

## Features

- OpenAI-compatible API endpoints
- Model configuration via YAML
- Support for multiple LLM providers
- Debug-level event logging
- OpenTelemetry integration
- Cost tracking and usage monitoring

## Setup

1. Clone the repository:
```bash
git clone https://github.com/langdb/ai-gateway.git
cd ai-gateway
```

1. Copy file `config.yaml`:
```yaml
rest:
  host: "127.0.0.1"
  port: 8080
  cors_allowed_origins:
    - "http://localhost:3000"
    - "http://127.0.0.1:3000"

models:
  - model: "gpt-4"
    model_provider: "openai"
    inference_provider:
      provider: "openai"
      model_name: "gpt-4"
    price:
      input_price: 0.00003
      output_price: 0.00006
    input_formats:
      - text
    output_formats:
      - text
    capabilities:
      - tools
    type: completions
    limits:
      max_context_size: 8000
    description: "GPT-4 is a large language model that can understand and generate human-like text"
```

3. Set up environment variables:
```bash
# OpenAI API Key
export LANGDB_OPENAI_API_KEY=your-api-key-here

# Optional: Set log level (default: info)
export RUST_LOG=debug
```

4. Build and run:
```bash
cargo run
```

## API Endpoints

The gateway provides the following OpenAI-compatible endpoints:

- `POST /v1/chat/completions` - Chat completions
- `GET /v1/models` - List available models
- `POST /v1/embeddings` - Generate embeddings
- `POST /v1/images/generations` - Generate images

## Example Usage

1. Run the server with your OpenAI API key:
```bash
LANGDB_OPENAI_API_KEY=your-api-key cargo run
```

2. Make a chat completion request:
```bash
curl http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4o-mini",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'
```

## Development

### Project Structure

- `gateway/` - Core gateway implementation
  - Models and provider integrations
  - API types and handlers
  - OpenTelemetry integration
- `server/` - HTTP server implementation
  - Configuration management
  - REST API endpoints
  - Cost tracking

### Running Tests

```bash
cargo test
```

### Logging

The gateway uses `tracing` for logging. Set the `RUST_LOG` environment variable to control log levels:

```bash
RUST_LOG=debug cargo run    # For detailed logs
RUST_LOG=info cargo run     # For standard logs
```

## License

