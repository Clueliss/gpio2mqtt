# gpio2mqtt

A server application to expose obscure devices to homeassistant via MQTT.
Not many devices are currently supported as I basically use it exclusively for my own purposes
and only use the ones that are implemented.

Currently supported:
- Covers connected to GPIO via seperate `Up`, `Down` and `Stop` pins (you can for example solder wires to a VELUX Integra remote, see below)
- VARTA Element Energy Storages via Sunspec-Modbus (probably also some others that are similar enough)


## Example Config
Example config defining one cover and one sunspec device:
```yaml
broker: 192.168.1.20
client_id: gpio2mqtt_bridge
covers:
    -   group_gpio_pause_ms: 1000
        devices:
            -   name: Velux 1
                chip: /dev/gpiochip0
                up_pin: 2
                stop_pin: 3
                down_pin: 4
                device_gpio_pause_ms: 300
                device:
                    identifier: velux_integra_1
                    manufacturer: VELUX
                    model: INTEGRA
            -   name: Velux 2
                chip: /dev/gpiochip0
                up_pin: 7
                stop_pin: 8
                down_pin: 25
                device_gpio_pause_ms: 300
                device:
                    identifier: velux_integra_2
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


## Example Hardware Setup for two Velux Integra Covers
### Required Components
- 2 Velux Remotes
- 2 Quad Bilateral Switches (e.g. [this](https://www.reichelt.de/analog-schalter-ic-4-kanal-dil-14-74hc-4066-p3234.html?&nbc=1))
- 1 Raspberry Pi 1 B+

### Steps
1. Dismantle the Velux remotes
2. Solder wires to the power and signal pads on the internal PCB <br/>
    <img src="https://github.com/Clueliss/gpio2mqtt/assets/31625940/b7d45670-563f-402a-9299-7917b08a9d76" width="25%"/>
  
3. Connect all components as shown by the schematic (the GPIO pins can free freely chosen, this just needs an adjustment the config shown above)
  ![grafik](https://github.com/Clueliss/gpio2mqtt/assets/31625940/3f26f3fe-db96-4ecd-b2e1-b03d307f5ed2)
