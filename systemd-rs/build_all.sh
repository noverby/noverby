#! /bin/sh
cargo build --workspace
# cargo build --workspace --features linux_eventfd
# cargo build --workspace --target x86_64-unknown-redox

cd test_c_service
gcc -o test_service test_service.c $(pkg-config libsystemd --libs)
