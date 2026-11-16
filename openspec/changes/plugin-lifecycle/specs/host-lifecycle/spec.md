## Purpose

Define the lifecycle verbs of a product-built runtime and of SDK serving entry points: construction, registration, start, stop and run, each verb paired.

## ADDED Requirements

### Requirement: Host Lifecycle Order

A product-built runtime SHALL be constructed with `new`, declare its contributions with `register`, begin activity with `start` and end it with `stop`. Construction and registration SHALL start no listener, plugin, service or storage worker.

#### Scenario: Register Before Start

- **WHEN** a product constructs a runtime and registers its catalog
- **THEN** no listener, plugin or service is running until the product calls start

#### Scenario: Stop Settles Owners

- **WHEN** a product stops a started runtime
- **THEN** every listener, plugin activation and service ends and its finalize obligations settle before stop returns

### Requirement: Run Is Start, Wait, Stop

`run` SHALL start the runtime, wait until a stop is requested and then stop it, returning the stop result. It SHALL add no phase beyond start and stop.

#### Scenario: Run Until Stopped

- **WHEN** a product runs a runtime and later requests a stop
- **THEN** run returns after the same stop sequence an explicit stop performs

### Requirement: Paired Host Verbs Only

The runtime and SDK serving surfaces SHALL use `start` with `stop` and `run` for start, wait, stop, in every language with idiomatic casing only. The verbs `launch`, `shutdown` and `serve` SHALL NOT denote these phases on any public surface.

#### Scenario: Serving a Plugin Process

- **WHEN** a plugin author serves a plugin from Rust, Python, C# or C++
- **THEN** the entry point is named run (RunAsync in C#), and a Python endpoint started with start ends with stop
