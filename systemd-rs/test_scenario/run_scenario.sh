#! /bin/bash
# This should print the output of the services ordered by the first number
../target/debug/systemd-rs 2> /dev/null | grep ".service]" &

sleep 5
killall systemd-rs
sleep 1
