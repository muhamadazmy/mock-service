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

## Available Steps

The following steps can be used in your handler configurations:

### `echo`

Echoes back the input it receives.

*   **Params**: None

### `sleep`

Pauses execution for a specified duration.

*   **Params**:
    *   `duration`: (Required) A human-readable duration string (e.g., `100ms`, `2s`, `1m`).

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

### `return`

Ends the handler execution and returns the value of a specified variable.

*   **Params**:
    *   `output`: (Required) The name of the variable in the execution context whose value will be returned as the result of the handler.
