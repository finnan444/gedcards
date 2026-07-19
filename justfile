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
    cargo sort -c
    cargo machete
    cargo deny --log-level error check advisories

# Autofix formatting and the clippy lints that can be fixed
fix:
    cargo clippy --fix --allow-dirty --allow-staged --all-targets
    cargo fmt --all
    cargo sort
    cargo machete --fix

# Run before committing
check: lint test

# Install the dev tools `just lint` depends on
install-tools:
    cargo install --locked cargo-sort
    cargo install --locked cargo-machete
    cargo install --locked cargo-deny

# Install the lefthook git hooks (pre-push runs `just check`)
install-hooks:
    lefthook install
