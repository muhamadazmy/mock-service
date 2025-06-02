use std::collections::HashMap;

use restate_sdk::discovery::{HandlerType, ServiceType};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct ServiceConfig {
    name: String,
    #[serde(rename = "type")]
    ty: ServiceType,
    handlers: HashMap<String, HandlerConfig>,
}

#[derive(Debug, Deserialize)]
struct HandlerConfig {
    name: String,
    #[serde(rename = "type")]
    ty: Option<HandlerType>,
    steps: Vec<StepConfig>,
}

#[derive(Debug, Deserialize)]
struct StepConfig {
    #[serde(rename = "type")]
    ty: String,
    params: serde_yaml::Value,
}

#[derive(Debug, Deserialize)]
struct Config {
    #[serde(flatten)]
    services: HashMap<String, ServiceConfig>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_yaml;

    const SAMPLE_CONFIG: &str = include_str!("../assets/test_config.yaml");

    #[test]
    fn test_parse_config() {
        let config: HashMap<String, ServiceConfig> = serde_yaml::from_str(SAMPLE_CONFIG).unwrap();

        // Test service level configuration
        let counter_service = config.get("counter").unwrap();
        assert_eq!(counter_service.name, "counter");
        assert_eq!(counter_service.ty, ServiceType::Service);

        // Test handlers
        let handlers = &counter_service.handlers;
        assert_eq!(handlers.len(), 2);

        // Test increment handler
        let increment = handlers.get("increment").unwrap();
        assert_eq!(increment.name, "increment");
        assert_eq!(increment.ty, Some(HandlerType::Exclusive));
        assert_eq!(increment.steps.len(), 2);

        // Test get_count handler
        let get_count = handlers.get("get_count").unwrap();
        assert_eq!(get_count.name, "get_count");
        assert_eq!(get_count.ty, Some(HandlerType::Shared));
        assert_eq!(get_count.steps.len(), 1);

        // Test step configuration
        let increment_steps = &increment.steps;
        assert_eq!(increment_steps[0].ty, "sleep");
        assert_eq!(increment_steps[1].ty, "success");
    }
}
