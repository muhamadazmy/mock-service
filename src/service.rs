use std::{collections::HashMap, sync::Arc};

use bytes::Bytes;
use restate_sdk::{
    discovery::{self, Handler, HandlerName, ServiceName, ServiceType},
    errors::HandlerError,
    prelude::{Context, ContextWriteState, Endpoint, ObjectContext, WorkflowContext},
    serde::{Deserialize, Serialize},
    service::{Discoverable, Service, ServiceBoxFuture},
};

tokio::task_local! {
    static DISCOVERY_METADATA: discovery::Service;
}

#[derive(Clone)]
pub struct JsonValue(pub serde_json::Value);

impl Deserialize for JsonValue {
    type Error = serde_json::Error;

    fn deserialize(bytes: &mut Bytes) -> Result<Self, Self::Error> {
        Ok(JsonValue(serde_json::from_slice(&bytes)?))
    }
}

impl Serialize for JsonValue {
    type Error = serde_json::Error;

    fn serialize(&self) -> Result<Bytes, Self::Error> {
        Ok(Bytes::from(serde_json::to_string(&self.0)?))
    }
}

pub struct MockService {
    name: ServiceName,
    ty: ServiceType,
    handlers: HashMap<String, MockHandler>,
}

impl MockService {
    pub fn new(name: ServiceName, ty: ServiceType) -> Self {
        Self {
            name,
            ty,
            handlers: HashMap::new(),
        }
    }

    pub fn add_handler(&mut self, handler: MockHandler) {
        self.handlers.insert(handler.name.clone().into(), handler);
    }

    fn service_discovery(&self) -> discovery::Service {
        discovery::Service {
            name: self.name.clone(),
            handlers: self
                .handlers
                .iter()
                .map(|(name, _)| Handler {
                    name: name.clone().try_into().unwrap(),
                    input: None,
                    output: None,
                    ty: None,
                })
                .collect(),
            ty: self.ty,
        }
    }

    pub async fn endpoint(self) -> Endpoint {
        let discovery = self.service_discovery();

        let wrapper = MockServiceWrapper {
            inner: Arc::new(self),
        };

        DISCOVERY_METADATA
            .scope(discovery, async move {
                Endpoint::builder().bind(wrapper).build()
            })
            .await
    }
}

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

            let (input, metadata) = ctx.input::<JsonValue>().await;

            let res = handler
                .run((&ctx, metadata).into(), &input)
                .await
                .map_err(::restate_sdk::errors::HandlerError::from);

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

#[derive(Default)]
pub struct ExecutionContext {
    ret: Option<JsonValue>,
}

impl ExecutionContext {
    pub fn return_value(&mut self, value: impl Into<JsonValue>) {
        if self.ret.is_some() {
            panic!("return_value can only be called once");
        }

        self.ret = Some(value.into());
    }
}

#[async_trait::async_trait]
pub trait Step: Send + Sync + 'static {
    async fn run(
        &self,
        ctx: &ObjectContext<'_>,
        step: &mut ExecutionContext,
        input: &JsonValue,
    ) -> Result<(), HandlerError>;
}

pub type BoxStep = Box<dyn Step>;

impl<T> From<T> for BoxStep
where
    T: Step,
{
    fn from(value: T) -> Self {
        Box::new(value)
    }
}

pub struct MockHandler {
    pub name: HandlerName,
    pub steps: Vec<BoxStep>,
}

pub struct EchoStep;

#[async_trait::async_trait]
impl Step for EchoStep {
    async fn run(
        &self,
        ctx: &ObjectContext<'_>,
        exec: &mut ExecutionContext,
        input: &JsonValue,
    ) -> Result<(), HandlerError> {
        ctx.set("test", 100);

        exec.return_value(input.clone());
        Ok(())
    }
}

impl MockHandler {
    async fn run(
        &self,
        ctx: ObjectContext<'_>,
        input: &JsonValue,
    ) -> Result<JsonValue, HandlerError> {
        let mut exec_ctx = ExecutionContext::default();
        for step in self.steps.iter() {
            step.run(&ctx, &mut exec_ctx, input).await?;
        }

        Ok(exec_ctx.ret.unwrap_or(JsonValue(serde_json::Value::Null)))
    }
}
