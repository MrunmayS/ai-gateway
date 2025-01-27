pub mod basic_executor;
pub mod stream_executor;

use std::collections::HashMap;
use std::sync::Arc;

use crate::llm_gateway::message_mapper::MessageMapper;
use crate::llm_gateway::provider::Provider;
use crate::model::tools::{GatewayTool, Tool};
use crate::model::types::ModelEvent;
use crate::types::gateway::CompletionModelUsage;
use actix_web::{HttpMessage, HttpRequest};
use either::Either::{self, Left, Right};
use futures::Stream;

use crate::{
    model::types::ModelEventType,
    types::{
        credentials::Credentials,
        engine::{
            CompletionModelDefinition, CompletionModelParams, ExecutionOptions, InputArgs, Model,
            ModelTool, ModelTools, ModelType, Prompt,
        },
        gateway::{
            ChatCompletionDelta, ChatCompletionRequest, ChatCompletionResponse, CostCalculator,
        },
    },
};
use tracing::Span;
use tracing_futures::Instrument;

use crate::executor::chat_completion::stream_executor::stream_chunks;
use crate::handler::extract_tags;
use crate::handler::find_model_by_full_name;
use crate::handler::AvailableModels;
use crate::handler::{CallbackHandlerFn, ModelEventWithDetails};
use crate::GatewayApiError;

pub async fn execute(
    mut request: ChatCompletionRequest,
    callback_handler: &CallbackHandlerFn,
    req: HttpRequest,
    provided_models: &AvailableModels,
    cost_calculator: Arc<Box<dyn CostCalculator>>,
) -> Result<
    Either<
        Result<
            impl Stream<
                Item = Result<
                    (Option<ChatCompletionDelta>, Option<CompletionModelUsage>),
                    GatewayApiError,
                >,
            >,
            GatewayApiError,
        >,
        Result<ChatCompletionResponse, GatewayApiError>,
    >,
    GatewayApiError,
> {
    let span = Span::current();
    let tags = extract_tags(&req)?;

    let llm_model = find_model_by_full_name(&request.model, provided_models)?;
    request.model = llm_model.inference_provider.model_name.clone();

    let user_id = uuid::Uuid::new_v4();

    let key_credentials = req.extensions().get::<Credentials>().cloned();

    let engine =
        Provider::get_completion_engine_for_model(&llm_model, &request, key_credentials.clone())?;

    let tools = ModelTools(request.tools.as_ref().map_or(vec![], |tools| {
        tools
            .iter()
            .map(|tool| ModelTool {
                name: tool.function.name.clone(),
                description: tool.function.description.clone(),
                passed_args: vec![],
            })
            .collect()
    }));

    let db_model = Model {
        name: request.model.clone(),
        description: Some("Generated model for chat completion".to_string()),
        provider_name: llm_model.inference_provider.provider.to_string(),
        prompt_name: None,
        model_params: HashMap::new(),
        execution_options: ExecutionOptions::default(),
        input_args: InputArgs(vec![]),
        tools: tools.clone(),
        model_type: ModelType::Completions,
        response_schema: None,
        credentials: key_credentials,
    };

    let completion_model_definition = CompletionModelDefinition {
        name: request.model.clone(),
        model_params: CompletionModelParams {
            engine: engine.clone(),
            provider_name: llm_model.model_provider.to_string(),
            prompt_name: None,
        },
        input_args: InputArgs(vec![]),
        prompt: Prompt::empty(),
        tools,
        db_model: db_model.clone(),
    };

    let tools_map: HashMap<String, Box<(dyn Tool + 'static)>> =
        request.tools.as_ref().map_or_else(HashMap::new, |tools| {
            tools
                .iter()
                .map(|tool| {
                    (
                        tool.function.name.clone(),
                        Box::new(GatewayTool { def: tool.clone() }) as Box<dyn Tool>,
                    )
                })
                .collect()
        });

    let model = crate::model::init_completion_model_instance(
        completion_model_definition.clone(),
        tools_map,
        Some(cost_calculator.clone()),
        llm_model.inference_provider.endpoint.as_deref(),
        Some(&llm_model.inference_provider.provider.to_string()),
    )
    .await
    .map_err(|e| GatewayApiError::CustomError(e.to_string()))?;

    let mut messages = vec![];

    for message in &request.messages {
        messages.push(MessageMapper::map_completions_message_to_langdb_message(
            message,
            &request.model,
            &user_id.to_string(),
        )?);
    }
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Option<ModelEvent>>(1000);

    let ch = callback_handler.clone();
    let handle = tokio::spawn(async move {
        let mut stop_event = None;
        let mut tool_calls = None;
        while let Some(Some(msg)) = rx.recv().await {
            if let ModelEvent {
                event: ModelEventType::LlmStop(e),
                ..
            } = &msg
            {
                stop_event = Some(e.clone());
            }

            if let ModelEvent {
                event: ModelEventType::ToolStart(e),
                ..
            } = &msg
            {
                if tool_calls.is_none() {
                    tool_calls = Some(vec![]);
                }
                tool_calls.as_mut().unwrap().push(e.clone());
            }

            if let ModelEvent {
                event: ModelEventType::LlmFirstToken(e),
                ..
            } = &msg
            {
                let current_span = Span::current();
                current_span.record("ttft", e.ttft);
            }

            ch.on_message(ModelEventWithDetails::new(msg, db_model.clone()));
        }

        (stop_event, tool_calls)
    });

    if request.stream.unwrap_or(false) {
        Ok(Left(
            stream_chunks(
                completion_model_definition,
                model,
                vec![],
                messages.clone(),
                callback_handler.clone().into(),
                tags.clone(),
            )
            .instrument(span)
            .await,
        ))
    } else {
        Ok(Right(
            basic_executor::execute(
                request,
                model,
                messages.clone(),
                tags.clone(),
                tx,
                span.clone(),
                handle,
            )
            .instrument(span)
            .await,
        ))
    }
}
