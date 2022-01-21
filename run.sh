#!/bin/bash

PI_IP=192.168.0.151
TARGET=arm-unknown-linux-gnueabihf # Pi 0/1

# build binary
case $1 in
  deploy)
    cross build --release --target $TARGET;
    scp ./target/$TARGET/debug/gpio2mqtt pi@$PI_IP:/home/pi;

    # copy binary to /usr/local/bin
    ssh pi@$PI_IP 'sudo install /home/pi/gpio2mqtt /usr/local/bin'

    # copy config to /etc
    scp ./gpio2mqtt.yaml pi@$PI_IP:/home/pi/gpio2mqtt.yaml
    ssh pi@$PI_IP 'sudo install /home/pi/gpio2mqtt.yaml /etc'

    # copy service to /etc/systemd/system
    scp ./gpio2mqtt.service pi@$PI_IP:/home/pi/gpio2mqtt.service
    ssh pi@$PI_IP 'sudo install /home/pi/gpio2mqtt.service /etc/systemd/system'
    ;;
  test)
    cross test --target=$TARGET --no-run --all

    if [[ $? != 0 ]]; then
      exit 1
    fi

    exec=$(cross test --target $TARGET --no-run --all -q --message-format=json | jq -r 'select(.reason == "compiler-artifact" and .target.name == "gpio2mqtt") | .executable' | tail -n1);

    scp ".$exec" pi@$PI_IP:/home/pi/gpio2mqtt;
    ;;
esac



