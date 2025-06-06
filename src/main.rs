use anyhow::Context;
use config::{Configuration, StepConfig};
use mock::{MockHandler, MockService, STEPS};
use restate_sdk::{
    discovery::{HandlerName, ServiceName, ServiceType},
    prelude::*,
};

mod config;
mod mock;
use clap::Parser;
use restate_sdk::endpoint::Endpoint;
use std::{fs::File, io::BufReader, path::PathBuf, str::FromStr};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

use crate::mock::{Step, StepError};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None, trailing_var_arg = true)]
struct Args {
    #[clap(short, long, value_parser)]
    config_file: PathBuf,
    #[clap(short, long, value_parser, default_value = "0.0.0.0:9200")]
    listen_address: String,
    #[clap(long, value_parser, default_value = "info")]
    log_level: String,
}

fn step_from_config(
    service_type: ServiceType,
    step_config: StepConfig,
) -> Result<Box<dyn Step>, StepError> {
    let factory = STEPS
        .get(step_config.ty.as_str())
        .ok_or_else(|| StepError::UnknownStepType(step_config.ty.clone()))?;

    let step = factory.create(step_config.params)?;

    step.validate(service_type)?;

    Ok(step)
}

#[restate_sdk::object]
trait TestVO {
    async fn work() -> HandlerResult<()>;
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(&args.log_level))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let subscriber = FmtSubscriber::builder().with_env_filter(filter).finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    tracing::debug!("Loading configuration from: {:?}", args.config_file);
    let file = File::open(&args.config_file)
        .with_context(|| format!("Failed to open config file {}", args.config_file.display()))?;

    let reader = BufReader::new(file);
    let config: Configuration =
        serde_yaml::from_reader(reader).context("Failed to parse config")?;

    let mut endpoint_builder = Endpoint::builder();

    for (service, service_config) in config.services {
        tracing::debug!(
            "Setting up service '{service}' of type {:?}",
            service_config.ty
        );

        let service_name = ServiceName::from_str(&service)
            .with_context(|| format!("Invalid service name {service}"))?;

        let mut mock_service = MockService::new(service_name, service_config.ty);

        for (handler_name, handler_config) in service_config.handlers {
            tracing::info!("Adding handler '{handler_name}'to service '{service}'");

            let mut steps: Vec<Box<dyn Step>> = Vec::new();
            for (idx, step_cfg) in handler_config.steps.into_iter().enumerate() {
                // The `?` operator will convert restate_sdk::Error into Box<dyn std::error::Error>
                steps.push(
                    step_from_config(service_config.ty, step_cfg).with_context(|| {
                        format!("Failed to create step {idx} for handler {handler_name}")
                    })?,
                );
            }

            let handler_name = HandlerName::from_str(&handler_name)
                .with_context(|| format!("Invalid handler name {}", handler_name))?;

            mock_service.add_handler(
                handler_name,
                MockHandler {
                    steps,
                    ty: handler_config.ty,
                },
            );
        }

        endpoint_builder = mock_service.bind(endpoint_builder).await;
    }

    let endpoint = endpoint_builder.build();

    tracing::info!("Starting server on {}", args.listen_address);
    HttpServer::new(endpoint)
        .listen_and_serve(args.listen_address.parse()?)
        .await;

    Ok(())
}
