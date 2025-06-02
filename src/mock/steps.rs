use std::{collections::HashMap, sync::LazyLock, time::Duration};

use anyhow::Context;
use rand::Rng;
use restate_sdk::{context::RequestTarget, discovery::ServiceType, prelude::*};
use serde::Deserialize;
use serde_with::serde_as;

use super::{
    context::Variable, BoxStep, ExecutionContext, JsonValue, Step, StepError, StepFactory,
};

pub static STEPS: LazyLock<HashMap<String, Box<dyn StepFactory>>> = LazyLock::new(|| {
    let mut steps: HashMap<String, Box<dyn StepFactory>> = HashMap::default();

    steps.insert("echo".to_owned(), Box::new(Echo));
    steps.insert("sleep".to_owned(), Box::new(Sleep));
    steps.insert("busy".to_owned(), Box::new(Busy));
    steps.insert("set".to_owned(), Box::new(Set));
    steps.insert("get".to_owned(), Box::new(Get));
    steps.insert("random".to_owned(), Box::new(Random));
    steps.insert("increment".to_owned(), Box::new(Increment));
    steps.insert("call".to_owned(), Box::new(Call));
    steps.insert("send".to_owned(), Box::new(Send));
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

/// A step that pauses execution for a specified duration. This step utilizes the Restate SDK's
/// `ctx.sleep()` method, meaning the sleep is managed by the Restate runtime and is durable.
/// It can be useful for simulating delays that should persist across retries or service restarts.
#[serde_as]
#[derive(Debug, Deserialize)]
pub struct SleepStep {
    /// The base duration for which to sleep. Parsed from a human-readable string like "2s" or "500ms".
    #[serde_as(as = "serde_with::DisplayFromStr")]
    duration: humantime::Duration,
    /// Optional: A factor (0.0 to 1.0) to add random jitter to the sleep duration.
    /// The actual jitter duration will be a random value between 0 and `jitter * duration`.
    /// For example, if `duration` is `10s` and `jitter` is `0.1`, an additional random delay
    /// between `0s` and `1s` will be added.
    jitter: Option<f32>,
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
        let jitter = self
            .jitter
            .map(|j| rand::random_range(0.0..=j) * self.duration.as_secs_f32())
            .map(Duration::from_secs_f32);

        let duration = Duration::from(self.duration) + jitter.unwrap_or_default();

        ctx.sleep(duration).await?;

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

        exec.set(&self.output, value);

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

        exec.set(&self.output, bytes);

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

        exec.return_value(serde_json::to_value(variable)?);

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

        exec.set(&self.input, value);

        Ok(())
    }
}

struct Call;

impl StepFactory for Call {
    fn create(&self, params: serde_yaml::Value) -> Result<BoxStep, StepError> {
        let step: CallStep = serde_yaml::from_value(params)?;
        Ok(Box::new(step))
    }
}

/// A step that makes a call to another handler, which can be part of any service,
/// virtual object, or workflow defined within the mock service configuration.
///
/// This step allows for complex interactions and chaining of logic across different
/// components of the mock setup.
#[derive(Debug, Deserialize)]
struct CallStep {
    /// Specifies the type of the target handler to be called (SERVICE, VIRTUAL_OBJECT, or WORKFLOW).
    target_type: ServiceType,
    /// The string name of the target service, virtual object, or workflow.
    service: String,
    /// The string name of the target handler to invoke on the specified service.
    handler: String,
    /// The string key to use when `target_type` is `VIRTUAL_OBJECT` or `WORKFLOW`.
    /// If the current service (caller) is a `VIRTUAL_OBJECT` or `WORKFLOW` and `key` is `None`,
    /// the key of the current service instance is used.
    /// Required if `target_type` is `VIRTUAL_OBJECT`/`WORKFLOW` and the caller is `SERVICE`,
    /// or if a specific key different from the caller's key is needed.
    key: Option<String>,
    /// Optional: The name of a variable in the execution context whose value will be sent as input.
    /// If `None` or the variable doesn't exist, `null` is sent.
    input: Option<String>,
    /// Optional: The name of a variable in the execution context to store the call's result.
    /// If `None`, the result is discarded.
    output: Option<String>,
}

#[async_trait::async_trait]
impl Step for CallStep {
    fn validate(&self, _service_type: ServiceType) -> Result<(), StepError> {
        Ok(())
    }

    async fn run(
        &self,
        ctx: &WorkflowContext<'_>,
        exec: &mut ExecutionContext,
        _input: &JsonValue,
    ) -> Result<(), HandlerError> {
        let request_target = match self.target_type {
            ServiceType::Service => RequestTarget::Service {
                name: self.service.clone(),
                handler: self.handler.clone(),
            },
            ServiceType::VirtualObject => RequestTarget::Object {
                name: self.service.clone(),
                key: self.key.clone().unwrap_or_else(|| ctx.key().to_string()),
                handler: self.handler.clone(),
            },
            ServiceType::Workflow => RequestTarget::Workflow {
                name: self.service.clone(),
                key: self.key.clone().unwrap_or_else(|| ctx.key().to_string()),
                handler: self.handler.clone(),
            },
        };

        let req = self
            .input
            .as_ref()
            .and_then(|input| exec.get_variable(input))
            .cloned()
            .unwrap_or(Variable::Null);

        let res: Variable = ctx.request(request_target, req).call().await?;
        if let Some(output) = self.output.as_ref() {
            exec.set(output, res);
        }

        Ok(())
    }
}

struct Send;

impl StepFactory for Send {
    fn create(&self, params: serde_yaml::Value) -> Result<BoxStep, StepError> {
        let step: SendStep = serde_yaml::from_value(params)?;
        Ok(Box::new(step))
    }
}

/// A step that sends a call to another handler, which can be part of any service,
/// virtual object, or workflow defined within the mock service configuration.
///
/// This step allows for complex interactions and chaining of logic across different
/// components of the mock setup.
///
/// Similar to [`CallStep`] but does not wait for output
#[derive(Debug, Deserialize)]
struct SendStep {
    /// Specifies the type of the target handler to be called (SERVICE, VIRTUAL_OBJECT, or WORKFLOW).
    target_type: ServiceType,
    /// The string name of the target service, virtual object, or workflow.
    service: String,
    /// The string name of the target handler to invoke on the specified service.
    handler: String,
    /// The string key to use when `target_type` is `VIRTUAL_OBJECT` or `WORKFLOW`.
    /// If the current service (caller) is a `VIRTUAL_OBJECT` or `WORKFLOW` and `key` is `None`,
    /// the key of the current service instance is used.
    /// Required if `target_type` is `VIRTUAL_OBJECT`/`WORKFLOW` and the caller is `SERVICE`,
    /// or if a specific key different from the caller's key is needed.
    key: Option<String>,
    /// Optional: The name of a variable in the execution context whose value will be sent as input.
    /// If `None` or the variable doesn't exist, `null` is sent.
    input: Option<String>,
}

#[async_trait::async_trait]
impl Step for SendStep {
    fn validate(&self, _service_type: ServiceType) -> Result<(), StepError> {
        Ok(())
    }

    async fn run(
        &self,
        ctx: &WorkflowContext<'_>,
        exec: &mut ExecutionContext,
        _input: &JsonValue,
    ) -> Result<(), HandlerError> {
        let request_target = match self.target_type {
            ServiceType::Service => RequestTarget::Service {
                name: self.service.clone(),
                handler: self.handler.clone(),
            },
            ServiceType::VirtualObject => RequestTarget::Object {
                name: self.service.clone(),
                key: self.key.clone().unwrap_or_else(|| ctx.key().to_string()),
                handler: self.handler.clone(),
            },
            ServiceType::Workflow => RequestTarget::Workflow {
                name: self.service.clone(),
                key: self.key.clone().unwrap_or_else(|| ctx.key().to_string()),
                handler: self.handler.clone(),
            },
        };

        let req = self
            .input
            .as_ref()
            .and_then(|input| exec.get_variable(input))
            .cloned()
            .unwrap_or(Variable::Null);

        ctx.request::<_, ()>(request_target, req).send();

        Ok(())
    }
}

struct Busy;

impl StepFactory for Busy {
    fn create(&self, params: serde_yaml::Value) -> Result<BoxStep, StepError> {
        let step: BusyStep = serde_yaml::from_value(params)?;
        Ok(Box::new(step))
    }
}

/// A step that simulates a busy handler by causing the current thread to sleep for a specified duration.
/// Unlike [`SleepStep`], this sleep is handled directly within the mock service using `tokio::time::sleep()`
/// and is not managed by the Restate runtime. This is useful for simulating CPU-bound work or
/// other synchronous delays within the handler itself, without involving durable timers.
#[serde_as]
#[derive(Debug, Deserialize)]
struct BusyStep {
    /// The base duration for which the handler will simulate being busy.
    /// Parsed from a human-readable string (e.g., "100ms", "1s").
    #[serde_as(as = "serde_with::DisplayFromStr")]
    duration: humantime::Duration,
    /// Optional: A factor (0.0 to 1.0) to add random jitter to the busy duration.
    /// The actual jitter duration will be a random value between 0 and `jitter * duration`.
    jitter: Option<f32>,
}

#[async_trait::async_trait]
impl Step for BusyStep {
    fn validate(&self, _service_type: ServiceType) -> Result<(), StepError> {
        Ok(())
    }

    async fn run(
        &self,
        _ctx: &WorkflowContext<'_>,
        _exec: &mut ExecutionContext,
        _input: &JsonValue,
    ) -> Result<(), HandlerError> {
        let jitter = self
            .jitter
            .map(|j| rand::random_range(0.0..=j) * self.duration.as_secs_f32())
            .map(Duration::from_secs_f32);

        let duration = Duration::from(self.duration) + jitter.unwrap_or_default();

        tokio::time::sleep(duration).await;

        Ok(())
    }
}
