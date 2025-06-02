use std::str::FromStr;

use bytes::Bytes;
use restate_sdk::{
    discovery::{self, Handler, HandlerName, HandlerType, ServiceName, ServiceType},
    prelude::*,
    service::{Discoverable, Service, ServiceBoxFuture},
};
use serde::{Deserialize, Serialize};
use service::{EchoStep, MockHandler, MockService};

mod config;
mod service;
#[restate_sdk::object]
trait Greet {
    async fn greet(name: String) -> Result<String, HandlerError>;
}

struct GreetImpl;

impl Greet for GreetImpl {
    async fn greet(&self, ctx: ObjectContext<'_>, name: String) -> Result<String, HandlerError> {
        Ok(format!("Hello, {}!", name))
    }
}

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let mut service = MockService::new("mock-service".try_into().unwrap(), ServiceType::Service);
    service.add_handler(MockHandler {
        name: "echo".try_into().unwrap(),
        steps: vec![EchoStep.into()],
    });

    //todo:
    // - Check if using the WofklowContext makes more sense since it
    // has all required functionality.
    // - Steps can verify if they are valid in with certain service type.
    // - Step verification happens during creation of the endpoint (or earlier)
    // - Build complex services via yaml ? or other means ?

    // Create and start the HTTP server
    HttpServer::new(service.endpoint().await)
        .listen_and_serve("0.0.0.0:9200".parse().unwrap())
        .await;
}
