# Reflow Oven

Toaster oven conversion to a reflow soldering oven with a custom controller PCB.

## Goals

- Reliable lead-free reflow profiles (peak ~245°C)
- Thermocouple-based closed-loop temperature control
- Programmable profiles (preheat → soak → reflow → cooling)
- Safe operation (over-temperature protection, door interlock)

## Architecture

```
[Thermocouple] → [Controller PCB] → [SSR] → [Heating elements]
                       ↓
                  [Display/UI]
```

## Components

| Component | Role |
|-----------|------|
| Toaster oven | Heating chamber (TBD model) |
| Controller PCB | Custom — profile management, PID control |
| SSR | Switches mains to heating elements |
| K-type thermocouple | Temperature sensing inside chamber |
| MAX31855 or MAX6675 | Thermocouple-to-SPI interface |

## Status

🚧 **Planning phase** — selecting oven, defining controller requirements.

## Open Questions

- [ ] Which toaster oven? (size must fit Granit PCB: 92 × 99.5mm)
- [ ] MCU choice: RP2040, ESP32, or STM32?
- [ ] UI: OLED display + buttons, or web interface (ESP32)?
- [ ] Single or dual zone heating?

## Related

- [Granit project](https://github.com/laenzlinger/granit) — the PCB this oven will reflow
- [df40c-jig](https://github.com/laenzlinger/df40c-jig) — alignment jig used during assembly

## License

[CERN Open Hardware Licence Version 2 - Permissive](https://ohwr.org/cern_ohl_p_v2.txt)
