use std::{collections::HashMap, sync::LazyLock};

use restate_sdk::{discovery::ServiceType, prelude::*};
use serde::Deserialize;
use serde_with::serde_as;

use super::{BoxStep, ExecutionContext, JsonValue, Step, StepError, StepFactory};

pub static STEPS: LazyLock<HashMap<String, Box<dyn StepFactory>>> = LazyLock::new(|| {
    let mut steps: HashMap<String, Box<dyn StepFactory>> = HashMap::default();

    steps.insert("echo".to_owned(), Box::new(Echo));
    steps.insert("sleep".to_owned(), Box::new(Sleep));
    steps
});

pub struct Echo;

impl StepFactory for Echo {
    fn new(&self, _params: serde_yaml::Value) -> Result<BoxStep, StepError> {
        Ok(Box::new(EchoStep))
    }
}
pub struct EchoStep;

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

pub struct Sleep;

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
