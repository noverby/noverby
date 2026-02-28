build:
    cargo build --workspace

test:
    cargo test --workspace

clippy:
    cargo clippy --workspace

# Run differential tests comparing systemd-rs against real systemd
difftest *ARGS:
    cargo test --package difftest -- {{ARGS}}

# Run differential tests with full report output
difftest-report:
    DIFFTEST_JUNIT_PATH=result/difftest-results.xml \
    DIFFTEST_JSON_PATH=result/difftest-results.json \
    DIFFTEST_MARKDOWN_PATH=result/difftest-results.md \
    cargo test --package difftest -- --nocapture

# Update differential test snapshots (approve current outputs as golden)
difftest-update-snapshots:
    DIFFTEST_UPDATE_SNAPSHOTS=1 cargo test --package difftest

# List all registered differential tests
difftest-list:
    cargo test --package difftest -- --list
