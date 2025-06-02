use std::{collections::HashMap, sync::LazyLock};

use anyhow::Context;
use rand::Rng;
use restate_sdk::{discovery::ServiceType, prelude::*};
use serde::Deserialize;
use serde_with::serde_as;

use super::{
    context::Variable, BoxStep, ExecutionContext, JsonValue, Step, StepError, StepFactory,
};

pub static STEPS: LazyLock<HashMap<String, Box<dyn StepFactory>>> = LazyLock::new(|| {
    let mut steps: HashMap<String, Box<dyn StepFactory>> = HashMap::default();

    steps.insert("echo".to_owned(), Box::new(Echo));
    steps.insert("sleep".to_owned(), Box::new(Sleep));
    steps.insert("set".to_owned(), Box::new(Set));
    steps.insert("get".to_owned(), Box::new(Get));
    steps.insert("random".to_owned(), Box::new(Random));
    steps.insert("increment".to_owned(), Box::new(Increment));
    steps.insert("return".to_owned(), Box::new(Return));

    steps
});

/// Factory for creating `EchoStep` instances.
struct Echo;

impl StepFactory for Echo {
    fn create(&self, _params: serde_yaml::Value) -> Result<BoxStep, StepError> {
        Ok(Box::new(EchoStep))
    }
}

/// A step that echoes back the input it receives.
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

/// Factory for creating `SleepStep` instances.
struct Sleep;

impl StepFactory for Sleep {
    fn create(&self, params: serde_yaml::Value) -> Result<BoxStep, StepError> {
        let step: SleepStep = serde_yaml::from_value(params)?;
        Ok(Box::new(step))
    }
}

/// A step that pauses execution for a specified duration.
#[serde_as]
#[derive(Debug, Deserialize)]
pub struct SleepStep {
    /// The duration for which to sleep. Parsed from a human-readable string like "2s" or "500ms".
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

/// Factory for creating `SetStep` instances.
struct Set;

impl StepFactory for Set {
    fn create(&self, params: serde_yaml::Value) -> Result<BoxStep, StepError> {
        let step: SetStep = serde_yaml::from_value(params)?;
        Ok(Box::new(step))
    }
}

/// A step that sets a key-value pair in the Restate state for the current virtual object.
/// This step is only valid for services of type `VIRTUAL_OBJECT`.
#[derive(Debug, Deserialize)]
struct SetStep {
    /// The string key to store the value under.
    key: String,
    /// The name of the variable in the execution context whose value will be stored.
    input: String,
}

#[async_trait::async_trait]
impl Step for SetStep {
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

/// Factory for creating `GetStep` instances.
struct Get;

impl StepFactory for Get {
    fn create(&self, params: serde_yaml::Value) -> Result<BoxStep, StepError> {
        let step: GetStep = serde_yaml::from_value(params)?;
        Ok(Box::new(step))
    }
}

/// A step that retrieves a value from the Restate state for the current virtual object
/// and stores it in a variable.
/// This step is only valid for services of type `VIRTUAL_OBJECT`.
#[derive(Debug, Deserialize)]
struct GetStep {
    /// The string key of the value to retrieve.
    key: String,
    /// The name of the variable in the execution context where the retrieved value will be stored.
    /// If the key is not found, `null` will be stored in the variable.
    output: String,
}

#[async_trait::async_trait]
impl Step for GetStep {
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
        let value: Variable = ctx.get(&self.key).await?.unwrap_or(Variable::Null);

        exec.set_variable(&self.output, value);

        Ok(())
    }
}

/// Factory for creating `RandomStep` instances.
struct Random;

impl StepFactory for Random {
    fn create(&self, params: serde_yaml::Value) -> Result<BoxStep, StepError> {
        let step: RandomStep = serde_yaml::from_value(params)?;
        Ok(Box::new(step))
    }
}

/// A step that generates a specified number of random bytes and stores them in a variable.
#[derive(Debug, Deserialize)]
struct RandomStep {
    /// The number of random bytes to generate.
    size: u16,
    /// The name of the variable in the execution context where the byte array will be stored.
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

/// Factory for creating `ReturnStep` instances.
struct Return;

impl StepFactory for Return {
    fn create(&self, params: serde_yaml::Value) -> Result<BoxStep, StepError> {
        let step: ReturnStep = serde_yaml::from_value(params)?;
        Ok(Box::new(step))
    }
}

/// A step that ends the handler execution and returns the value of a specified variable.
#[derive(Debug, Deserialize)]
struct ReturnStep {
    /// The name of the variable in the execution context whose value will be returned
    /// as the result of the handler.
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

/// Factory for creating `IncrementStep` instances.
struct Increment;

impl StepFactory for Increment {
    fn create(&self, params: serde_yaml::Value) -> Result<BoxStep, StepError> {
        let step: IncrementStep = serde_yaml::from_value(params)?;
        Ok(Box::new(step))
    }
}

/// A step that increments a numerical value stored in a variable.
/// If the variable doesn't exist or is not a number, it defaults to 0 before incrementing.
#[derive(Debug, Deserialize)]
struct IncrementStep {
    /// The name of the variable in the execution context holding the numerical value to increment.
    /// The result is stored back in the same variable.
    input: String,
    /// The integer amount to increment by. Defaults to `1`.
    #[serde(default = "default_steps")]
    steps: isize,
}

fn default_steps() -> isize {
    1
}

#[async_trait::async_trait]
impl Step for IncrementStep {
    fn validate(&self, _service_type: ServiceType) -> Result<(), StepError> {
        Ok(())
    }

    async fn run(
        &self,
        _ctx: &WorkflowContext<'_>,
        exec: &mut ExecutionContext,
        _input: &JsonValue,
    ) -> Result<(), HandlerError> {
        let mut value = exec
            .get::<isize>(&self.input)
            .unwrap_or(Ok(0))
            .context("Failed to get variable")?;

        value += self.steps;

        exec.set_variable(&self.input, value);

        Ok(())
    }
}
