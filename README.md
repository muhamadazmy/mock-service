# Mock Service

A flexible service for mocking responses and behaviors, configurable via YAML. Built with Rust and the Restate SDK.

## How to Use

1.  **Define your mock services in a YAML configuration file.** See the example below for the structure.
2.  **Run the mock service**, pointing it to your configuration file:

    ```bash
    cargo run -- --config-file <your_config_file.yaml>
    ```

    By default, the service listens on `0.0.0.0:9200`. You can change this with the `--listen-address` flag:

    ```bash
    cargo run -- --config-file <your_config_file.yaml> --listen-address <ip>:<port>
    ```

## Example YAML Configuration

```yaml
echo:
  type: SERVICE # Can be SERVICE or VIRTUAL_OBJECT
  handlers:
    echo: # Name of the handler
      steps: # A list of steps to execute
        - type: sleep
          params:
            duration: 2s # Human-readable duration (e.g., 500ms, 2s, 1m)
        - type: echo
random:
  type: VIRTUAL_OBJECT
  handlers:
    random:
      steps:
        - type: random
          params:
            size: 50 # Number of random bytes to generate
            output: random_bytes # Variable name to store the random bytes
        - type: set
          params:
            key: my_key # Key for storing the value (only for VIRTUAL_OBJECT)
            input: random_bytes # Variable name holding the value to store
        - type: return
          params:
            output: random_bytes # Variable name whose value will be returned
    get:
      steps:
        - type: get
          params:
            key: my_key # Key to retrieve the value from (only for VIRTUAL_OBJECT)
            output: random_bytes # Variable name to store the retrieved value
        - type: return
          params:
            output: random_bytes
counter:
  type: VIRTUAL_OBJECT
  handlers:
    counter:
      steps:
        - type: get
          params:
            key: counter
            output: value
        - type: increment
          params:
            input: value # Variable name holding the number to increment
            steps: 2 # Amount to increment by (defaults to 1)
        - type: set
          params:
            key: counter
            input: value
        - type: return
          params:
            output: value
```

> Also check [example.yaml](example.yaml) for a more comprehensive example

## Available Steps

The following steps can be used in your handler configurations:

### `echo`

Echoes back the input it receives.

*   **Params**: None

### `sleep`

Pauses execution for a specified duration. This step utilizes the Restate SDK's `ctx.sleep()` method, meaning the sleep is managed by the Restate runtime and is durable. It can be useful for simulating delays that should persist across retries or service restarts.

*   **Params**:
    *   `duration`: (Required) The base duration for which to sleep. Parsed from a human-readable string (e.g., `100ms`, `2s`, `1m`).
    *   `jitter`: (Optional) A factor (e.g., `0.1` for 10%) to add random jitter to the sleep duration. The actual jitter duration will be a random value between `0` and `jitter * duration`. For example, if `duration` is `10s` and `jitter` is `0.1`, an additional random delay between `0s` and `1s` will be added to the base `10s` duration.

### `busy`

Simulates a busy handler by causing the current handler's execution to sleep for a specified duration. Unlike the `sleep` step, this uses `tokio::time::sleep()` and is handled directly within the mock service, not by the Restate runtime. This is useful for simulating CPU-bound work or other synchronous delays within the handler itself, without involving durable Restate timers.

*   **Params**:
    *   `duration`: (Required) The base duration for which the handler will simulate being busy. Parsed from a human-readable string (e.g., `100ms`, `1s`).
    *   `jitter`: (Optional) A factor (e.g., `0.1` for 10%) to add random jitter to the busy duration. The actual jitter duration will be a random value between `0` and `jitter * duration`.

### `set`

Sets a key-value pair in the Restate state for the current virtual object.
**Note:** This step is only valid for services of type `VIRTUAL_OBJECT`.

*   **Params**:
    *   `key`: (Required) The string key to store the value under.
    *   `input`: (Required) The name of the variable in the execution context whose value will be stored.

### `get`

Retrieves a value from the Restate state for the current virtual object and stores it in a variable.
**Note:** This step is only valid for services of type `VIRTUAL_OBJECT`.

*   **Params**:
    *   `key`: (Required) The string key of the value to retrieve.
    *   `output`: (Required) The name of the variable in the execution context where the retrieved value will be stored. If the key is not found, `null` will be stored.

### `random`

Generates a specified number of random bytes and stores them in a variable.

*   **Params**:
    *   `size`: (Required) The number of random bytes to generate (integer).
    *   `output`: (Required) The name of the variable in the execution context where the byte array will be stored.

### `increment`

Increments a numerical value stored in a variable. If the variable doesn't exist or is not a number, it defaults to 0 before incrementing.

*   **Params**:
    *   `input`: (Required) The name of the variable in the execution context holding the numerical value to increment. The result is stored back in the same variable.
    *   `steps`: (Optional) The integer amount to increment by. Defaults to `1`.

### `call`

Makes a call to another handler, which can be part of any service, virtual object, or workflow defined within the mock service configuration. This step allows for complex interactions and chaining of logic across different components of your mock setup.

*   **Params**:
    *   `target_type`: (Required) Specifies the type of the target handler to be called. Must be one of:
        *   `SERVICE`: For calling a handler on a stateless service.
        *   `VIRTUAL_OBJECT`: For calling a handler on a keyed virtual object.
        *   `WORKFLOW`: For calling a handler on a keyed workflow.
    *   `service`: (Required) The string name of the target service, virtual object, or workflow (as defined in your YAML configuration).
    *   `handler`: (Required) The string name of the target handler to invoke on the specified `service`.
    *   `key`: (Optional/Conditionally Required) The string key to use when `target_type` is `VIRTUAL_OBJECT` or `WORKFLOW`.
        *   If the current service (the one executing this `call` step) is itself a `VIRTUAL_OBJECT` or `WORKFLOW`, and this `key` parameter is omitted in the YAML, the key of the current service instance will automatically be used for the target call.
        *   This parameter **is required** if `target_type` is `VIRTUAL_OBJECT` or `WORKFLOW` AND the current service (the caller) is of type `SERVICE`. It is also required if you intend to target a specific key different from the current service's key (when calling from a `VIRTUAL_OBJECT` or `WORKFLOW`).
    *   `input`: (Optional) The name of a variable existing in the current execution context. The value of this variable will be serialized and sent as the input payload to the target handler. If this parameter is omitted, or if the specified variable does not exist in the context, a `null` value will be sent as input.
    *   `output`: (Optional) The name of a variable in the current execution context. The result returned by the invoked target handler will be deserialized and stored in this variable. If this parameter is omitted, the result of the call is effectively discarded (not stored).

### `send`

Similar to `call` but does not wait for call to complete.

*   **Params**:
    *   `target_type`: (Required) Specifies the type of the target handler to be called. Must be one of:
        *   `SERVICE`: For calling a handler on a stateless service.
        *   `VIRTUAL_OBJECT`: For calling a handler on a keyed virtual object.
        *   `WORKFLOW`: For calling a handler on a keyed workflow.
    *   `service`: (Required) The string name of the target service, virtual object, or workflow (as defined in your YAML configuration).
    *   `handler`: (Required) The string name of the target handler to invoke on the specified `service`.
    *   `key`: (Optional/Conditionally Required) The string key to use when `target_type` is `VIRTUAL_OBJECT` or `WORKFLOW`.
        *   If the current service (the one executing this `call` step) is itself a `VIRTUAL_OBJECT` or `WORKFLOW`, and this `key` parameter is omitted in the YAML, the key of the current service instance will automatically be used for the target call.
        *   This parameter **is required** if `target_type` is `VIRTUAL_OBJECT` or `WORKFLOW` AND the current service (the caller) is of type `SERVICE`. It is also required if you intend to target a specific key different from the current service's key (when calling from a `VIRTUAL_OBJECT` or `WORKFLOW`).
    *   `input`: (Optional) The name of a variable existing in the current execution context. The value of this variable will be serialized and sent as the input payload to the target handler. If this parameter is omitted, or if the specified variable does not exist in the context, a `null` value will be sent as input.

### `loop`

Executes a sequence of nested steps repeatedly for a specified number of iterations, or indefinitely.

*   **Params**:
    *   `count`: (Optional) An integer specifying the number of times to execute the nested `steps`. If omitted, the loop will run indefinitely (technically, up to `usize::MAX` times, which is a very large number).
    *   `steps`: (Required) A list of step configurations. These steps will be executed in order during each iteration of the loop. The same execution context and input (from the handler's perspective) are passed to these nested steps.

### `return`

Ends the handler execution and returns the value of a specified variable.

*   **Params**:
    *   `output`: (Required) The name of the variable in the execution context whose value will be returned as the result of the handler.
