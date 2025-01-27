use std::collections::HashMap;

use crate::embed_mod::Embed;
use crate::embed_mod::OpenAIEmbed;
use crate::error::GatewayError;
use crate::model::types::ModelEvent;
use crate::models::LlmModelDefinition;
use crate::types::credentials::Credentials;
use async_openai::types::EmbeddingInput;
use tracing::Span;

use crate::types::embed::OpenAiEmbeddingParams;
use crate::types::{
    engine::{ExecutionOptions, InputArgs, Model, ModelTools, ModelType},
    gateway::{CreateEmbeddingRequest, Input},
};
use tracing_futures::Instrument;

use crate::handler::{CallbackHandlerFn, ModelEventWithDetails};

pub async fn handle_embeddings_invoke(
    mut request: CreateEmbeddingRequest,
    callback_handler: &CallbackHandlerFn,
    llm_model: &LlmModelDefinition,
    key_credentials: Option<&Credentials>,
) -> Result<async_openai::types::CreateEmbeddingResponse, GatewayError> {
    let span = Span::current();
    request.model = llm_model.inference_provider.model_name.clone();

    let params = OpenAiEmbeddingParams {
        model: Some(llm_model.model.clone()),
        dimensions: request.dimensions,
    };

    let input: EmbeddingInput = match &request.input {
        Input::String(s) => s.into(),
        Input::Array(vec) => vec.into(),
    };

    let (tx, mut rx) = tokio::sync::mpsc::channel::<Option<ModelEvent>>(1000);
    let model_name = llm_model.model.clone();

    let callback_handler = callback_handler.clone();
    tokio::spawn(async move {
        while let Some(Some(msg)) = rx.recv().await {
            callback_handler.on_message(ModelEventWithDetails::new(
                msg,
                Model {
                    name: model_name.clone(),
                    description: None,
                    provider_name: "openai".to_string(),
                    prompt_name: None,
                    model_params: HashMap::new(),
                    execution_options: ExecutionOptions::default(),
                    input_args: InputArgs(vec![]),
                    tools: ModelTools(vec![]),
                    model_type: ModelType::Embedding,
                    response_schema: None,
                    credentials: None,
                },
            ));
        }
    });

    let api_key_credentials = match key_credentials {
        Some(Credentials::ApiKey(api_key)) => Some(api_key),
        _ => None,
    };

    let embed = OpenAIEmbed::new(params, api_key_credentials)?;
    embed
        .invoke(input, Some(tx.clone()))
        .instrument(span.clone())
        .await
}
