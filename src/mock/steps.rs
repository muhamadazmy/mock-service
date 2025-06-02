use std::{collections::HashMap, sync::LazyLock};

use rand::{Rng, RngCore};
use restate_sdk::{discovery::ServiceType, prelude::*};
use serde::Deserialize;
use serde_with::serde_as;

use super::{BoxStep, ExecutionContext, JsonValue, Step, StepError, StepFactory};

pub static STEPS: LazyLock<HashMap<String, Box<dyn StepFactory>>> = LazyLock::new(|| {
    let mut steps: HashMap<String, Box<dyn StepFactory>> = HashMap::default();

    steps.insert("echo".to_owned(), Box::new(Echo));
    steps.insert("sleep".to_owned(), Box::new(Sleep));
    steps.insert("set".to_owned(), Box::new(Set));
    steps.insert("random".to_owned(), Box::new(Random));
    steps.insert("return".to_owned(), Box::new(Return));

    steps
});

struct Echo;

impl StepFactory for Echo {
    fn new(&self, _params: serde_yaml::Value) -> Result<BoxStep, StepError> {
        Ok(Box::new(EchoStep))
    }
}
struct EchoStep;

#[async_trait::async_trait]
impl Step for EchoStep {
    fn validate(&self, _service_type: ServiceType) -> Result<(), StepError> {
        Ok(())
    }

    async fn run(
        &self,
        _ctx: &WorkflowContext<'_>,
        exec: &mut ExecutionContext,
        input: &JsonValue,
    ) -> Result<(), HandlerError> {
        exec.return_value(input.clone());
        Ok(())
    }
}

struct Sleep;

impl StepFactory for Sleep {
    fn new(&self, params: serde_yaml::Value) -> Result<BoxStep, StepError> {
        let step: SleepStep = serde_yaml::from_value(params)?;
        Ok(Box::new(step))
    }
}

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct SleepStep {
    #[serde_as(as = "serde_with::DisplayFromStr")]
    duration: humantime::Duration,
}

#[async_trait::async_trait]
impl Step for SleepStep {
    fn validate(&self, _service_type: ServiceType) -> Result<(), StepError> {
        Ok(())
    }

    async fn run(
        &self,
        ctx: &WorkflowContext<'_>,
        _step: &mut ExecutionContext,
        _input: &JsonValue,
    ) -> Result<(), HandlerError> {
        ctx.sleep(self.duration.into()).await?;

        Ok(())
    }
}

struct Set;

impl StepFactory for Set {
    fn new(&self, params: serde_yaml::Value) -> Result<BoxStep, StepError> {
        let step: SetVariableStep = serde_yaml::from_value(params)?;
        Ok(Box::new(step))
    }
}

#[derive(Debug, Deserialize)]
struct SetVariableStep {
    key: String,
    input: String,
}

#[async_trait::async_trait]
impl Step for SetVariableStep {
    fn validate(&self, service_type: ServiceType) -> Result<(), StepError> {
        if service_type == ServiceType::Service {
            return Err(StepError::InvalidServiceType(service_type));
        }

        Ok(())
    }

    async fn run(
        &self,
        ctx: &WorkflowContext<'_>,
        exec: &mut ExecutionContext,
        _input: &JsonValue,
    ) -> Result<(), HandlerError> {
        ctx.set(
            &self.key,
            exec.get_variable(&self.input)
                .ok_or_else(|| TerminalError::new(format!("unkown variable {}", self.input)))?
                .clone(),
        );

        Ok(())
    }
}

struct Random;

impl StepFactory for Random {
    fn new(&self, params: serde_yaml::Value) -> Result<BoxStep, StepError> {
        let step: RandomStep = serde_yaml::from_value(params)?;
        Ok(Box::new(step))
    }
}

#[derive(Debug, Deserialize)]
struct RandomStep {
    size: u16,
    output: String,
}

#[async_trait::async_trait]
impl Step for RandomStep {
    fn validate(&self, _service_type: ServiceType) -> Result<(), StepError> {
        Ok(())
    }

    async fn run(
        &self,
        _ctx: &WorkflowContext<'_>,
        exec: &mut ExecutionContext,
        _input: &JsonValue,
    ) -> Result<(), HandlerError> {
        let mut rng = rand::rng();
        let bytes: Vec<u8> = (0..self.size).map(|_| rng.random()).collect();

        exec.set_variable(&self.output, bytes);

        Ok(())
    }
}

struct Return;

impl StepFactory for Return {
    fn new(&self, params: serde_yaml::Value) -> Result<BoxStep, StepError> {
        let step: ReturnStep = serde_yaml::from_value(params)?;
        Ok(Box::new(step))
    }
}

#[derive(Debug, Deserialize)]
struct ReturnStep {
    output: String,
}

#[async_trait::async_trait]
impl Step for ReturnStep {
    fn validate(&self, _service_type: ServiceType) -> Result<(), StepError> {
        Ok(())
    }

    async fn run(
        &self,
        _ctx: &WorkflowContext<'_>,
        exec: &mut ExecutionContext,
        _input: &JsonValue,
    ) -> Result<(), HandlerError> {
        let variable = exec
            .get_variable(&self.output)
            .ok_or_else(|| TerminalError::new(format!("unkown variable {}", self.output)))?;

        exec.return_value(JsonValue::try_from(variable.clone())?);

        Ok(())
    }
}
