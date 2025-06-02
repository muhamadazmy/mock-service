use std::{collections::HashMap, sync::Arc};

use bytes::Bytes;
use context::ExecutionContext;
use restate_sdk::{
    discovery::{self, Handler, HandlerName, HandlerType, ServiceName, ServiceType},
    endpoint::Builder,
    errors::HandlerError,
    prelude::WorkflowContext,
    serde::{Deserialize, Serialize},
    service::{Discoverable, Service, ServiceBoxFuture},
};
pub use steps::STEPS;
use tracing::debug;

mod context;
mod steps;

tokio::task_local! {
    static DISCOVERY_METADATA: discovery::Service;
}

/// A wrapper around `serde_json::Value` to facilitate its use with Restate SDK's serialization.
#[derive(Clone)]
pub struct JsonValue(pub serde_json::Value);

impl Deserialize for JsonValue {
    type Error = serde_json::Error;

    fn deserialize(bytes: &mut Bytes) -> Result<Self, Self::Error> {
        Ok(JsonValue(serde_json::from_slice(bytes)?))
    }
}

impl Serialize for JsonValue {
    type Error = serde_json::Error;

    fn serialize(&self) -> Result<Bytes, Self::Error> {
        Ok(Bytes::from(serde_json::to_string(&self.0)?))
    }
}

impl From<serde_json::Value> for JsonValue {
    fn from(value: serde_json::Value) -> Self {
        JsonValue(value)
    }
}

/// Represents a configurable mock service that can handle requests based on predefined steps.
///
/// A `MockService` is defined by its name, type (e.g., `SERVICE` or `VIRTUAL_OBJECT`), and a collection of handlers.
/// Each handler, in turn, consists of a sequence of steps that dictate its behavior when invoked.
pub struct MockService {
    name: ServiceName,
    ty: ServiceType,
    handlers: HashMap<String, MockHandler>,
}

impl MockService {
    /// Creates a new `MockService` with the given name and type.
    pub fn new(name: ServiceName, ty: ServiceType) -> Self {
        Self {
            name,
            ty,
            handlers: HashMap::new(),
        }
    }

    /// Adds a handler to the service.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the handler.
    /// * `handler` - The `MockHandler` definition.
    pub fn add_handler(&mut self, name: HandlerName, handler: MockHandler) {
        self.handlers.insert(name.to_string(), handler);
    }

    /// Generates the service discovery information for this mock service.
    fn service_discovery(&self) -> discovery::Service {
        discovery::Service {
            name: self.name.clone(),
            handlers: self
                .handlers
                .iter()
                .map(|(name, handler)| Handler {
                    name: name.clone().try_into().unwrap(),
                    input: None,
                    output: None,
                    ty: handler.ty,
                })
                .collect(),
            ty: self.ty,
        }
    }

    /// Binds this mock service to the Restate endpoint builder.
    ///
    /// This method sets up the service with Restate, making its handlers discoverable and callable.
    pub async fn bind(self, endpoint: Builder) -> Builder {
        let discovery = self.service_discovery();

        let wrapper = MockServiceWrapper {
            inner: Arc::new(self),
        };

        DISCOVERY_METADATA
            .scope(discovery, async move { endpoint.bind(wrapper) })
            .await
    }
}

/// A wrapper around `MockService` to make it compatible with the Restate `Service` trait.
/// This is used internally for integrating with the Restate SDK.
#[derive(Clone)]
struct MockServiceWrapper {
    inner: Arc<MockService>,
}

impl Service for MockServiceWrapper {
    type Future = ServiceBoxFuture;

    fn handle(&self, ctx: restate_sdk::endpoint::ContextInternal) -> Self::Future {
        let service_clone = self.clone();
        Box::pin(async move {
            let Some(handler) = service_clone.inner.handlers.get(ctx.handler_name()) else {
                return Err(::restate_sdk::endpoint::Error::unknown_handler(
                    ctx.service_name(),
                    ctx.handler_name(),
                ));
            };

            debug!(
                "Running handler {}/{}",
                ctx.service_name(),
                ctx.handler_name()
            );

            let (input, metadata) = ctx.input::<JsonValue>().await;

            let res = handler.run((&ctx, metadata).into(), &input).await;

            ctx.handle_handler_result(res);
            ctx.end();
            Ok(())
        })
    }
}

impl Discoverable for MockServiceWrapper {
    fn discover() -> discovery::Service {
        DISCOVERY_METADATA.get()
    }
}

/// Defines errors that can occur during step creation or validation.
#[derive(Debug, thiserror::Error)]
pub enum StepError {
    /// Error indicating that a step is not valid for the given service type.
    #[error("Invalid service type: {}", .0.to_string())]
    InvalidServiceType(ServiceType),
    /// Error indicating invalid parameters were provided for a step.
    #[error("Invalid step parameters: {0}")]
    InvalidStepParameters(#[from] serde_yaml::Error),
}

/// Trait defining the contract for a step in a mock handler's execution flow.
///
/// Each step must be able to validate itself against a service type and execute its logic.
#[async_trait::async_trait]
pub trait Step: Send + Sync + 'static {
    /// Validates if the step is appropriate for the given `ServiceType`.
    ///
    /// # Arguments
    ///
    /// * `service_type` - The type of the service this step belongs to.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the step is valid, otherwise a `StepError`.
    fn validate(&self, service_type: ServiceType) -> Result<(), StepError>;

    /// Executes the step's logic.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The `WorkflowContext` providing access to Restate features like state and timers.
    /// * `exec_ctx` - The `ExecutionContext` for the current handler, allowing variables to be set and retrieved.
    /// * `input` - The input value to the handler.
    ///
    /// # Returns
    ///
    /// `Ok(())` if execution is successful, otherwise a `HandlerError`.
    async fn run(
        &self,
        ctx: &WorkflowContext<'_>,
        step: &mut ExecutionContext,
        input: &JsonValue,
    ) -> Result<(), HandlerError>;
}

/// A type alias for a boxed `Step` trait object.
pub type BoxStep = Box<dyn Step>;

impl<T> From<T> for BoxStep
where
    T: Step,
{
    fn from(value: T) -> Self {
        Box::new(value)
    }
}

/// Represents a handler within a `MockService`.
///
/// A `MockHandler` contains a sequence of `Step`s that are executed in order when the handler is called.
/// It also optionally defines the `HandlerType` (e.g., `WORKFLOW`, `UNARY`).
pub struct MockHandler {
    /// The sequence of steps to be executed by this handler.
    pub steps: Vec<BoxStep>,
    /// The type of the handler (e.g., workflow, unary). If `None`, Restate's default is used.
    pub ty: Option<HandlerType>,
}

impl MockHandler {
    /// Runs the sequence of steps defined for this handler.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The `WorkflowContext` for the current invocation.
    /// * `input` - The input `JsonValue` passed to the handler.
    ///
    /// # Returns
    ///
    /// A `Result` containing the `JsonValue` returned by the handler's execution (often from a `ReturnStep`),
    /// or a `HandlerError` if any step fails.
    async fn run(
        &self,
        ctx: WorkflowContext<'_>,
        input: &JsonValue,
    ) -> Result<JsonValue, HandlerError> {
        let mut exec_ctx = ExecutionContext::default();
        for step in self.steps.iter() {
            step.run(&ctx, &mut exec_ctx, input).await?;
        }

        Ok(exec_ctx.ret().unwrap_or(JsonValue(serde_json::Value::Null)))
    }
}

/// Trait for a factory that can create instances of a specific `Step`.
///
/// Each step type (e.g., `Echo`, `Sleep`) will have an associated factory.
pub trait StepFactory: Send + Sync + 'static {
    /// Creates a new `BoxStep` instance from YAML configuration parameters.
    ///
    /// # Arguments
    ///
    /// * `params` - The YAML value containing the parameters for configuring the step.
    ///
    /// # Returns
    ///
    /// A `Result` containing the created `BoxStep` or a `StepError` if creation fails
    /// (e.g., due to invalid parameters).
    fn create(&self, params: serde_yaml::Value) -> Result<BoxStep, StepError>;
}
