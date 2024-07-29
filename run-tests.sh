#!/bin/bash

set -e

cargo test

./yosys-plugin/run_tests.sh

lit -v tests/integration_tests
