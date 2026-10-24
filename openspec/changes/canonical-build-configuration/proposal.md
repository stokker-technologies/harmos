# Canonical Build Configuration

## Why

Product and Tests repeat broad feature configurations and currently run sequentially in one Build job.

## What Changes

Run independent parallel Product and Tests jobs using existing canonical shipping configurations per native platform and profile, owned by repository mise tasks. Preserve all test sources and packaging contracts. Move simulation and other noncanonical feature stories, all-features, no-default-features and cross-target compatibility checks to the explicit optional local extended suite. No provider CI context or ordinary installer invokes that suite. Measure comparable-cache Tests Build against Product Build and the complete warm-cache pipeline against the required time budget.

## Impact

Build tasks, authored build/test producers, CI selection, affected tests, maintained specifications and README claims. Hub owns shared guidance and generated tooling. No release publication is authorized.

