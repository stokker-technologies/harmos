# Rendered by stkr project update from templates/python/ci/simulate.py; sha256:3dedc0f3b0db8d3352d5f386de09c1c50a0d4168cb64e87e83994ebda74b1e5b
# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***
"""Execute The Rendered CI Graph In Isolated Native And Docker Jobs."""

# === Imports === #

# Standard Library Imports
import argparse
import concurrent.futures
import ctypes
import hashlib
import importlib.util
import json
import os
import queue
import shutil
import subprocess
import sys
import tarfile
import tempfile
import threading
import time
import traceback
import uuid
from itertools import chain
from pathlib import Path

# Third Party Imports
# Local Imports
from ..process import spawn_flags
from . import containers, impact, runs, workspaces
from . import environment as step_environment
from . import summary as reporting

# Type Checking Imports


# === Public Exports === #

# === Constants === #

PLAN = json.loads(
    '{"dispatch_guard":null,"platforms":["unix","windows"],"jobs":[{"key":"source","id":"check:repository","platform":"unix","independent":true,"pipelines":["pr","dev","release","nightly"],"release_review_only":false,"needs":[],"inputs":[],"outputs":{},"commands":[{"run":"mise run check:repository","cached":false,"shell":null,"condition":null}],"bootstrap":[],"setup":"python3 .github/actions/setup/run.py --platform unix","provider_setup":[],"tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:taiki-e/cargo-llvm-cov,github:bnjbvr/cargo-machete,sccache,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,taplo,git-cliff","skip":null,"compiler_cache":false,"image":"managed:ubuntu-24.04","minutes":120},{"key":"check_python","id":"check:python","platform":"unix","independent":true,"pipelines":["pr","dev","release","nightly"],"release_review_only":false,"needs":[],"inputs":[],"outputs":{},"commands":[{"run":"mise run check:python","cached":false,"shell":null,"condition":null}],"bootstrap":[],"setup":"python3 .github/actions/setup/run.py --platform unix","provider_setup":[],"tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,git-cliff","skip":null,"compiler_cache":false,"image":"managed:ubuntu-24.04","minutes":120},{"key":"bench_rust_unit","id":"bench:rust:unit","platform":"unix","independent":true,"pipelines":["nightly"],"release_review_only":false,"needs":["build_bench","check_generated_python","check_python","check_rust_unix","check_rust_windows","source","validate_coverage_rust","validate_test_rust_acceptance_unix","validate_test_rust_acceptance_windows","validate_test_rust_integration_unix","validate_test_rust_integration_windows","validate_test_rust_starter_unix","validate_test_rust_starter_windows","validate_test_rust_system_unix","validate_test_rust_system_windows","validate_test_rust_unit_unix","validate_test_rust_unit_windows"],"inputs":["build:rust:bench#bench"],"outputs":{},"commands":[{"run":"mise run bench:rust:unit --refresh-baseline","cached":false,"shell":null,"condition":null}],"bootstrap":[],"setup":"python3 .github/actions/setup/run.py --platform unix","provider_setup":[],"tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,git-cliff","skip":"build:rust:bench","compiler_cache":false,"image":"managed:ubuntu-24.04","minutes":120},{"key":"check_rust_windows","id":"check:rust","platform":"windows","independent":false,"pipelines":["pr","dev","release","nightly"],"release_review_only":false,"needs":[],"inputs":[],"outputs":{},"commands":[{"run":"mise run check:rust","cached":true,"shell":null,"condition":null}],"bootstrap":[],"setup":"python .github/actions/setup/run.py --platform windows","provider_setup":[],"tools":"aqua:koalaman/shellcheck,github:bnjbvr/cargo-machete,go,go:github.com/rhysd/actionlint/cmd/actionlint,node,python,rust,sccache,taplo,uv","skip":null,"compiler_cache":true,"image":"managed:ubuntu-24.04","minutes":120},{"key":"build_product_windows","id":"build","platform":"windows","independent":false,"pipelines":["pr","dev","release","nightly"],"release_review_only":false,"needs":[],"inputs":[],"outputs":{"product":["target/build/{profile}/{target}/bin","target/build/{profile}/{target}/lib","target/build/{profile}/{target}/product","target/generated/python/pyproject.toml","target/generated/python/src"]},"commands":[{"run":"mise exec -- uv run --locked --project tools/python python -m harmos_tools.ci.build --profile ${{ env.PROFILE }} --output product --task build","cached":false,"shell":null,"condition":null},{"run":"mise exec -- uv run --locked --project tools/python python -m harmos_tools.ci.build --profile release --output product --task build","cached":false,"shell":null,"condition":"env.PROFILE == \'dev\'"}],"bootstrap":[],"setup":"python .github/actions/setup/run.py --platform windows","provider_setup":[],"tools":"aqua:koalaman/shellcheck,aqua:nextest-rs/nextest/cargo-nextest,github:bnjbvr/cargo-machete,go,go:github.com/rhysd/actionlint/cmd/actionlint,node,python,rust,sccache,taplo,uv","skip":null,"compiler_cache":true,"image":"managed:ubuntu-24.04","minutes":120},{"key":"build_tests_windows","id":"build:rust:tests","platform":"windows","independent":false,"pipelines":["pr","dev","release","nightly"],"release_review_only":false,"needs":[],"inputs":[],"outputs":{"fixtures":["target/build/{profile}/{target}/fixtures"],"tests":["target/build/{profile}/{target}/tests/archive.tar.zst","target/build/{profile}/{target}/tests/compiler.json"]},"commands":[{"run":"mise exec -- uv run --locked --project tools/python python -m harmos_tools.ci.build --profile ${{ env.PROFILE }} --output tests --task build:rust:tests","cached":false,"shell":null,"condition":null}],"bootstrap":[],"setup":"python .github/actions/setup/run.py --platform windows","provider_setup":[],"tools":"aqua:koalaman/shellcheck,aqua:nextest-rs/nextest/cargo-nextest,github:bnjbvr/cargo-machete,go,go:github.com/rhysd/actionlint/cmd/actionlint,node,python,rust,sccache,taplo,uv","skip":null,"compiler_cache":true,"image":"managed:ubuntu-24.04","minutes":120},{"key":"check_generated_python","id":"check:python:generated","platform":"unix","independent":true,"pipelines":["pr","dev","release","nightly"],"release_review_only":false,"needs":["build_product_unix","build_product_windows"],"inputs":["build#product"],"outputs":{},"commands":[{"run":"mise exec -- uv run --locked --project tools/python python -m harmos_tools.test.archive --profile ${{ env.PROFILE }} --install-wheels","cached":false,"shell":null,"condition":null},{"run":"mise run check:python:generated","cached":false,"shell":null,"condition":null}],"bootstrap":[],"setup":"python3 .github/actions/setup/run.py --platform unix","provider_setup":[],"tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,git-cliff","skip":"build","compiler_cache":false,"image":"managed:ubuntu-24.04","minutes":120},{"key":"prepare_test_fixtures_windows","id":"prepare:tests:fixtures","platform":"windows","independent":false,"pipelines":["pr","dev","release","nightly"],"release_review_only":false,"needs":["build_product_unix","build_product_windows","build_tests_unix","build_tests_windows"],"inputs":["build#product","build:rust:tests#fixtures"],"outputs":{"fixtures":["target/build/{profile}/{target}/fixtures"]},"commands":[{"run":"mise run prepare:tests:fixtures --profile ${{ env.PROFILE }}","cached":true,"shell":null,"condition":null}],"bootstrap":[],"setup":"python .github/actions/setup/run.py --platform windows","provider_setup":[],"tools":"aqua:koalaman/shellcheck,aqua:nextest-rs/nextest/cargo-nextest,git-cliff,go,go:github.com/rhysd/actionlint/cmd/actionlint,node,python,rust,sccache,uv","skip":"build,build:rust:tests","compiler_cache":true,"image":"managed:ubuntu-24.04","minutes":120},{"key":"validate_test_rust_unit_windows","id":"test:rust:unit","platform":"windows","independent":false,"pipelines":["pr","dev","release","nightly"],"release_review_only":false,"needs":["build_tests_unix","build_tests_windows"],"inputs":["build:rust:tests#tests"],"outputs":{},"commands":[{"run":"mise run test:rust:unit --profile ${{ env.PROFILE }}","cached":false,"shell":null,"condition":null}],"bootstrap":[],"setup":"python .github/actions/setup/run.py --platform windows","provider_setup":[],"tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,git-cliff","skip":"build:rust:tests","compiler_cache":false,"image":"managed:ubuntu-24.04","minutes":120},{"key":"validate_test_rust_integration_windows","id":"test:rust:integration","platform":"windows","independent":false,"pipelines":["pr","dev","release","nightly"],"release_review_only":false,"needs":["build_tests_unix","build_tests_windows","prepare_test_fixtures_unix","prepare_test_fixtures_windows"],"inputs":["build:rust:tests#tests","prepare:tests:fixtures#fixtures"],"outputs":{},"commands":[{"run":"mise run test:rust:integration --profile ${{ env.PROFILE }}","cached":false,"shell":null,"condition":null}],"bootstrap":[],"setup":"python .github/actions/setup/run.py --platform windows","provider_setup":[],"tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,git-cliff","skip":"build:rust:tests,prepare:tests:fixtures","compiler_cache":false,"image":"managed:ubuntu-24.04","minutes":120},{"key":"validate_test_rust_acceptance_windows","id":"test:rust:acceptance","platform":"windows","independent":false,"pipelines":["pr","dev","release","nightly"],"release_review_only":false,"needs":["build_product_unix","build_product_windows","build_tests_unix","build_tests_windows","prepare_test_fixtures_unix","prepare_test_fixtures_windows"],"inputs":["build#product","build:rust:tests#tests","prepare:tests:fixtures#fixtures"],"outputs":{},"commands":[{"run":"mise run test:rust:acceptance --profile ${{ env.PROFILE }}","cached":false,"shell":null,"condition":null}],"bootstrap":[],"setup":"python .github/actions/setup/run.py --platform windows","provider_setup":[],"tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,git-cliff","skip":"build,build:rust:tests,prepare:tests:fixtures","compiler_cache":false,"image":"managed:ubuntu-24.04","minutes":120},{"key":"validate_test_rust_system_windows","id":"test:rust:system","platform":"windows","independent":false,"pipelines":["pr","dev","release","nightly"],"release_review_only":false,"needs":["build_product_unix","build_product_windows","build_tests_unix","build_tests_windows","prepare_test_fixtures_unix","prepare_test_fixtures_windows"],"inputs":["build#product","build:rust:tests#tests","prepare:tests:fixtures#fixtures"],"outputs":{},"commands":[{"run":"mise run test:rust:system --profile ${{ env.PROFILE }}","cached":false,"shell":null,"condition":null}],"bootstrap":[],"setup":"python .github/actions/setup/run.py --platform windows","provider_setup":[],"tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,git-cliff","skip":"build,build:rust:tests,prepare:tests:fixtures","compiler_cache":false,"image":"managed:ubuntu-24.04","minutes":120},{"key":"validate_test_rust_starter_windows","id":"test:rust:starter","platform":"windows","independent":false,"pipelines":["nightly"],"release_review_only":false,"needs":["build_product_unix","build_product_windows","build_tests_unix","build_tests_windows","prepare_test_fixtures_unix","prepare_test_fixtures_windows"],"inputs":["build#product","build:rust:tests#tests","prepare:tests:fixtures#fixtures"],"outputs":{},"commands":[{"run":"mise run test:rust:starter --profile ${{ env.PROFILE }}","cached":false,"shell":null,"condition":null}],"bootstrap":[],"setup":"python .github/actions/setup/run.py --platform windows","provider_setup":[],"tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,git-cliff","skip":"build,build:rust:tests,prepare:tests:fixtures","compiler_cache":false,"image":"managed:ubuntu-24.04","minutes":120},{"key":"validate_coverage_rust","id":"test:rust:coverage","platform":"unix","independent":true,"pipelines":["nightly"],"release_review_only":false,"needs":["build_product_unix","build_product_windows","build_tests_unix","build_tests_windows","prepare_test_fixtures_unix","prepare_test_fixtures_windows"],"inputs":["build#product","build:rust:tests#tests","prepare:tests:fixtures#fixtures"],"outputs":{},"commands":[{"run":"mise run test:rust:coverage --profile ${{ env.PROFILE }}","cached":false,"shell":null,"condition":null}],"bootstrap":[],"setup":"python3 .github/actions/setup/run.py --platform unix","provider_setup":[],"tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:taiki-e/cargo-llvm-cov,github:bnjbvr/cargo-machete,sccache,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,taplo","skip":"build,build:rust:tests,prepare:tests:fixtures","compiler_cache":false,"image":"managed:ubuntu-24.04","minutes":120},{"key":"build_bench","id":"build:rust:bench","platform":"unix","independent":true,"pipelines":["nightly"],"release_review_only":false,"needs":[],"inputs":[],"outputs":{"bench":["target/build/bench/{target}/bench","target/build/bench/{target}/bin","target/build/bench/{target}/fixtures"]},"commands":[{"run":"mise exec -- uv run --locked --project tools/python python -m harmos_tools.ci.build --profile bench --output bench --task build:rust:bench","cached":false,"shell":null,"condition":null}],"bootstrap":[],"setup":"python3 .github/actions/setup/run.py --platform unix","provider_setup":[],"tools":"aqua:koalaman/shellcheck,aqua:nextest-rs/nextest/cargo-nextest,github:bnjbvr/cargo-machete,go,go:github.com/rhysd/actionlint/cmd/actionlint,node,python,rust,sccache,taplo,uv","skip":null,"compiler_cache":true,"image":"managed:ubuntu-24.04","minutes":120},{"key":"bench_rust_integration","id":"bench:rust:integration","platform":"unix","independent":true,"pipelines":["nightly"],"release_review_only":false,"needs":["build_bench","check_generated_python","check_python","check_rust_unix","check_rust_windows","source","validate_coverage_rust","validate_test_rust_acceptance_unix","validate_test_rust_acceptance_windows","validate_test_rust_integration_unix","validate_test_rust_integration_windows","validate_test_rust_starter_unix","validate_test_rust_starter_windows","validate_test_rust_system_unix","validate_test_rust_system_windows","validate_test_rust_unit_unix","validate_test_rust_unit_windows"],"inputs":["build:rust:bench#bench"],"outputs":{},"commands":[{"run":"mise run bench:rust:integration --refresh-baseline","cached":false,"shell":null,"condition":null}],"bootstrap":[],"setup":"python3 .github/actions/setup/run.py --platform unix","provider_setup":[],"tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,git-cliff","skip":"build:rust:bench","compiler_cache":false,"image":"managed:ubuntu-24.04","minutes":120},{"key":"bench_rust_system","id":"bench:rust:system","platform":"unix","independent":true,"pipelines":["nightly"],"release_review_only":false,"needs":["build_bench","check_generated_python","check_python","check_rust_unix","check_rust_windows","source","validate_coverage_rust","validate_test_rust_acceptance_unix","validate_test_rust_acceptance_windows","validate_test_rust_integration_unix","validate_test_rust_integration_windows","validate_test_rust_starter_unix","validate_test_rust_starter_windows","validate_test_rust_system_unix","validate_test_rust_system_windows","validate_test_rust_unit_unix","validate_test_rust_unit_windows"],"inputs":["build:rust:bench#bench"],"outputs":{},"commands":[{"run":"mise run bench:rust:system --refresh-baseline","cached":false,"shell":null,"condition":null}],"bootstrap":[],"setup":"python3 .github/actions/setup/run.py --platform unix","provider_setup":[],"tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,git-cliff","skip":"build:rust:bench","compiler_cache":false,"image":"managed:ubuntu-24.04","minutes":120},{"key":"check_rust_unix","id":"check:rust","platform":"unix","independent":false,"pipelines":["pr","dev","release","nightly"],"release_review_only":false,"needs":[],"inputs":[],"outputs":{},"commands":[{"run":"mise run check:rust","cached":true,"shell":null,"condition":null}],"bootstrap":[],"setup":"python3 .github/actions/setup/run.py --platform unix","provider_setup":[],"tools":"aqua:koalaman/shellcheck,github:bnjbvr/cargo-machete,go,go:github.com/rhysd/actionlint/cmd/actionlint,node,python,rust,sccache,taplo,uv","skip":null,"compiler_cache":true,"image":"managed:ubuntu-24.04","minutes":120},{"key":"build_product_unix","id":"build","platform":"unix","independent":false,"pipelines":["pr","dev","release","nightly"],"release_review_only":false,"needs":[],"inputs":[],"outputs":{"product":["target/build/{profile}/{target}/bin","target/build/{profile}/{target}/lib","target/build/{profile}/{target}/product","target/generated/python/pyproject.toml","target/generated/python/src"]},"commands":[{"run":"mise exec -- uv run --locked --project tools/python python -m harmos_tools.ci.build --profile ${{ env.PROFILE }} --output product --task build","cached":false,"shell":null,"condition":null},{"run":"mise exec -- uv run --locked --project tools/python python -m harmos_tools.ci.build --profile release --output product --task build","cached":false,"shell":null,"condition":"env.PROFILE == \'dev\'"}],"bootstrap":[],"setup":"python3 .github/actions/setup/run.py --platform unix","provider_setup":[],"tools":"aqua:koalaman/shellcheck,aqua:nextest-rs/nextest/cargo-nextest,github:bnjbvr/cargo-machete,go,go:github.com/rhysd/actionlint/cmd/actionlint,node,python,rust,sccache,taplo,uv","skip":null,"compiler_cache":true,"image":"managed:ubuntu-24.04","minutes":120},{"key":"build_tests_unix","id":"build:rust:tests","platform":"unix","independent":false,"pipelines":["pr","dev","release","nightly"],"release_review_only":false,"needs":[],"inputs":[],"outputs":{"fixtures":["target/build/{profile}/{target}/fixtures"],"tests":["target/build/{profile}/{target}/tests/archive.tar.zst","target/build/{profile}/{target}/tests/compiler.json"]},"commands":[{"run":"mise exec -- uv run --locked --project tools/python python -m harmos_tools.ci.build --profile ${{ env.PROFILE }} --output tests --task build:rust:tests","cached":false,"shell":null,"condition":null}],"bootstrap":[],"setup":"python3 .github/actions/setup/run.py --platform unix","provider_setup":[],"tools":"aqua:koalaman/shellcheck,aqua:nextest-rs/nextest/cargo-nextest,github:bnjbvr/cargo-machete,go,go:github.com/rhysd/actionlint/cmd/actionlint,node,python,rust,sccache,taplo,uv","skip":null,"compiler_cache":true,"image":"managed:ubuntu-24.04","minutes":120},{"key":"prepare_test_fixtures_unix","id":"prepare:tests:fixtures","platform":"unix","independent":false,"pipelines":["pr","dev","release","nightly"],"release_review_only":false,"needs":["build_product_unix","build_product_windows","build_tests_unix","build_tests_windows"],"inputs":["build#product","build:rust:tests#fixtures"],"outputs":{"fixtures":["target/build/{profile}/{target}/fixtures"]},"commands":[{"run":"mise run prepare:tests:fixtures --profile ${{ env.PROFILE }}","cached":true,"shell":null,"condition":null}],"bootstrap":[],"setup":"python3 .github/actions/setup/run.py --platform unix","provider_setup":[],"tools":"aqua:koalaman/shellcheck,aqua:nextest-rs/nextest/cargo-nextest,git-cliff,go,go:github.com/rhysd/actionlint/cmd/actionlint,node,python,rust,sccache,uv","skip":"build,build:rust:tests","compiler_cache":true,"image":"managed:ubuntu-24.04","minutes":120},{"key":"validate_test_rust_unit_unix","id":"test:rust:unit","platform":"unix","independent":false,"pipelines":["pr","dev","release","nightly"],"release_review_only":false,"needs":["build_tests_unix","build_tests_windows"],"inputs":["build:rust:tests#tests"],"outputs":{},"commands":[{"run":"mise run test:rust:unit --profile ${{ env.PROFILE }}","cached":false,"shell":null,"condition":null}],"bootstrap":[],"setup":"python3 .github/actions/setup/run.py --platform unix","provider_setup":[],"tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,git-cliff","skip":"build:rust:tests","compiler_cache":false,"image":"managed:ubuntu-24.04","minutes":120},{"key":"validate_test_rust_integration_unix","id":"test:rust:integration","platform":"unix","independent":false,"pipelines":["pr","dev","release","nightly"],"release_review_only":false,"needs":["build_tests_unix","build_tests_windows","prepare_test_fixtures_unix","prepare_test_fixtures_windows"],"inputs":["build:rust:tests#tests","prepare:tests:fixtures#fixtures"],"outputs":{},"commands":[{"run":"mise run test:rust:integration --profile ${{ env.PROFILE }}","cached":false,"shell":null,"condition":null}],"bootstrap":[],"setup":"python3 .github/actions/setup/run.py --platform unix","provider_setup":[],"tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,git-cliff","skip":"build:rust:tests,prepare:tests:fixtures","compiler_cache":false,"image":"managed:ubuntu-24.04","minutes":120},{"key":"validate_test_rust_acceptance_unix","id":"test:rust:acceptance","platform":"unix","independent":false,"pipelines":["pr","dev","release","nightly"],"release_review_only":false,"needs":["build_product_unix","build_product_windows","build_tests_unix","build_tests_windows","prepare_test_fixtures_unix","prepare_test_fixtures_windows"],"inputs":["build#product","build:rust:tests#tests","prepare:tests:fixtures#fixtures"],"outputs":{},"commands":[{"run":"mise run test:rust:acceptance --profile ${{ env.PROFILE }}","cached":false,"shell":null,"condition":null}],"bootstrap":[],"setup":"python3 .github/actions/setup/run.py --platform unix","provider_setup":[],"tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,git-cliff","skip":"build,build:rust:tests,prepare:tests:fixtures","compiler_cache":false,"image":"managed:ubuntu-24.04","minutes":120},{"key":"validate_test_rust_system_unix","id":"test:rust:system","platform":"unix","independent":false,"pipelines":["pr","dev","release","nightly"],"release_review_only":false,"needs":["build_product_unix","build_product_windows","build_tests_unix","build_tests_windows","prepare_test_fixtures_unix","prepare_test_fixtures_windows"],"inputs":["build#product","build:rust:tests#tests","prepare:tests:fixtures#fixtures"],"outputs":{},"commands":[{"run":"mise run test:rust:system --profile ${{ env.PROFILE }}","cached":false,"shell":null,"condition":null}],"bootstrap":[],"setup":"python3 .github/actions/setup/run.py --platform unix","provider_setup":[],"tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,git-cliff","skip":"build,build:rust:tests,prepare:tests:fixtures","compiler_cache":false,"image":"managed:ubuntu-24.04","minutes":120},{"key":"validate_test_rust_starter_unix","id":"test:rust:starter","platform":"unix","independent":false,"pipelines":["nightly"],"release_review_only":false,"needs":["build_product_unix","build_product_windows","build_tests_unix","build_tests_windows","prepare_test_fixtures_unix","prepare_test_fixtures_windows"],"inputs":["build#product","build:rust:tests#tests","prepare:tests:fixtures#fixtures"],"outputs":{},"commands":[{"run":"mise run test:rust:starter --profile ${{ env.PROFILE }}","cached":false,"shell":null,"condition":null}],"bootstrap":[],"setup":"python3 .github/actions/setup/run.py --platform unix","provider_setup":[],"tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,git-cliff","skip":"build,build:rust:tests,prepare:tests:fixtures","compiler_cache":false,"image":"managed:ubuntu-24.04","minutes":120},{"key":"package_rust_unix","id":"package:rust","platform":"unix","independent":false,"pipelines":["pr","dev","release","nightly"],"release_review_only":false,"needs":["build_product_unix","build_product_windows","check_generated_python","check_python","check_rust_unix","check_rust_windows","source"],"inputs":["build#product"],"outputs":{"package":["target/package/{profile}/{target}"]},"commands":[{"run":"mise run package:rust --profile ${{ env.PROFILE }}","cached":false,"shell":null,"condition":null}],"bootstrap":[],"setup":"python3 .github/actions/setup/run.py --platform unix","provider_setup":[],"tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,git-cliff","skip":"build","compiler_cache":false,"image":"managed:ubuntu-24.04","minutes":120},{"key":"package_rust_windows","id":"package:rust","platform":"windows","independent":false,"pipelines":["pr","dev","release","nightly"],"release_review_only":false,"needs":["build_product_unix","build_product_windows","check_generated_python","check_python","check_rust_unix","check_rust_windows","source"],"inputs":["build#product"],"outputs":{"package":["target/package/{profile}/{target}"]},"commands":[{"run":"mise run package:rust --profile ${{ env.PROFILE }}","cached":false,"shell":null,"condition":null}],"bootstrap":[],"setup":"python .github/actions/setup/run.py --platform windows","provider_setup":[],"tools":"go,node,python,rust,uv,aqua:nextest-rs/nextest/cargo-nextest,aqua:koalaman/shellcheck,go:github.com/rhysd/actionlint/cmd/actionlint,git-cliff","skip":"build","compiler_cache":false,"image":"managed:ubuntu-24.04","minutes":120},{"key":"release_rehearsal","id":"release:rehearse","platform":"unix","independent":true,"pipelines":["pr"],"release_review_only":true,"needs":["build_product_unix","build_product_windows","build_tests_unix","build_tests_windows","check_generated_python","check_python","check_rust_unix","check_rust_windows","package_rust_unix","package_rust_windows","prepare_test_fixtures_unix","prepare_test_fixtures_windows","source","validate_test_rust_acceptance_unix","validate_test_rust_acceptance_windows","validate_test_rust_integration_unix","validate_test_rust_integration_windows","validate_test_rust_system_unix","validate_test_rust_system_windows","validate_test_rust_unit_unix","validate_test_rust_unit_windows"],"inputs":["package:rust#package"],"outputs":{},"commands":[{"run":"mise run release:rehearse","cached":false,"shell":null,"condition":null}],"bootstrap":[],"setup":"python3 .github/actions/setup/run.py --platform unix","provider_setup":[],"tools":"go,node,python,rust,uv,git-cliff","skip":"package:rust","compiler_cache":false,"image":"managed:ubuntu-24.04","minutes":120},{"key":"release_dry","id":"release:publish:dry","platform":"unix","independent":true,"pipelines":["pr","dev","release"],"release_review_only":false,"needs":["build_product_unix","build_product_windows","build_tests_unix","build_tests_windows","check_generated_python","check_python","check_rust_unix","check_rust_windows","package_rust_unix","package_rust_windows","prepare_test_fixtures_unix","prepare_test_fixtures_windows","source","validate_test_rust_acceptance_unix","validate_test_rust_acceptance_windows","validate_test_rust_integration_unix","validate_test_rust_integration_windows","validate_test_rust_system_unix","validate_test_rust_system_windows","validate_test_rust_unit_unix","validate_test_rust_unit_windows"],"inputs":["package:rust#package"],"outputs":{},"commands":[{"run":"mise run release:publish:dry","cached":true,"shell":null,"condition":null}],"bootstrap":[],"setup":"python3 .github/actions/setup/run.py --platform unix","provider_setup":[],"tools":"git-cliff,go,node,python,rust,sccache,uv","skip":"package:rust","compiler_cache":true,"image":"managed:ubuntu-24.04","minutes":120,"pipeline_needs":{"pr":["source","check_python","check_rust_windows","build_product_windows","build_tests_windows","check_generated_python","prepare_test_fixtures_windows","validate_test_rust_unit_windows","validate_test_rust_integration_windows","validate_test_rust_acceptance_windows","validate_test_rust_system_windows","check_rust_unix","build_product_unix","build_tests_unix","prepare_test_fixtures_unix","validate_test_rust_unit_unix","validate_test_rust_integration_unix","validate_test_rust_acceptance_unix","validate_test_rust_system_unix","package_rust_unix","package_rust_windows"],"dev":["source","check_python","check_rust_windows","build_product_windows","build_tests_windows","check_generated_python","prepare_test_fixtures_windows","validate_test_rust_unit_windows","validate_test_rust_integration_windows","validate_test_rust_acceptance_windows","validate_test_rust_system_windows","check_rust_unix","build_product_unix","build_tests_unix","prepare_test_fixtures_unix","validate_test_rust_unit_unix","validate_test_rust_integration_unix","validate_test_rust_acceptance_unix","validate_test_rust_system_unix","package_rust_unix","package_rust_windows"],"release":["source","check_python","check_rust_windows","build_product_windows","build_tests_windows","check_generated_python","prepare_test_fixtures_windows","validate_test_rust_unit_windows","validate_test_rust_integration_windows","validate_test_rust_acceptance_windows","validate_test_rust_system_windows","check_rust_unix","build_product_unix","build_tests_unix","prepare_test_fixtures_unix","validate_test_rust_unit_unix","validate_test_rust_integration_unix","validate_test_rust_acceptance_unix","validate_test_rust_system_unix","package_rust_unix","package_rust_windows"],"nightly":["source","check_python","check_rust_windows","build_product_windows","build_tests_windows","check_generated_python","prepare_test_fixtures_windows","validate_test_rust_unit_windows","validate_test_rust_integration_windows","validate_test_rust_acceptance_windows","validate_test_rust_system_windows","check_rust_unix","build_product_unix","build_tests_unix","prepare_test_fixtures_unix","validate_test_rust_unit_unix","validate_test_rust_integration_unix","validate_test_rust_acceptance_unix","validate_test_rust_system_unix","package_rust_unix","package_rust_windows"],"release-review":["source","check_python","check_rust_windows","build_product_windows","build_tests_windows","check_generated_python","prepare_test_fixtures_windows","validate_test_rust_unit_windows","validate_test_rust_integration_windows","validate_test_rust_acceptance_windows","validate_test_rust_system_windows","check_rust_unix","build_product_unix","build_tests_unix","prepare_test_fixtures_unix","validate_test_rust_unit_unix","validate_test_rust_integration_unix","validate_test_rust_acceptance_unix","validate_test_rust_system_unix","package_rust_unix","package_rust_windows"]}},{"key":"release_dry_nightly","id":"release:publish:dry","platform":"unix","independent":true,"pipelines":["nightly"],"release_review_only":false,"needs":["build_product_unix","build_product_windows","build_tests_unix","build_tests_windows","check_generated_python","check_python","check_rust_unix","check_rust_windows","package_rust_unix","package_rust_windows","prepare_test_fixtures_unix","prepare_test_fixtures_windows","source","validate_test_rust_acceptance_unix","validate_test_rust_acceptance_windows","validate_test_rust_integration_unix","validate_test_rust_integration_windows","validate_test_rust_system_unix","validate_test_rust_system_windows","validate_test_rust_unit_unix","validate_test_rust_unit_windows"],"inputs":["package:rust#package"],"outputs":{},"commands":[{"run":"mise run release:publish:dry","cached":true,"shell":null,"condition":null}],"bootstrap":[],"setup":"python3 .github/actions/setup/run.py --platform unix","provider_setup":[],"tools":"git-cliff,go,node,python,rust,sccache,uv","skip":"package:rust","compiler_cache":true,"image":"managed:ubuntu-24.04","minutes":120,"pipeline_needs":{"pr":["source","check_python","check_rust_windows","build_product_windows","build_tests_windows","check_generated_python","prepare_test_fixtures_windows","validate_test_rust_unit_windows","validate_test_rust_integration_windows","validate_test_rust_acceptance_windows","validate_test_rust_system_windows","check_rust_unix","build_product_unix","build_tests_unix","prepare_test_fixtures_unix","validate_test_rust_unit_unix","validate_test_rust_integration_unix","validate_test_rust_acceptance_unix","validate_test_rust_system_unix","package_rust_unix","package_rust_windows"],"dev":["source","check_python","check_rust_windows","build_product_windows","build_tests_windows","check_generated_python","prepare_test_fixtures_windows","validate_test_rust_unit_windows","validate_test_rust_integration_windows","validate_test_rust_acceptance_windows","validate_test_rust_system_windows","check_rust_unix","build_product_unix","build_tests_unix","prepare_test_fixtures_unix","validate_test_rust_unit_unix","validate_test_rust_integration_unix","validate_test_rust_acceptance_unix","validate_test_rust_system_unix","package_rust_unix","package_rust_windows"],"release":["source","check_python","check_rust_windows","build_product_windows","build_tests_windows","check_generated_python","prepare_test_fixtures_windows","validate_test_rust_unit_windows","validate_test_rust_integration_windows","validate_test_rust_acceptance_windows","validate_test_rust_system_windows","check_rust_unix","build_product_unix","build_tests_unix","prepare_test_fixtures_unix","validate_test_rust_unit_unix","validate_test_rust_integration_unix","validate_test_rust_acceptance_unix","validate_test_rust_system_unix","package_rust_unix","package_rust_windows"],"nightly":["source","check_python","check_rust_windows","build_product_windows","build_tests_windows","check_generated_python","prepare_test_fixtures_windows","validate_test_rust_unit_windows","validate_test_rust_integration_windows","validate_test_rust_acceptance_windows","validate_test_rust_system_windows","check_rust_unix","build_product_unix","build_tests_unix","prepare_test_fixtures_unix","validate_test_rust_unit_unix","validate_test_rust_integration_unix","validate_test_rust_acceptance_unix","validate_test_rust_system_unix","package_rust_unix","package_rust_windows"],"release-review":["source","check_python","check_rust_windows","build_product_windows","build_tests_windows","check_generated_python","prepare_test_fixtures_windows","validate_test_rust_unit_windows","validate_test_rust_integration_windows","validate_test_rust_acceptance_windows","validate_test_rust_system_windows","check_rust_unix","build_product_unix","build_tests_unix","prepare_test_fixtures_unix","validate_test_rust_unit_unix","validate_test_rust_integration_unix","validate_test_rust_acceptance_unix","validate_test_rust_system_unix","package_rust_unix","package_rust_windows"]}}],"mise_image":"ghcr.io/jdx/mise:2026.10.8-debian@sha256:8cc0b83dd86465df9ae4a813cce9ce24cb0b4f749617c419d3e4beb838b365b1","actions_root":".github/actions","impact":{"rules":[{"include":["^(?:/?|.*/)skills/.*$","^(?:/?|.*/)\\\\.agents/.*$","^(?:/?|.*/)\\\\.codex/.*$","^(?:/?|.*/)\\\\.claude/.*$","^(?:/?|.*/)AGENTS\\\\.md$","^(?:/?|.*/)CLAUDE\\\\.md$","^(?:/?|.*/)\\\\.github/copilot\\\\-instructions\\\\.md$","^(?:/?|.*/)README\\\\.md$","^(?:/?|.*/)readme\\\\.md$","^(?:/?|.*/)CONTRIBUTING\\\\.md$","^(?:/?|.*/)contributing\\\\.md$"],"exclude":[],"jobs":[]},{"include":["^LICENSE$","^LICENSE\\\\..*$","^docs(?:/|/.*/).*\\\\.md$"],"exclude":["^docs/examples/.*$","^docs/tutorials/.*$","^docs/src/.*$","^(?:/?|.*/)skills/.*$","^(?:/?|.*/)\\\\.agents/.*$","^(?:/?|.*/)\\\\.codex/.*$","^(?:/?|.*/)\\\\.claude/.*$","^(?:/?|.*/)AGENTS\\\\.md$","^(?:/?|.*/)CLAUDE\\\\.md$","^(?:/?|.*/)\\\\.github/copilot\\\\-instructions\\\\.md$","^(?:/?|.*/)README\\\\.md$","^(?:/?|.*/)readme\\\\.md$","^(?:/?|.*/)CONTRIBUTING\\\\.md$","^(?:/?|.*/)contributing\\\\.md$"],"jobs":["check:repository"]}],"dependencies":{"bench:rust:integration":["build:rust:bench","check:python","check:python:generated","check:repository","check:rust","test:rust:acceptance","test:rust:coverage","test:rust:integration","test:rust:starter","test:rust:system","test:rust:unit"],"bench:rust:system":["build:rust:bench","check:python","check:python:generated","check:repository","check:rust","test:rust:acceptance","test:rust:coverage","test:rust:integration","test:rust:starter","test:rust:system","test:rust:unit"],"bench:rust:unit":["build:rust:bench","check:python","check:python:generated","check:repository","check:rust","test:rust:acceptance","test:rust:coverage","test:rust:integration","test:rust:starter","test:rust:system","test:rust:unit"],"build":[],"build:rust:bench":[],"build:rust:tests":[],"check:python":[],"check:python:generated":["build"],"check:repository":[],"check:rust":[],"package:rust":["build","check:python","check:python:generated","check:repository","check:rust"],"prepare:tests:fixtures":["build","build:rust:tests"],"release:prepare":["bench:rust:integration","bench:rust:system","bench:rust:unit","build","build:rust:bench","build:rust:tests","check:python","check:python:generated","check:repository","check:rust","package:rust","prepare:tests:fixtures","release:publish:dry","release:rehearse","test:rust:acceptance","test:rust:coverage","test:rust:integration","test:rust:starter","test:rust:system","test:rust:unit"],"release:publish":["bench:rust:integration","bench:rust:system","bench:rust:unit","build","build:rust:bench","build:rust:tests","check:python","check:python:generated","check:repository","check:rust","package:rust","prepare:tests:fixtures","release:publish:dry","release:rehearse","test:rust:acceptance","test:rust:coverage","test:rust:integration","test:rust:starter","test:rust:system","test:rust:unit"],"release:publish:dry":["bench:rust:integration","bench:rust:system","bench:rust:unit","build","build:rust:bench","build:rust:tests","check:python","check:python:generated","check:repository","check:rust","package:rust","prepare:tests:fixtures","release:publish:dry","release:rehearse","test:rust:acceptance","test:rust:coverage","test:rust:integration","test:rust:starter","test:rust:system","test:rust:unit"],"release:rehearse":["bench:rust:integration","bench:rust:system","bench:rust:unit","build","build:rust:bench","build:rust:tests","check:python","check:python:generated","check:repository","check:rust","package:rust","prepare:tests:fixtures","release:publish:dry","release:rehearse","test:rust:acceptance","test:rust:coverage","test:rust:integration","test:rust:starter","test:rust:system","test:rust:unit"],"release:tag":["bench:rust:integration","bench:rust:system","bench:rust:unit","build","build:rust:bench","build:rust:tests","check:python","check:python:generated","check:repository","check:rust","package:rust","prepare:tests:fixtures","release:publish:dry","release:rehearse","test:rust:acceptance","test:rust:coverage","test:rust:integration","test:rust:starter","test:rust:system","test:rust:unit"],"release:verify":["bench:rust:integration","bench:rust:system","bench:rust:unit","build","build:rust:bench","build:rust:tests","check:python","check:python:generated","check:repository","check:rust","package:rust","prepare:tests:fixtures","release:publish:dry","release:rehearse","test:rust:acceptance","test:rust:coverage","test:rust:integration","test:rust:starter","test:rust:system","test:rust:unit"],"test:rust:acceptance":["build","build:rust:tests","prepare:tests:fixtures"],"test:rust:coverage":["build","build:rust:tests","prepare:tests:fixtures"],"test:rust:integration":["build:rust:tests","prepare:tests:fixtures"],"test:rust:starter":["build","build:rust:tests","prepare:tests:fixtures"],"test:rust:system":["build","build:rust:tests","prepare:tests:fixtures"],"test:rust:unit":["build:rust:tests"]}},"setup":{"windows":"python .github/actions/setup/run.py --platform windows","unix":"python3 .github/actions/setup/run.py --platform unix"}}'
)
TRANSFER_PATH = Path(__file__).resolve().parents[5] / PLAN["actions_root"] / "archive-build/run.py"
TRANSFER_SPEC = importlib.util.spec_from_file_location("artifact_transfer", TRANSFER_PATH)
if TRANSFER_SPEC is None or TRANSFER_SPEC.loader is None:
    raise RuntimeError("render the generic artifact action before simulation")
transfers = importlib.util.module_from_spec(TRANSFER_SPEC)
TRANSFER_SPEC.loader.exec_module(transfers)
PRINT = threading.Lock()
CANCEL = threading.Event()


# === Types === #

# === Classes === #


class SimulationError(RuntimeError):
    pass


class ProcessTree:
    def __init__(self, name: str | None = None):
        if sys.platform != "win32":
            raise SimulationError("native process ownership requires Windows")

        class Basic(ctypes.Structure):
            _fields_ = [
                ("process_time", ctypes.c_longlong),
                ("job_time", ctypes.c_longlong),
                ("flags", ctypes.c_ulong),
                ("minimum", ctypes.c_size_t),
                ("maximum", ctypes.c_size_t),
                ("active", ctypes.c_ulong),
                ("affinity", ctypes.c_size_t),
                ("priority", ctypes.c_ulong),
                ("scheduling", ctypes.c_ulong),
            ]

        class Counters(ctypes.Structure):
            _fields_ = [(name, ctypes.c_ulonglong) for name in ("read", "write", "other", "read_bytes", "write_bytes", "other_bytes")]

        class Limits(ctypes.Structure):
            _fields_ = [
                ("basic", Basic),
                ("io", Counters),
                ("process_memory", ctypes.c_size_t),
                ("job_memory", ctypes.c_size_t),
                ("peak_process", ctypes.c_size_t),
                ("peak_job", ctypes.c_size_t),
            ]

        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel.CreateJobObjectW.argtypes = [ctypes.c_void_p, ctypes.c_wchar_p]
        kernel.CreateJobObjectW.restype = ctypes.c_void_p
        kernel.SetInformationJobObject.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_void_p, ctypes.c_ulong]
        kernel.AssignProcessToJobObject.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
        kernel.CloseHandle.argtypes = [ctypes.c_void_p]
        self.kernel = kernel
        self.handle = kernel.CreateJobObjectW(None, name)
        if not self.handle:
            raise SimulationError("cannot create native process tree")
        limits = Limits()
        limits.basic.flags = 0x2000
        if not kernel.SetInformationJobObject(self.handle, 9, ctypes.byref(limits), ctypes.sizeof(limits)):
            self.close()
            raise SimulationError("cannot configure owned process-tree cleanup")

    def attach(self, process, workspace_owner: workspaces.Owner | None = None) -> None:
        if sys.platform != "win32":
            raise SimulationError("native process ownership requires Windows")

        if not self.kernel.AssignProcessToJobObject(self.handle, int(process._handle)):
            process.kill()
            process.wait()
            raise SimulationError("cannot attach the owned command to its owned process tree")
        if workspace_owner:
            workspace_owner.attached(process.pid)

        class Thread(ctypes.Structure):
            _fields_ = [
                ("size", ctypes.c_ulong),
                ("usage", ctypes.c_ulong),
                ("identity", ctypes.c_ulong),
                ("owner", ctypes.c_ulong),
                ("priority", ctypes.c_long),
                ("delta", ctypes.c_long),
                ("flags", ctypes.c_ulong),
            ]

        self.kernel.CreateToolhelp32Snapshot.restype = ctypes.c_void_p
        self.kernel.Thread32First.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
        self.kernel.Thread32Next.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
        self.kernel.OpenThread.restype = ctypes.c_void_p
        self.kernel.ResumeThread.argtypes = [ctypes.c_void_p]
        self.kernel.ResumeThread.restype = ctypes.c_ulong
        snapshot = self.kernel.CreateToolhelp32Snapshot(4, 0)
        resumed = False
        try:
            entry = Thread()
            entry.size = ctypes.sizeof(entry)
            present = self.kernel.Thread32First(snapshot, ctypes.byref(entry))
            while present:
                if entry.owner == process.pid:
                    thread = self.kernel.OpenThread(2, False, entry.identity)
                    if thread:
                        try:
                            resumed = self.kernel.ResumeThread(thread) != 0xFFFFFFFF
                        finally:
                            self.kernel.CloseHandle(thread)
                        break
                present = self.kernel.Thread32Next(snapshot, ctypes.byref(entry))
        finally:
            self.kernel.CloseHandle(snapshot)
        if not resumed:
            process.kill()
            process.wait()
            raise SimulationError("cannot resume the owned owned command")

    def close(self) -> None:
        if sys.platform != "win32":
            return

        if self.handle:
            self.kernel.TerminateJobObject.argtypes = [ctypes.c_void_p, ctypes.c_uint]
            try:
                if not self.kernel.TerminateJobObject(self.handle, 1):
                    raise SimulationError("cannot terminate the owned command tree")
                deadline = time.monotonic() + 5
                while workspaces.active_processes(self.kernel, self.handle):
                    if time.monotonic() >= deadline:
                        raise SimulationError("owned command tree did not settle after termination")
                    time.sleep(0.01)
            finally:
                self.kernel.CloseHandle(self.handle)
                self.handle = None


# === Public Functions === #


def repository_identity(root: Path) -> str:
    path = root
    try:
        common = subprocess.run(
            ["git", "-C", str(root), "rev-parse", "--path-format=absolute", "--git-common-dir"], capture_output=True, text=True, check=False
        )
        if common.returncode == 0:
            path = Path(common.stdout.strip())
    except FileNotFoundError:
        pass
    return hashlib.sha256(os.path.normcase(str(path.resolve())).encode()).hexdigest()[:16]


def cache_root(root: Path) -> Path:
    identity = repository_identity(root)
    if sys.platform == "win32":
        return Path.home() / ".cache/ci-simulation" / identity
    return Path(os.environ.get("LOCALAPPDATA", os.environ.get("XDG_CACHE_HOME", str(Path.home() / ".cache")))) / "ci-simulation" / identity


def repository_worktrees(root: Path) -> list[Path]:
    listed = subprocess.run(["git", "-C", str(root), "worktree", "list", "--porcelain", "-z"], capture_output=True, text=True, check=False)
    if listed.returncode:
        return [root]
    roots = [Path(entry.removeprefix("worktree ")) for entry in listed.stdout.split("\0") if entry.startswith("worktree ")]
    return list(dict.fromkeys([root, *roots]))


def recover_worktrees(root: Path, identity: str) -> None:
    for checkout in repository_worktrees(root):
        containers.reconcile(checkout / "target/ci/simulation", identity, recover_reports)
        workspaces.reconcile(checkout / "target/ci/simulation", identity, recover_workspace)


def main() -> int:
    parser = argparse.ArgumentParser(description="Simulate The Declared CI Graph")
    parser.add_argument("--pipeline", choices=("pr", "dev", "release", "nightly"), default="pr")
    parser.add_argument("--platform", action="append", choices=("windows", "unix"))
    parser.add_argument("--executor", action="append", help="platform=native|docker")
    parser.add_argument("--export", help=argparse.SUPPRESS)
    options = parser.parse_args()
    if options.export:
        transfers.export(Path.cwd(), json.loads(options.export), Path("target/ci/simulation-output.tar"))
        return 0
    started = time.monotonic()
    results = {}
    output = None
    run_owner = None
    simulation_lease = None
    impact_empty = False
    exit_code = 1
    try:
        root = next(path for path in (Path.cwd(), *Path.cwd().parents) if (path / "mise.toml").is_file())
        authorize_dispatch(root)
        cache = cache_root(root)
        identity = cache.name
        cache.mkdir(parents=True, exist_ok=True)
        lock = cache / "simulation.lock"
        if lock.is_symlink() or lock.is_junction():
            raise SimulationError("simulation lease must not be linked")
        lock.touch(exist_ok=True)
        simulation_lease = containers.lease(lock)
        simulation_lease.__enter__()
        recover_worktrees(root, identity)
        token = uuid.uuid4().hex[:12]
        run_owner = runs.owner(root / "target/ci/simulation", identity, token)
        output = run_owner.__enter__()
        platforms = options.platform or PLAN["platforms"]
        executors = {platform: "native" if platform == "windows" else "docker" for platform in platforms}
        for selection in options.executor or []:
            platform, executor = selection.split("=", 1)
            if platform not in platforms or executor not in ("native", "docker"):
                raise SimulationError("executor must select a declared platform=native|docker")
            executors[platform] = executor
        branch = subprocess.run(["git", "branch", "--show-current"], cwd=root, capture_output=True, text=True, check=False).stdout.strip()
        release_review = options.pipeline == "release" or options.pipeline == "pr" and branch.startswith("release/v")
        contexts = {options.pipeline} | ({"pr", "release"} if release_review else set())
        jobs = [
            job
            for job in PLAN["jobs"]
            if contexts.intersection(job["pipelines"])
            and (job["platform"] in platforms or job["platform"] == "any" or job.get("independent"))
            and (not job.get("release_review_only") or release_review)
        ]
        if options.pipeline in ("pr", "dev") and not release_review:
            selected = impact.selection(root, PLAN["impact"])
            impact_empty = bool(jobs) and not selected
            jobs = [job for job in jobs if job["id"] in selected]
        if set(platforms) != set(PLAN["platforms"]):
            jobs = [job for job in jobs if job["id"] not in ("release:publish:dry", "release:rehearse")]
            emit(
                "Scope",
                "Partial platform run: aggregate Release / Dry and Rehearsal are excluded; this run does not satisfy the review gate",
            )
        keys = {job["key"] for job in jobs}
        for job in jobs:
            job["needs"] = job.get("pipeline_needs", {}).get("release-review" if release_review else options.pipeline, job["needs"])
            excluded = {
                source["key"] for source in PLAN["jobs"] if source["platform"] in PLAN["platforms"] and source["platform"] not in platforms
            }
            job["needs"] = [need for need in job["needs"] if need not in excluded]
            unavailable = set(job["needs"]) - keys
            if unavailable:
                raise SimulationError(f"{job['key']}: unavailable selected prerequisites: {sorted(unavailable)}")
            platform = job["platform"] if job["platform"] in platforms else platforms[0]
            job["execution_platform"] = platform
            job["setup"] = PLAN["setup"][platform]
            job["executor"] = executors[platform]
        if not jobs:
            if not impact_empty:
                raise SimulationError("no jobs match the selected pipeline and platforms")
            emit("Scope", "Matched Impact Rules Select No Execution Jobs")
            exit_code = 0
            return exit_code
        needed = {job["execution_platform"] for job in jobs}
        preflight({platform: executor for platform, executor in executors.items() if platform in needed})
        docker_jobs = [job for job in jobs if job["executor"] == "docker"]
        if any(job["image"] == "managed:ubuntu-24.04" for job in docker_jobs):
            image = managed_image(PLAN["mise_image"], PLAN["setup"]["unix"])
            for job in docker_jobs:
                if job["image"] == "managed:ubuntu-24.04":
                    job["image"] = image
        for image in {job["image"] for job in docker_jobs}:
            probe = subprocess.run(
                ["docker", "run", "--rm", "--entrypoint", "bash", image, "-c", "command -v python3 && command -v mise && command -v git"],
                capture_output=True,
                text=True,
                check=False,
            )
            if probe.returncode:
                raise SimulationError(
                    f"Linux image {image} must provide bash, Python3, mise and git before authored bootstrap; no fallback was applied"
                )
        profile = "release" if options.pipeline in ("release", "nightly") or release_review else "dev"
        with workspaces.job(output, identity, token, "_candidate", lambda _: None) as owner:
            candidate = owner.root / "workspace"
            stage(root, candidate)

            def execute(job):
                return run_job(job, candidate, output, cache, identity, token, profile)

            results = schedule(jobs, execute, results=results, output=output)
        exit_code = 0 if all(result["state"] == "passed" for result in results.values()) else 1
        return exit_code
    except (OSError, ValueError, StopIteration, SimulationError, containers.Busy, subprocess.SubprocessError) as error:
        emit("Preflight", str(error))
        return 1
    except KeyboardInterrupt:
        exit_code = 130
        return exit_code
    finally:
        summary = summarize(results, time.monotonic() - started)
        summary["exit_code"] = exit_code
        if impact_empty:
            summary["selection"] = "explicit-empty-impact"
        statistics = summary["cache"] if isinstance(summary["cache"], dict) else {}
        reporting.render(
            "CI Simulation",
            summary["wall_seconds"],
            statistics,
            phases={
                "Critical Path": summary["critical_path_seconds"],
                "Queue Wait": sum(result["wait_seconds"] for result in results.values()),
            },
            caches={name: {"Restore": "Unavailable"} for name in ("Cargo", "UV", "PNPM", "Mise")},
        )
        for key, result in results.items():
            reporting.render(
                f"{key} / {result['state']}",
                result["seconds"],
                result.get("cache"),
                phases={"Queue Wait": result["wait_seconds"], **result.get("phases", {})},
            )
            if result["state"] == "failed":
                emit(key, f"{result.get('error', 'failure reason unavailable')}; log: {result.get('log', 'unavailable')}")
        if output:
            try:
                (output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
            finally:
                if run_owner is not None:
                    run_owner.__exit__(None, None, None)
                if simulation_lease is not None:
                    simulation_lease.__exit__(None, None, None)
        elif simulation_lease is not None:
            simulation_lease.__exit__(None, None, None)


def authorize_dispatch(root: Path) -> None:
    guard = PLAN.get("dispatch_guard")
    if not guard:
        return
    manual = (
        os.environ.get("GITHUB_EVENT_NAME") == "workflow_dispatch"
        if guard["DISPATCH_PROVIDER"] == "github"
        else os.environ.get("CI_PIPELINE_SOURCE") == "web"
    )
    if manual:
        script = root / PLAN["actions_root"] / "authorize-release/run.py"
        subprocess.run([sys.executable, str(script)], env=dict(os.environ, **guard), check=True, creationflags=spawn_flags())


def schedule(jobs: list[dict], execute, *, results: dict | None = None, output: Path | None = None) -> dict:
    pending = {job["key"]: job for job in jobs}
    results = {} if results is None else results
    queued = time.monotonic()
    completed = queue.Queue()
    running = {}
    started_jobs = {}
    unexpected = []
    # The Whole Graph Can Run At Once; No Smaller Pool Or Resource Ceiling Is Imposed.
    executor = concurrent.futures.ThreadPoolExecutor(max_workers=max(1, len(jobs)))
    CANCEL.clear()

    def run(job, wait):
        started = time.monotonic()
        started_jobs[job["key"]] = started
        try:
            result = execute(job)
        except (OSError, ValueError, SimulationError, containers.Busy, tarfile.TarError, subprocess.SubprocessError) as error:
            result = {"state": "failed", "seconds": time.monotonic() - started, "cache": None, "error": f"{type(error).__name__}: {error}"}
        result.update(wait_seconds=wait, needs=job["needs"])
        return result

    try:
        while pending or running:
            ready = [(key, job) for key, job in pending.items() if all(need in results for need in job["needs"])]
            for key, job in ready:
                del pending[key]
                wait = time.monotonic() - queued
                if any(results[need]["state"] != "passed" for need in job["needs"]):
                    results[key] = {"state": "blocked", "seconds": 0, "cache": None, "wait_seconds": wait, "needs": job["needs"]}
                    emit(key, "blocked")
                    continue
                emit(key, "running")
                future = executor.submit(run, job, wait)
                running[future] = (key, job, wait)
                future.add_done_callback(completed.put)
            if ready:
                continue
            if running:
                # Timed Queue Waits Keep SIGINT Deliverable On Native Windows.
                try:
                    future = completed.get(timeout=0.2)
                except queue.Empty:
                    continue
                key, job, wait = running.pop(future)
                error = future.exception()
                if error is None:
                    result = future.result()
                else:
                    unexpected.append(error)
                    result = {
                        "state": "failed",
                        "seconds": time.monotonic() - started_jobs[key],
                        "cache": None,
                        "error": f"{type(error).__name__}: {error}",
                        "wait_seconds": wait,
                        "needs": job["needs"],
                    }
                if result["state"] == "failed" and output is not None:
                    log = output / f"{key}.log"
                    result["log"] = str(log.resolve())
                    with log.open("a", encoding="utf-8") as diagnostic:
                        if error is not None:
                            traceback.print_exception(error, file=diagnostic)
                        else:
                            diagnostic.write(f"{result.get('error', 'failure reason unavailable')}\n")
                results[key] = result
                message = str(result["state"])
                if result["state"] == "failed":
                    message += f": {result.get('error', 'failure reason unavailable')}; log: {result.get('log', 'unavailable')}"
                emit(key, message)
            elif pending:
                raise SimulationError("selected graph has an unavailable dependency or cycle")
    finally:
        CANCEL.set()
        executor.shutdown(wait=True)
        CANCEL.clear()
        ordered = {job["key"]: results[job["key"]] for job in jobs if job["key"] in results}
        results.clear()
        results.update(ordered)
    if unexpected:
        raise unexpected[0]
    return results


def summarize(results: dict, wall: float) -> dict:
    paths = {}
    remaining = dict(results)
    while remaining:
        ready = [(key, result) for key, result in remaining.items() if all(need in paths for need in result["needs"])]
        if not ready:
            break
        for key, result in ready:
            paths[key] = result["seconds"] + max((paths[need] for need in result["needs"]), default=0)
            del remaining[key]
    caches = [result["cache"] for result in results.values() if result.get("cache") is not None]
    cache = (
        {
            name: sum(item[name] for item in caches if item.get(name) is not None)
            for name in sorted({key for item in caches for key in item})
        }
        if caches
        else "unavailable"
    )
    return {"wall_seconds": wall, "critical_path_seconds": max(paths.values(), default=0), "cache": cache, "jobs": results}


# === Private Functions === #


def emit(job: str, message: str) -> None:
    secrets = [
        value
        for key, value in os.environ.items()
        if any(word in key.upper() for word in ("TOKEN", "PASSWORD", "SECRET", "CREDENTIAL")) and len(value) >= 6
    ]
    for secret in secrets:
        message = message.replace(secret, "[redacted]")
    with PRINT:
        print(f"[{job}] {message}", flush=True)


def preflight(executors: dict) -> None:
    checked_docker = False
    for platform, executor in executors.items():
        if executor == "native":
            if platform != "windows" or os.name != "nt" or not shutil.which("mise") or not shutil.which("pwsh"):
                raise SimulationError(
                    f"required native {platform} executor unavailable; install mise and its native shell or select an available executor"
                )
        elif platform != "unix":
            raise SimulationError("Docker supports Linux only; a native Windows host is required for Windows")
        elif not checked_docker:
            command = subprocess.run(["docker", "info", "--format", "{" + "{json .}" + "}"], capture_output=True, text=True, check=True)
            if json.loads(command.stdout).get("OSType") != "linux":
                raise SimulationError("required Docker Linux engine unavailable; start Docker Desktop with its WSL Linux backend")
            checked_docker = True


def stage(root: Path, destination: Path) -> None:
    git = subprocess.run(["git", "-C", str(root), "rev-parse", "--show-toplevel"], capture_output=True, text=True, check=False)
    if git.returncode == 0:
        subprocess.run(["git", "clone", "--quiet", "--no-checkout", "--no-hardlinks", "--local", str(root), str(destination)], check=True)
        for key, default in (("core.autocrlf", "false"), ("core.eol", "native")):
            setting = subprocess.run(["git", "-C", str(root), "config", "--get", key], capture_output=True, text=True, check=False)
            if setting.returncode not in (0, 1):
                raise SimulationError(f"could not read candidate Git setting: {key}")
            value = setting.stdout.strip() if setting.returncode == 0 else default
            subprocess.run(["git", "-C", str(destination), "config", "--local", key, value], check=True)
        index = subprocess.run(["git", "-C", str(root), "ls-files", "--stage", "-z"], capture_output=True, check=True).stdout
        subprocess.run(["git", "-C", str(destination), "read-tree", "--empty"], check=True)
        subprocess.run(["git", "-C", str(destination), "update-index", "-z", "--index-info"], input=index, check=True)
        listed = subprocess.run(
            ["git", "-C", str(root), "ls-files", "-z", "--cached", "--others", "--exclude-standard"], capture_output=True, check=True
        )
        files = [Path(os.fsdecode(name)) for name in listed.stdout.split(b"\0") if name]
    else:
        destination.mkdir()
        files = [
            path.relative_to(root)
            for path in root.rglob("*")
            if path.is_file() and not set(path.relative_to(root).parts) & {"target", ".cache", ".git", ".venv", "node_modules", ".env"}
        ]
    for relative in files:
        source = root / relative
        if not source.is_file() and not source.is_symlink():
            continue
        if not source.resolve().is_relative_to(root.resolve()):
            raise SimulationError(f"candidate file escapes repository: {relative}")
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        if source.is_symlink():
            target.symlink_to(os.readlink(source))
        else:
            shutil.copy2(source, target)


def source_archive(workspace: Path, archive: Path, modes: dict) -> dict:
    indexed = subprocess.run(["git", "ls-files", "--stage", "-z"], cwd=workspace, capture_output=True, check=False)
    tracked = {}
    if indexed.returncode == 0:
        for entry in indexed.stdout.split(b"\0"):
            if entry:
                metadata, name = entry.split(b"\t", 1)
                tracked[os.fsdecode(name)] = int(metadata.split()[0], 8)
    records = {}
    with tarfile.open(archive, "w") as target:
        for path in workspace.rglob("*"):
            relative = path.relative_to(workspace).as_posix()
            member = target.gettarinfo(str(path), arcname=relative)
            if member.issym() or tracked.get(relative) == 0o120000:
                link = os.readlink(path) if path.is_symlink() else path.read_text(encoding="utf-8")
                if not (path.parent / link).resolve().is_relative_to(workspace.resolve()):
                    raise SimulationError("candidate symlink escapes repository")
                member.type, member.linkname, member.size = tarfile.SYMTYPE, link, 0
            member.mode = (
                modes.get(relative, tracked.get(relative, (0o755 if path.is_dir() else 0o644) if os.name == "nt" else member.mode)) & 0o777
            )
            if member.isfile():
                with path.open("rb") as source:
                    target.addfile(member, source)
            elif member.isdir() or member.issym():
                target.addfile(member)
            else:
                raise SimulationError("unsupported candidate file type")
            records[relative] = {
                "type": "file" if member.isfile() else "directory" if member.isdir() else "symlink",
                "mode": member.mode,
                "origin": "artifact"
                if relative in modes
                else "git"
                if relative in tracked
                else "windows-default"
                if os.name == "nt"
                else "filesystem",
            }
    return records


def run_job(job: dict, candidate: Path, output: Path, cache: Path, identity: str, token: str, profile: str) -> dict:
    started = time.monotonic()
    log = output / f"{job['key']}.log"
    try:
        log.touch(exist_ok=True)
        with runs.borrow(output, required=False):
            result = _run_job(job, candidate, output, cache, identity, token, profile)
    except (OSError, ValueError, SimulationError, containers.Busy, tarfile.TarError, subprocess.SubprocessError) as error:
        result = {"state": "failed", "seconds": time.monotonic() - started, "cache": None, "error": f"{type(error).__name__}: {error}"}
        if isinstance(error, subprocess.CalledProcessError):
            result.update(exit_code=error.returncode, command=error.cmd)
        try:
            with log.open("a", encoding="utf-8") as diagnostic:
                traceback.print_exc(file=diagnostic)
        except OSError as log_error:
            result["error"] += f"; cannot preserve log: {log_error}"
    result["log"] = str(log.resolve())
    return result


def _run_job(job: dict, candidate: Path, output: Path, cache: Path, identity: str, token: str, profile: str) -> dict:
    started = time.monotonic()
    key = job["key"]
    result: dict = {"state": "failed", "seconds": 0, "cache": None}
    with workspaces.job(output, identity, token, key, lambda workspace: recover_workspace(output, key, workspace), stable=True) as owner:
        workspace = owner.root / "workspace"
        shutil.copytree(candidate, workspace, symlinks=True)
        transferred_modes = {}
        transfer_started = time.monotonic()
        for reference in job["inputs"]:
            producer, name = reference.split("#", 1)
            candidates = [
                source
                for source in PLAN["jobs"]
                if source["id"] == producer
                and source["key"] in job["needs"]
                and (source["platform"] in ("any", job["platform"]) or job["id"].startswith("release:"))
            ]
            if not candidates:
                raise SimulationError(f"missing artifact producer {reference}")
            for source in candidates:
                artifact = output / "artifacts" / source["key"] / name
                if not artifact.is_dir():
                    raise SimulationError(f"missing input {reference}")
                shutil.copytree(artifact, workspace, dirs_exist_ok=True)
                metadata = artifact.parent.with_name(artifact.parent.name + ".modes.json")
                for relative, mode in transfers.artifact_modes(metadata).items():
                    if relative.startswith(name + "/"):
                        transferred_modes[relative.removeprefix(name + "/")] = mode
        transfer_seconds = time.monotonic() - transfer_started
        native = job.get("execution_platform", job["platform"] if job["platform"] != "any" else "unix")
        label = f"{native}/{key}"
        target = f"{'linux' if native == 'unix' else native}-x86_64" if job["platform"] != "any" else "any"
        graph_profile = "bench" if job["id"] == "build:rust:bench" or job["id"].startswith("bench:") else profile

        def replace(text):
            return (
                text.replace("$" + "{" + "{ env.PROFILE }" + "}", graph_profile)
                .replace("$env:PROFILE", graph_profile)
                .replace("$PROFILE", graph_profile)
            )

        outputs = {
            name: [path.replace("{profile}", graph_profile).replace("{target}", target) for path in patterns]
            for name, patterns in job["outputs"].items()
        }
        environment = {
            name: value
            for name, value in os.environ.items()
            if not any(word in name.upper() for word in ("TOKEN", "PASSWORD", "SECRET", "CREDENTIAL"))
            and name not in ("UV_PROJECT_ENVIRONMENT", "RUSTC_WRAPPER", "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_TARGET_DIR")
            and not name.startswith(("COMPILER_CACHE_", "SCCACHE_", "GITHUB_", "CI_", "CARGO_PROFILE_"))
        }
        environment.update(
            PROFILE=graph_profile,
            RELEASE_PROFILE=graph_profile,
            CARGO_INCREMENTAL="0",
            MISE_YES="1",
            MISE_LOCKED="1",
            MISE_EXPERIMENTAL="1",
            GIT_TERMINAL_PROMPT="0",
            CI_SIMULATE="1",
        )
        if job.get("compiler_cache"):
            environment.update(COMPILER_CACHE_BACKEND="disk", COMPILER_CACHE_MODE="READ_WRITE")
        if job.get("tools"):
            environment["MISE_ENABLE_TOOLS"] = job["tools"]
        if job.get("skip"):
            environment["MISE_TASK_SKIP"] = job["skip"]
        commands = [command for command in (job["setup"], *job["bootstrap"], "mise trust --yes", "mise install --yes") if command.strip()]
        commands.extend(job.get("provider_setup", []))
        for step in job["commands"]:
            if step.get("condition") == "env.PROFILE == 'dev'" and graph_profile != "dev":
                continue
            command = replace(step["run"])
            if step.get("cached"):
                shell = step.get("shell") or ("pwsh" if native == "windows" else "bash")
                command = f"mise exec -- uv run --locked --project tools/python python -m harmos_tools.ci.build --internal-script {command.encode().hex()} --shell {shell}"
            commands.append(command)
        with (output / f"{key}.log").open("w", encoding="utf-8") as log:
            if job["executor"] == "docker":
                status = docker(job, workspace, output, identity, token, environment, commands, outputs, log, result, transferred_modes)
            else:
                local = cache / native
                local.mkdir(parents=True, exist_ok=True)
                environment.update(
                    MISE_DATA_DIR=str(local / "mise"),
                    MISE_CACHE_DIR=str(local / "mise-cache"),
                    MISE_DOWNLOADS_DIR=str(local / "mise-downloads"),
                    CARGO_HOME=str(local / "cargo"),
                    RUSTUP_HOME=str(local / "rustup"),
                    MISE_RUSTUP_HOME=str(local / "rustup"),
                    UV_CACHE_DIR=str(local / "uv"),
                    pnpm_config_store_dir=str(local / "pnpm"),
                    SCCACHE_DIR=str(cache.parent / "compiler" / native),
                )
                evidence(output, job, commands, environment)
                windows = os.name == "nt"
                script = workspace.with_suffix(".ps1" if windows else ".sh")
                for name, suffix in [("GITHUB_ENV", ".env"), ("GITHUB_PATH", ".path")]:
                    scratch = workspace.with_suffix(suffix)
                    scratch.write_text("", encoding="utf-8")
                    environment[name] = str(scratch)
                lines = ["$ErrorActionPreference='Stop'" if windows else "set -eo pipefail"]
                for index, command in enumerate(commands):
                    step_label = f"Command {index + 1}/{len(commands)}: {command.splitlines()[0]}".replace(
                        "'", "''" if windows else "'\"'\"'"
                    )
                    if windows:
                        lines.extend(
                            [
                                f"Write-Output '{step_label}'",
                                "$global:LASTEXITCODE=0",
                                command,
                                "if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }",
                            ]
                        )
                    else:
                        lines.extend([f"printf '%s\\n' '{step_label}'", command])
                    lines.extend(step_environment.commands(windows, sys.executable, str(Path(__file__).with_name("environment.py"))))
                script.write_text("\n".join(lines) + "\n", encoding="utf-8")
                shell = ["pwsh", "-NoProfile", "-NonInteractive", "-File", str(script)] if windows else ["bash", str(script)]
                status = stream(
                    shell, workspace, environment, label, log, job["minutes"] * 60 - (time.monotonic() - started), result, owner
                )
                if status == 0:
                    transfer_started = time.monotonic()
                    archive = workspace / "target/ci/simulation-output.tar"
                    transfers.export(workspace, outputs, archive)
                    transfers.transport(archive, output / "artifacts" / key)
                    transfer_seconds += time.monotonic() - transfer_started
        result.setdefault("phases", {})["Transfer"] = transfer_seconds
        result.update(state="passed" if status == 0 else "failed", seconds=time.monotonic() - started)
        if status:
            result["exit_code"] = status
            result["error"] = f"{result.get('command', 'job command')} exited with code {status}"
    return result


def docker(
    job: dict,
    workspace: Path,
    output: Path,
    identity: str,
    token: str,
    environment: dict,
    commands: list[str],
    outputs: dict,
    log,
    result: dict,
    modes: dict | None = None,
) -> int:
    with containers.job(
        output, identity, token, job["key"], lambda container: preserve_reports(workspace, output, job["key"], log, container)
    ) as owner:
        name = owner.record["container"]
        volume = owner.record["volume"]
        image_identity = hashlib.sha256(job["image"].encode()).hexdigest()[:12]
        cache = f"ci-{identity}-unix-{image_identity}-cache"
        compiler_cache = "ci-compiler-unix"
        script = workspace.with_suffix(".sh")
        lines = ["set -eo pipefail"]
        if job["image"].startswith("ci-ubuntu24:"):
            lines.extend(
                [
                    'sudo -n chown -R "$(id -u):$(id -g)" /workspace',
                    'sudo -n chown "$(id -u):$(id -g)" /cache /compiler-cache',
                ]
            )
        lines.extend(
            [
                "mkdir -p /workspace/target/ci",
                ': > "$GITHUB_ENV"; : > "$GITHUB_PATH"',
            ]
        )
        for index, command in enumerate(commands):
            label = f"Command {index + 1}/{len(commands)}: {command.splitlines()[0]}".replace("'", "'\"'\"'")
            lines.extend([f"printf '%s\\n' '{label}'", command])
            lines.extend(step_environment.commands(False, "python3", "/ci-environment.py"))

        export_argument = json.dumps(outputs).replace("'", "'\"'\"'")
        lines.append(
            f"mise exec -- uv run --locked --project tools/python python -m harmos_tools.ci.simulate --export '{export_argument}'",
        )
        script.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
        child = {
            key: value
            for key, value in environment.items()
            if key
            in (
                "PROFILE",
                "RELEASE_PROFILE",
                "CARGO_INCREMENTAL",
                "MISE_YES",
                "MISE_LOCKED",
                "MISE_EXPERIMENTAL",
                "MISE_ENABLE_TOOLS",
                "MISE_TASK_SKIP",
                "CI_SIMULATE",
                "COMPILER_CACHE_BACKEND",
                "COMPILER_CACHE_MODE",
            )
        }
        child.update(
            MISE_DATA_DIR="/cache/mise",
            MISE_CACHE_DIR="/cache/mise-cache",
            MISE_DOWNLOADS_DIR="/cache/downloads",
            CARGO_HOME="/cache/cargo",
            RUSTUP_HOME="/cache/rustup",
            UV_CACHE_DIR="/cache/uv",
            pnpm_config_store_dir="/cache/pnpm",
            SCCACHE_DIR="/compiler-cache",
            GITHUB_ENV="/workspace/target/ci/step.env",
            GITHUB_PATH="/workspace/target/ci/step.path",
        )
        evidence(output, job, commands, child)
        create = [
            "docker",
            "create",
            "--name",
            name,
            "--mount",
            f"type=volume,source={volume},target=/workspace",
            "--mount",
            f"type=volume,source={compiler_cache},target=/compiler-cache",
            "--mount",
            f"type=volume,source={cache},target=/cache",
            "--workdir",
            "/workspace",
        ]
        for key, value in child.items():
            create += ["--env", f"{key}={value}"]
        create += [job["image"], "bash", "/ci-simulation.sh"]
        owner.create(create)
        archive = workspace.with_suffix(".tar")
        records = source_archive(workspace, archive, modes or {})
        (output / f"{job['key']}.source-modes.json").write_text(json.dumps(records), encoding="utf-8")
        with archive.open("rb") as source:
            subprocess.run(["docker", "cp", "-", name + ":/workspace"], stdin=source, check=True, capture_output=True)
        archive.unlink()
        subprocess.run(["docker", "cp", str(script), name + ":/ci-simulation.sh"], check=True, capture_output=True)
        subprocess.run(
            ["docker", "cp", str(Path(__file__).with_name("environment.py")), name + ":/ci-environment.py"], check=True, capture_output=True
        )
        status = stream(["docker", "start", "--attach", name], workspace, None, f"unix/{job['key']}", log, job["minutes"] * 60, result)
        if status == 0:
            archive = workspace / "simulation-output.tar"
            subprocess.run(
                ["docker", "cp", name + ":/workspace/target/ci/simulation-output.tar", str(archive)], check=True, capture_output=True
            )
            transfers.transport(archive, output / "artifacts" / job["key"])
        return status


def recover_workspace(output: Path, key: str, workspace: Path) -> None:
    with (output / f"{key}.log").open("a", encoding="utf-8") as log:
        if not preserve_reports(workspace, output, key, log):
            raise SimulationError("workspace reports remain available for recovery")


def recover_reports(output: Path, key: str, container: str) -> None:
    with (
        tempfile.TemporaryDirectory(prefix="ci-recovery-") as directory,
        (output / f"{key}.cleanup.log").open("a", encoding="utf-8") as log,
    ):
        if not preserve_reports(Path(directory) / "workspace", output, key, log, container):
            raise SimulationError("Docker reports remain available for recovery")


def preserve_reports(workspace: Path, output: Path, key: str, log, container: str | None = None) -> bool:
    archive = workspace.with_suffix(".reports.tar")
    for relative, destination in (("target/tests", ""), (".stkr/benchmarks", ".stkr"), (".stkr/traces", ".stkr")):
        try:
            if container:
                with archive.open("wb") as target:
                    subprocess.run(
                        ["docker", "cp", container + ":/workspace/" + relative, "-"],
                        stdout=target,
                        stderr=subprocess.PIPE,
                        check=True,
                        timeout=30,
                    )
            else:
                reports = workspace / relative
                if not reports.exists():
                    continue
                with tarfile.open(archive, "w") as target:
                    for path in chain((reports,), reports.rglob("*")):
                        if path.is_symlink() or path.is_junction() or not path.resolve().is_relative_to(reports.resolve()):
                            raise SimulationError("diagnostic escapes native reports")
                        if path.is_file() or path.is_dir():
                            target.add(path, arcname=path.relative_to(reports.parent).as_posix(), recursive=False)
            transfers.transport(archive, output / "diagnostics" / key / destination)
        except (OSError, tarfile.TarError, subprocess.SubprocessError, SimulationError) as error:
            missing = f"Error response from daemon: Could not find the file /workspace/{relative} in container {container}"
            if (
                container
                and isinstance(error, subprocess.CalledProcessError)
                and error.returncode == 1
                and error.stderr == (missing + "\n").encode()
            ):
                continue
            try:
                log.write(f"Diagnostic collection unavailable: {error}\n")
                log.flush()
            except OSError:
                pass
            return False
    return True


def evidence(output: Path, job: dict, commands: list[str], environment: dict) -> None:
    selected = {
        name: value
        for name, value in environment.items()
        if name
        in (
            "PROFILE",
            "RELEASE_PROFILE",
            "CARGO_INCREMENTAL",
            "CARGO_HOME",
            "RUSTUP_HOME",
            "UV_CACHE_DIR",
            "pnpm_config_store_dir",
        )
        or name in ("SCCACHE_DIR", "SCCACHE_BUCKET", "SCCACHE_REGION", "SCCACHE_ENDPOINT", "COMPILER_CACHE_BACKEND", "COMPILER_CACHE_MODE")
        or name.startswith(("MISE_", "CI_SIMULATE"))
    }
    recorded = {"job": job, "commands": commands, "environment": selected}
    (output / f"{job['key']}.execution.json").write_text(json.dumps(recorded, indent=2) + "\n", encoding="utf-8")


def stream(
    command: list[str],
    cwd: Path,
    environment: dict | None,
    key: str,
    log,
    timeout: float,
    result: dict,
    workspace_owner: workspaces.Owner | None = None,
) -> int:
    result["command"] = subprocess.list2cmdline(command)
    if timeout <= 0:
        raise SimulationError("job deadline exceeded")
    flags = spawn_flags()
    name = workspace_owner.starting() if workspace_owner else None
    owner = ProcessTree(name) if os.name == "nt" and command[0] != "docker" else None
    if owner:
        flags |= 4
    try:
        process = subprocess.Popen(
            command,
            cwd=cwd,
            env=environment,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            errors="replace",
            creationflags=flags,
            start_new_session=os.name != "nt",
        )
    except BaseException as error:
        if owner:
            owner.close()
        if workspace_owner and isinstance(error, FileNotFoundError):
            workspace_owner.settled()
        raise
    try:
        if owner:
            owner.attach(process, workspace_owner)
        elif workspace_owner:
            workspace_owner.attached(process.pid)
    except BaseException:
        if owner:
            owner.close()
        elif os.name == "nt":
            process.kill()
            process.wait()
        else:
            import signal

            os.killpg(process.pid, signal.SIGKILL)
            process.wait()
        raise

    def drain():
        assert process.stdout is not None
        for line in process.stdout:
            for secret_name, secret in os.environ.items():
                if any(word in secret_name.upper() for word in ("TOKEN", "PASSWORD", "SECRET", "CREDENTIAL")) and len(secret) >= 6:
                    line = line.replace(secret, "[redacted]")
            log.write(line)
            log.flush()
            emit(key, line.rstrip())
            if line.startswith("Command "):
                result["command"] = line.strip()
            if line.startswith("BUILD_STATISTICS "):
                information = json.loads(line.removeprefix("BUILD_STATISTICS "))
                metrics = {name: value for name, value in information["statistics"].items() if type(value) is int}
                result["phases"] = information["phases"]
                if metrics:
                    result["cache"] = (
                        {name: result.get("cache", {}).get(name, 0) + value for name, value in metrics.items()}
                        if result.get("cache")
                        else metrics
                    )

    reader = threading.Thread(target=drain, daemon=True)
    reader.start()
    try:
        deadline = time.monotonic() + timeout
        while process.poll() is None:
            if CANCEL.is_set():
                raise SimulationError("simulation cancelled")
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise subprocess.TimeoutExpired(command, timeout)
            try:
                return process.wait(timeout=min(0.2, remaining))
            except subprocess.TimeoutExpired:
                continue
        return process.returncode
    except (subprocess.TimeoutExpired, KeyboardInterrupt) as error:
        if os.name == "nt":
            subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"], capture_output=True, check=False)
        else:
            import signal

            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
        process.wait()
        if isinstance(error, KeyboardInterrupt):
            raise
        raise SimulationError("job deadline exceeded") from None
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()
        if owner:
            owner.close()
        elif os.name != "nt":
            import signal

            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
        if workspace_owner:
            deadline = time.monotonic() + 5
            while workspaces.command_live(workspace_owner.record["command"]):
                if time.monotonic() >= deadline:
                    raise SimulationError("owned command tree did not settle after termination")
                time.sleep(0.01)
            workspace_owner.settled()
        reader.join(timeout=5)
        if reader.is_alive():
            raise SimulationError("owned command retained its output stream after termination")


def managed_image(mise_image: str, setup: str) -> str:
    recipe = f"""FROM {mise_image} AS mise
FROM ubuntu:24.04@sha256:68ee22004a9109b44973313b02ff7b1de250f1de0f5ecb628d73f579aaff33ac
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends python3 git gh curl ca-certificates sudo && rm -rf /var/lib/apt/lists/*
COPY --from=mise /usr/local/bin/mise /usr/local/bin/mise
COPY setup.py /opt/ci/setup.py
RUN python3 /opt/ci/setup.py --platform unix && rm -rf /var/lib/apt/lists/*
RUN groupadd --gid 1001 runner && useradd --uid 1001 --gid 1001 --create-home --shell /bin/bash runner && echo 'runner ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/runner && chmod 0440 /etc/sudoers.d/runner
ENV HOME=/home/runner
USER runner
ENTRYPOINT []
"""
    source = (Path(__file__).resolve().parents[5] / PLAN["actions_root"] / "setup/run.py").read_text(encoding="utf-8")
    fingerprint = hashlib.sha256((recipe + source + setup).encode()).hexdigest()
    image = f"ci-ubuntu24:{fingerprint}"
    if subprocess.run(["docker", "image", "inspect", image], capture_output=True, check=False).returncode == 0:
        return image
    print(f"Preparing managed Ubuntu 24.04 executor image {image}", flush=True)
    with tempfile.TemporaryDirectory(prefix="ci-image-") as temporary:
        context = Path(temporary)
        (context / "Dockerfile").write_text(recipe, encoding="utf-8", newline="\n")
        (context / "setup.py").write_text(source, encoding="utf-8", newline="\n")
        subprocess.run(["docker", "build", "--tag", image, str(context)], check=True)
    return image


# === Entry Point === #

if __name__ == "__main__":
    raise SystemExit(main())
