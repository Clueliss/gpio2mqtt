# gpio2mqtt

A server application to expose obscure devices to homeassistant via MQTT.
Not many devices are currently supported as I basically use it exclusively for my own purposes
and only need the implemented ones.

Currently supported:
- Covers connected to GPIO via UP,DOWN and STOP pins (you can for example solder wires to a VELUX integra remote to use it with this)
- VARTA Element Energy Storages via Sunspec-Modbus (probably also some others that are similar enough)

Example config defining one cover and one sunspec device:
```yaml
broker: 192.168.0.20
client_id: gpio2mqtt_bridge
covers:
    -   group_gpio_pause_ms: 1000
        devices:
            -   name: Cover 1
                chip: /dev/gpiochip0
                up_pin: 2
                down_pin: 4
                stop_pin: 3
                device_gpio_pause_ms: 300
                device:
                    identifier: velux_integra_1
                    manufacturer: VELUX
                    model: INTEGRA
sunspec:
    -   name: Varta Element
        host: 192.168.0.52
        device_polling_delay_ms: 1000
        device:
            identifier: varta_element_1
            manufacturer: Varta
            model: Element
```
