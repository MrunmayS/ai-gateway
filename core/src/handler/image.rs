use crate::executor::image_generation::handle_image_generation;
use crate::handler::record_map_err;
use crate::handler::AvailableModels;
use crate::handler::CallbackHandlerFn;
use crate::types::gateway::CreateImageRequest;
use crate::types::{credentials::Credentials, gateway::CostCalculator};
use crate::GatewayApiError;
use actix_web::HttpMessage;
use actix_web::{web, HttpRequest, HttpResponse};
use tracing::Span;
use tracing_futures::Instrument;

use super::extract_tags;
use super::find_model_by_full_name;

pub async fn create_image(
    request: web::Json<CreateImageRequest>,
    models: web::Data<AvailableModels>,
    req: HttpRequest,
    cost_calculator: web::Data<Box<dyn CostCalculator>>,
    callback_handler: web::Data<CallbackHandlerFn>,
) -> Result<HttpResponse, GatewayApiError> {
    let request = request.into_inner();
    let available_models = models.into_inner();
    let llm_model = find_model_by_full_name(&request.model, &available_models)?;

    let span = Span::current();

    let tags = extract_tags(&req)?;

    let key = req.extensions().get::<Credentials>().cloned();
    let result = handle_image_generation(
        request,
        callback_handler.get_ref(),
        &llm_model,
        key.as_ref(),
        cost_calculator.into_inner(),
        tags,
    )
    .instrument(span.clone())
    .await
    .map_err(|e| record_map_err(e, span.clone()))?;

    Ok(HttpResponse::Ok().json(result))
}
