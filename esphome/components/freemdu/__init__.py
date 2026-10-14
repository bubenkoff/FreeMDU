"""ESPHome component for Miele appliances, built on the FreeMDU protocol crate.

The device implementations are the Rust ones from `protocol/`, linked in as a static library
(`../../ffi`). This file only describes the configuration and links that library.
"""

from pathlib import Path

import esphome.codegen as cg
# Imported under another name: this package's own sensor platform module would shadow it
from esphome.components import sensor as sensor_component, uart
from esphome.components.esp32 import (
    VARIANT_ESP32C3,
    VARIANT_ESP32C6,
    get_esp32_variant,
)
import esphome.config_validation as cv
from esphome.const import (
    CONF_ENERGY,
    CONF_ID,
    CONF_POWER,
    CONF_TIMEOUT,
    DEVICE_CLASS_ENERGY,
    DEVICE_CLASS_POWER,
    STATE_CLASS_MEASUREMENT,
    STATE_CLASS_TOTAL_INCREASING,
    UNIT_KILOWATT_HOURS,
    UNIT_WATT,
)
from esphome.core import CORE, EsphomeError

CODEOWNERS = ["@bubenkoff"]
DEPENDENCIES = ["uart"]
AUTO_LOAD = ["sensor"]
MULTI_CONF = True

CONF_FREEMDU_ID = "freemdu_id"
CONF_PROPERTY = "property"
CONF_DISCARD_ECHO = "discard_echo"
CONF_PROTOCOL_LOG = "protocol_log"
CONF_HOLD_TIME = "hold_time"
CONF_RECONNECT_INTERVAL = "reconnect_interval"
CONF_HEATER_POWER = "heater_power"
CONF_RUNNING_POWER = "running_power"
CONF_STANDBY_POWER = "standby_power"
CONF_DRUM_POWER = "drum_power_per_1000_rpm"
CONF_PROGRAM_ENERGY = "program_energy"
CONF_MEMORY_LOG = "memory_log"
CONF_ADDRESS = "address"
CONF_LENGTH = "length"

freemdu_ns = cg.esphome_ns.namespace("freemdu")
FreeMDUComponent = freemdu_ns.class_(
    "FreeMDUComponent", cg.PollingComponent, uart.UARTDevice
)

# Rust target the static library is built for, per ESP32 variant
RUST_TARGETS = {
    VARIANT_ESP32C3: "riscv32imc-unknown-none-elf",
    VARIANT_ESP32C6: "riscv32imac-unknown-none-elf",
}
LIB_DIR = Path(__file__).parent / "lib"
# Functions of freemdu_ffi.h used by freemdu.cpp
FFI_SYMBOLS = [
    "fmdu_connect",
    "fmdu_free",
    "fmdu_device_kind",
    "fmdu_property_find",
    "fmdu_query_property",
    "fmdu_status_str",
    "fmdu_set_logger",
    "fmdu_read_memory",
]

PROPERTY_ID = cv.All(cv.string_strict, cv.Length(min=1))

POWER = cv.All(cv.float_with_unit("power", "(W|w)?"), cv.float_range(min=0))

# Energy estimate for appliances without an energy meter of their own. The power is inferred
# from the device's own properties: the heater rating while the heater actuator is on, plus a
# flat figure while a program runs (or standby otherwise).
ENERGY_SCHEMA = cv.All(
    cv.Schema(
        {
            cv.Required(CONF_HEATER_POWER): POWER,
            cv.Optional(CONF_RUNNING_POWER, default="0W"): POWER,
            cv.Optional(CONF_STANDBY_POWER, default="0W"): POWER,
            # Added per 1000 rpm of measured drum speed (tachometer_speed), for the spin
            cv.Optional(CONF_DRUM_POWER, default="0W"): POWER,
            cv.Optional(CONF_POWER): sensor_component.sensor_schema(
                unit_of_measurement=UNIT_WATT,
                accuracy_decimals=0,
                device_class=DEVICE_CLASS_POWER,
                state_class=STATE_CLASS_MEASUREMENT,
            ),
            cv.Optional(CONF_ENERGY): sensor_component.sensor_schema(
                unit_of_measurement=UNIT_KILOWATT_HOURS,
                accuracy_decimals=3,
                device_class=DEVICE_CLASS_ENERGY,
                state_class=STATE_CLASS_TOTAL_INCREASING,
            ),
            cv.Optional(CONF_PROGRAM_ENERGY): sensor_component.sensor_schema(
                unit_of_measurement=UNIT_KILOWATT_HOURS,
                accuracy_decimals=2,
                device_class=DEVICE_CLASS_ENERGY,
            ),
        }
    ),
    cv.has_at_least_one_key(CONF_POWER, CONF_ENERGY, CONF_PROGRAM_ENERGY),
)


def _multiple_of_16(value):
    if value % 16:
        raise cv.Invalid("must be a multiple of 16")
    return value


def _validate_platform(config):
    if not CORE.is_esp32 or get_esp32_variant() not in RUST_TARGETS:
        raise cv.Invalid(
            "freemdu is only available for "
            + " and ".join(RUST_TARGETS)
            + " (the targets the FreeMDU firmware supports)"
        )
    return config


CONFIG_SCHEMA = cv.All(
    cv.Schema(
        {
            cv.GenerateID(): cv.declare_id(FreeMDUComponent),
            # The optical interface sees the adapter's own transmitter
            cv.Optional(CONF_DISCARD_ECHO, default=True): cv.boolean,
            # How long to wait for each byte from the device
            cv.Optional(
                CONF_TIMEOUT, default="500ms"
            ): cv.positive_time_period_milliseconds,
            # Log every request and byte (trace level of the protocol crate)
            cv.Optional(CONF_PROTOCOL_LOG, default=False): cv.boolean,
            # Keep the last values through failed polls for this long before reporting them as
            # unknown, so that a single missed poll does not interrupt a state sequence
            cv.Optional(
                CONF_HOLD_TIME, default="30s"
            ): cv.positive_time_period_milliseconds,
            # Poll this often instead while the device does not answer; 0s keeps update_interval
            cv.Optional(
                CONF_RECONNECT_INTERVAL, default="1s"
            ): cv.positive_time_period_milliseconds,
            cv.Optional(CONF_ENERGY): ENERGY_SCHEMA,
            # Log a range of the device's memory at every poll, for mapping a new device
            cv.Optional(CONF_MEMORY_LOG): cv.Schema(
                {
                    cv.Required(CONF_ADDRESS): cv.hex_uint32_t,
                    cv.Required(CONF_LENGTH): cv.All(
                        cv.int_range(min=16, max=4096), _multiple_of_16
                    ),
                }
            ),
        }
    )
    .extend(cv.polling_component_schema("10s"))
    .extend(uart.UART_DEVICE_SCHEMA),
    _validate_platform,
)

FINAL_VALIDATE_SCHEMA = uart.final_validate_device_schema(
    "freemdu",
    baud_rate=2400,
    require_rx=True,
    require_tx=True,
    data_bits=8,
    parity="EVEN",
    stop_bits=1,
)

FREEMDU_CHILD_SCHEMA = cv.Schema(
    {cv.GenerateID(CONF_FREEMDU_ID): cv.use_id(FreeMDUComponent)}
)


async def to_code(config):
    var = cg.new_Pvariable(config[CONF_ID])
    await cg.register_component(var, config)
    await uart.register_uart_device(var, config)

    cg.add(var.set_discard_echo(config[CONF_DISCARD_ECHO]))
    cg.add(var.set_timeout(config[CONF_TIMEOUT]))
    cg.add(var.set_protocol_log(config[CONF_PROTOCOL_LOG]))
    cg.add(var.set_hold_time(config[CONF_HOLD_TIME]))
    cg.add(var.set_reconnect_interval(config[CONF_RECONNECT_INTERVAL]))

    if memory_log := config.get(CONF_MEMORY_LOG):
        cg.add(var.set_memory_log(memory_log[CONF_ADDRESS], memory_log[CONF_LENGTH]))

    if energy := config.get(CONF_ENERGY):
        cg.add(
            var.set_energy_model(
                energy[CONF_HEATER_POWER],
                energy[CONF_RUNNING_POWER],
                energy[CONF_STANDBY_POWER],
                energy[CONF_DRUM_POWER],
            )
        )
        if CONF_POWER in energy:
            cg.add(var.set_power_sensor(await sensor_component.new_sensor(energy[CONF_POWER])))
        if CONF_ENERGY in energy:
            cg.add(var.set_energy_sensor(await sensor_component.new_sensor(energy[CONF_ENERGY])))
        if CONF_PROGRAM_ENERGY in energy:
            cg.add(
                var.set_program_energy_sensor(
                    await sensor_component.new_sensor(energy[CONF_PROGRAM_ENERGY])
                )
            )

    lib = LIB_DIR / RUST_TARGETS[get_esp32_variant()]
    if not (lib / "libfreemdu_ffi.a").is_file():
        raise EsphomeError(
            f"{lib / 'libfreemdu_ffi.a'} is missing; build it with esphome/build.sh"
        )
    # ESPHome's ESP-IDF build passes only -Wl options to the linker, and places them before the
    # object files. So the archive is named directly and its entry points are marked undefined
    # up front; otherwise the linker would skip it before anything referenced them.
    for symbol in FFI_SYMBOLS:
        cg.add_build_flag(f"-Wl,--undefined={symbol}")
    cg.add_build_flag(f"-Wl,{lib / 'libfreemdu_ffi.a'}")
