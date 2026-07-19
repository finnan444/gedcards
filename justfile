# List available recipes
default:
    @just --list

# Build the gedc binary
build:
    cargo build

# Run the test suite; extra args go to cargo test
test *args='':
    cargo test {{args}}

# Reject what CI would reject
lint:
    cargo fmt --all --check
    cargo clippy --all-targets -- -D warnings

# Autofix formatting and the clippy lints that can be fixed
fix:
    cargo clippy --fix --allow-dirty --allow-staged --all-targets
    cargo fmt --all

# Run before committing
check: lint test
