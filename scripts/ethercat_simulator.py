#!/usr/bin/env python3
"""EtherCAT/UDP Simulator - CNC Machine Sensor Data Generator

Simulates 40 CNC machines with vibration, temperature, and displacement sensors,
sending data via UDP using both JSON and binary (struct-packed) protocols.
Supports bearing degradation modeling with gradual wear and random bursts.
"""

import argparse
import asyncio
import json
import math
import random
import struct
import time
from dataclasses import dataclass, field
from datetime import datetime, timezone
from enum import IntEnum
from typing import List


MAGIC_HEADER = b"ECAT"
HEADER_FMT = "!4sHHQ"
HEADER_SIZE = struct.calcsize(HEADER_FMT)
SENSOR_BLOCK_FMT = "!HBBddddddd"
SENSOR_BLOCK_SIZE = struct.calcsize(SENSOR_BLOCK_FMT)

SENSOR_TYPE_VIBRATION = 0
SENSOR_TYPE_TEMPERATURE = 1
SENSOR_TYPE_DISPLACEMENT = 2

SENSOR_TYPE_MAP = {
    "vibration": SENSOR_TYPE_VIBRATION,
    "temperature": SENSOR_TYPE_TEMPERATURE,
    "displacement": SENSOR_TYPE_DISPLACEMENT,
}


class SensorType(IntEnum):
    VIBRATION = 0
    TEMPERATURE = 1
    DISPLACEMENT = 2


@dataclass
class SensorConfig:
    sensor_id: int
    sensor_type: str


@dataclass
class MachineConfig:
    machine_id: int
    sensors: List[SensorConfig] = field(default_factory=list)
    rpm: float = 12000.0
    is_degrading: bool = False
    degradation_start_hour: float = 0.0
    degradation_severity: float = 0.0
    burst_active: bool = False
    burst_remaining_cycles: int = 0
    burst_intensity: float = 0.0


@dataclass
class SensorReading:
    machine_id: int
    sensor_id: int
    sensor_type: str
    timestamp: str
    value: float
    rpm: float
    vibration_rms: float
    temperature: float
    displacement: float


def build_sensor_configs() -> List[SensorConfig]:
    configs = []
    for sid in range(1, 9):
        configs.append(SensorConfig(sensor_id=sid, sensor_type="vibration"))
    for sid in range(9, 13):
        configs.append(SensorConfig(sensor_id=sid, sensor_type="temperature"))
    for sid in range(13, 15):
        configs.append(SensorConfig(sensor_id=sid, sensor_type="displacement"))
    return configs


class DegradationModel:
    def __init__(self, degradation_rate: float = 1.0):
        self.degradation_rate = degradation_rate
        self._per_machine_offset: dict = {}

    def _get_offset(self, machine_id: int) -> float:
        if machine_id not in self._per_machine_offset:
            self._per_machine_offset[machine_id] = random.uniform(0, 100)
        return self._per_machine_offset[machine_id]

    def compute_vibration_factor(self, machine: MachineConfig, elapsed_hours: float) -> float:
        if not machine.is_degrading:
            return 1.0
        effective_hours = max(0.0, elapsed_hours - machine.degradation_start_hour)
        if effective_hours <= 0:
            return 1.0
        base_factor = 1.0 + machine.degradation_severity * (
            1.0 - math.exp(-self.degradation_rate * effective_hours / 10.0)
        )
        burst_factor = 1.0
        if machine.burst_active:
            burst_factor = 1.0 + machine.burst_intensity
        return base_factor * burst_factor

    def compute_temperature_offset(self, machine: MachineConfig, elapsed_hours: float) -> float:
        if not machine.is_degrading:
            return 0.0
        effective_hours = max(0.0, elapsed_hours - machine.degradation_start_hour)
        if effective_hours <= 0:
            return 0.0
        return machine.degradation_severity * 5.0 * (
            1.0 - math.exp(-self.degradation_rate * effective_hours / 20.0)
        )

    def update_burst_state(self, machine: MachineConfig) -> None:
        if machine.burst_active:
            machine.burst_remaining_cycles -= 1
            if machine.burst_remaining_cycles <= 0:
                machine.burst_active = False
                machine.burst_intensity = 0.0
        else:
            if machine.is_degrading and random.random() < 0.002:
                machine.burst_active = True
                machine.burst_remaining_cycles = random.randint(5, 30)
                machine.burst_intensity = random.uniform(0.5, 3.0)


class MachineSimulator:
    VIBRATION_BASE_RMS = 1.2
    VIBRATION_NOISE_STD = 0.3
    TEMPERATURE_BASE = 40.0
    TEMPERATURE_NOISE_STD = 0.5
    DISPLACEMENT_BASE = 2.5
    DISPLACEMENT_NOISE_STD = 0.3

    def __init__(
        self,
        machine_id: int,
        sensors: List[SensorConfig],
        degradation_rate: float = 1.0,
    ):
        self.machine = MachineConfig(machine_id=machine_id, sensors=sensors)
        self.degradation = DegradationModel(degradation_rate)
        self.start_time = time.monotonic()
        self._rpm_phase = random.uniform(0, 2 * math.pi)

        if random.random() < 0.3:
            self.machine.is_degrading = True
            self.machine.degradation_start_hour = random.uniform(0, 4)
            self.machine.degradation_severity = random.uniform(0.5, 2.0)

        self._sensor_noise_offsets = {}
        for s in sensors:
            self._sensor_noise_offsets[s.sensor_id] = random.uniform(-0.1, 0.1)

    @property
    def machine_id(self) -> int:
        return self.machine.machine_id

    @property
    def sensors(self) -> List[SensorConfig]:
        return self.machine.sensors

    def _elapsed_hours(self) -> float:
        return (time.monotonic() - self.start_time) / 3600.0

    def _generate_rpm(self) -> float:
        t = time.monotonic()
        base_rpm = random.choice([6000, 8000, 10000, 12000, 15000, 18000, 24000])
        variation = 200 * math.sin(0.1 * t + self._rpm_phase)
        return max(6000, min(24000, base_rpm + variation + random.gauss(0, 50)))

    def generate_reading(self, sensor: SensorConfig, rpm: float) -> SensorReading:
        elapsed = self._elapsed_hours()
        vib_factor = self.degradation.compute_vibration_factor(self.machine, elapsed)
        temp_offset = self.degradation.compute_temperature_offset(self.machine, elapsed)
        noise_off = self._sensor_noise_offsets[sensor.sensor_id]

        if sensor.sensor_type == "vibration":
            bearing_freq_1x = rpm / 60.0
            bpfo = bearing_freq_1x * 3.57
            bpfi = bearing_freq_1x * 5.43
            bsf = bearing_freq_1x * 2.36
            ftf = bearing_freq_1x * 0.42

            t = time.monotonic()
            signal = 0.0
            signal += 0.3 * math.sin(2 * math.pi * bpfo * t / 1000)
            signal += 0.25 * math.sin(2 * math.pi * bpfi * t / 1000)
            signal += 0.15 * math.sin(2 * math.pi * bsf * t / 1000)
            signal += 0.1 * math.sin(2 * math.pi * ftf * t / 1000)

            degraded_amplitude = signal * vib_factor
            noise = random.gauss(0, self.VIBRATION_NOISE_STD)
            value = self.VIBRATION_BASE_RMS * vib_factor + degraded_amplitude + noise + noise_off
            vibration_rms = abs(value)
            temperature = self.TEMPERATURE_BASE + temp_offset + random.gauss(0, self.TEMPERATURE_NOISE_STD)
            displacement = self.DISPLACEMENT_BASE + random.gauss(0, self.DISPLACEMENT_NOISE_STD)

        elif sensor.sensor_type == "temperature":
            value = self.TEMPERATURE_BASE + temp_offset + noise_off + random.gauss(0, self.TEMPERATURE_NOISE_STD)
            vibration_rms = self.VIBRATION_BASE_RMS * vib_factor + random.gauss(0, self.VIBRATION_NOISE_STD)
            temperature = value
            displacement = self.DISPLACEMENT_BASE + random.gauss(0, self.DISPLACEMENT_NOISE_STD)

        else:
            disp_degrad = 0.0
            if self.machine.is_degrading and elapsed > self.machine.degradation_start_hour:
                eff_h = elapsed - self.machine.degradation_start_hour
                disp_degrad = self.machine.degradation_severity * 1.5 * (
                    1.0 - math.exp(-self.degradation.degradation_rate * eff_h / 15.0)
                )
            value = self.DISPLACEMENT_BASE + disp_degrad + noise_off + random.gauss(0, self.DISPLACEMENT_NOISE_STD)
            vibration_rms = self.VIBRATION_BASE_RMS * vib_factor + random.gauss(0, self.VIBRATION_NOISE_STD)
            temperature = self.TEMPERATURE_BASE + temp_offset + random.gauss(0, self.TEMPERATURE_NOISE_STD)
            displacement = value

        now = datetime.now(timezone.utc)
        return SensorReading(
            machine_id=self.machine_id,
            sensor_id=sensor.sensor_id,
            sensor_type=sensor.sensor_type,
            timestamp=now.isoformat(),
            value=round(value, 6),
            rpm=round(rpm, 1),
            vibration_rms=round(vibration_rms, 6),
            temperature=round(temperature, 4),
            displacement=round(displacement, 4),
        )

    def generate_fft_spectrum(self, sensor: SensorConfig, rpm: float) -> dict:
        bearing_freq_1x = rpm / 60.0
        bpfo = bearing_freq_1x * 3.57
        bpfi = bearing_freq_1x * 5.43
        bsf = bearing_freq_1x * 2.36
        ftf = bearing_freq_1x * 0.42

        num_bins = 256
        max_freq = 2000.0
        freq_resolution = max_freq / num_bins

        elapsed = self._elapsed_hours()
        vib_factor = self.degradation.compute_vibration_factor(self.machine, elapsed)

        spectrum = []
        for i in range(num_bins):
            freq = i * freq_resolution
            amplitude = random.gauss(0.01, 0.02)

            for characteristic_freq in [bpfo, bpfi, bsf, ftf, bearing_freq_1x]:
                for harmonic in range(1, 6):
                    peak_freq = characteristic_freq * harmonic
                    if abs(freq - peak_freq) < freq_resolution * 2:
                        amplitude += (0.5 / harmonic) * vib_factor * math.exp(
                            -((freq - peak_freq) ** 2) / (2 * (freq_resolution * 1.5) ** 2)
                        )

            amplitude = max(0.0, amplitude)
            spectrum.append(round(amplitude, 6))

        now = datetime.now(timezone.utc)
        return {
            "machine_id": self.machine_id,
            "sensor_id": sensor.sensor_id,
            "sensor_type": "vibration",
            "timestamp": now.isoformat(),
            "rpm": round(rpm, 1),
            "spectrum_freq_resolution": round(freq_resolution, 4),
            "spectrum_max_freq": max_freq,
            "spectrum_bins": num_bins,
            "spectrum": spectrum,
            "degradation_factor": round(vib_factor, 4),
        }

    def update(self) -> None:
        self.degradation.update_burst_state(self.machine)

    def tick_count(self) -> int:
        return int((time.monotonic() - self.start_time) / 0.1)


class BinaryProtocol:
    @staticmethod
    def pack_header(machine_id: int, sensor_count: int, timestamp_ns: int) -> bytes:
        return struct.pack(HEADER_FMT, MAGIC_HEADER, machine_id, sensor_count, timestamp_ns)

    @staticmethod
    def pack_sensor_block(reading: SensorReading) -> bytes:
        type_code = SENSOR_TYPE_MAP.get(reading.sensor_type, 0)
        return struct.pack(
            SENSOR_BLOCK_FMT,
            reading.sensor_id,
            type_code,
            0,
            reading.value,
            reading.rpm,
            reading.vibration_rms,
            reading.temperature,
            reading.displacement,
            0.0,
            0.0,
        )

    @staticmethod
    def pack_machine_data(readings: List[SensorReading]) -> bytes:
        if not readings:
            return b""
        first = readings[0]
        ts = datetime.fromisoformat(first.timestamp)
        timestamp_ns = int(ts.timestamp() * 1e9)
        header = BinaryProtocol.pack_header(first.machine_id, len(readings), timestamp_ns)
        blocks = b"".join(BinaryProtocol.pack_sensor_block(r) for r in readings)
        return header + blocks

    @staticmethod
    def unpack_header(data: bytes):
        magic, machine_id, sensor_count, timestamp_ns = struct.unpack(HEADER_FMT, data[:HEADER_SIZE])
        return magic, machine_id, sensor_count, timestamp_ns

    @staticmethod
    def unpack_sensor_block(data: bytes):
        fields = struct.unpack(SENSOR_BLOCK_FMT, data[:SENSOR_BLOCK_SIZE])
        return {
            "sensor_id": fields[0],
            "type_code": fields[1],
            "reserved": fields[2],
            "value": fields[3],
            "rpm": fields[4],
            "vibration_rms": fields[5],
            "temperature": fields[6],
            "displacement": fields[7],
        }


class UDPSender:
    def __init__(self, host: str, port: int):
        self.host = host
        self.port = port
        self._transport = None
        self._loop = None

    async def start(self) -> None:
        self._loop = asyncio.get_running_loop()
        self._transport, _ = await self._loop.create_datagram_endpoint(
            asyncio.DatagramProtocol,
            remote_addr=(self.host, self.port),
        )

    async def send_json(self, data: dict) -> None:
        payload = json.dumps(data, separators=(",", ":")).encode("utf-8")
        self._transport.sendto(payload)

    async def send_binary(self, readings: List[SensorReading]) -> None:
        payload = BinaryProtocol.pack_machine_data(readings)
        self._transport.sendto(payload)

    async def close(self) -> None:
        if self._transport:
            self._transport.close()


class Simulator:
    def __init__(self, args: argparse.Namespace):
        self.target_host = args.target_host
        self.target_port = args.target_port
        self.num_machines = args.num_machines
        self.degradation_rate = args.degradation_rate
        self.send_binary = args.binary
        self.machines: List[MachineSimulator] = []
        self.sender: UDPSender = None

    def _create_machines(self) -> None:
        sensor_configs = build_sensor_configs()
        for mid in range(1, self.num_machines + 1):
            sim = MachineSimulator(
                machine_id=mid,
                sensors=sensor_configs,
                degradation_rate=self.degradation_rate,
            )
            self.machines.append(sim)

    async def _send_cycle(self) -> None:
        for machine in self.machines:
            machine.update()
            rpm = machine._generate_rpm()
            readings = []
            for sensor in machine.sensors:
                reading = machine.generate_reading(sensor, rpm)
                readings.append(reading)

            if self.send_binary:
                await self.sender.send_binary(readings)
            else:
                for reading in readings:
                    data = {
                        "machine_id": reading.machine_id,
                        "sensor_id": reading.sensor_id,
                        "sensor_type": reading.sensor_type,
                        "timestamp": reading.timestamp,
                        "value": reading.value,
                        "rpm": reading.rpm,
                        "vibration_rms": reading.vibration_rms,
                        "temperature": reading.temperature,
                        "displacement": reading.displacement,
                    }
                    await self.sender.send_json(data)

    async def _send_fft_cycle(self) -> None:
        for machine in self.machines:
            vibration_sensors = [s for s in machine.sensors if s.sensor_type == "vibration"]
            if not vibration_sensors:
                continue
            chosen = random.choice(vibration_sensors)
            rpm = machine._generate_rpm()
            spectrum_data = machine.generate_fft_spectrum(chosen, rpm)
            await self.sender.send_json(spectrum_data)

    async def run(self) -> None:
        self._create_machines()
        self.sender = UDPSender(self.target_host, self.target_port)
        await self.sender.start()

        print(f"EtherCAT Simulator started: {self.num_machines} machines -> {self.target_host}:{self.target_port}")
        print(f"Protocol: {'binary' if self.send_binary else 'JSON'}")
        print(f"Degradation rate: {self.degradation_rate}")

        degrading_count = sum(1 for m in self.machines if m.machine.is_degrading)
        print(f"Machines with degradation: {degrading_count}/{self.num_machines}")

        tick = 0
        try:
            while True:
                await self._send_cycle()
                tick += 1
                if tick % 10 == 0:
                    await self._send_fft_cycle()
                if tick % 100 == 0:
                    elapsed = (time.monotonic() - self.machines[0].start_time)
                    degrading = [
                        m for m in self.machines
                        if m.machine.is_degrading and m.machine.burst_active
                    ]
                    burst_info = f" | bursts active: {len(degrading)}" if degrading else ""
                    print(f"[{elapsed:.1f}s] tick={tick} messages sent{burst_info}")
                await asyncio.sleep(0.1)
        except KeyboardInterrupt:
            print("\nSimulator stopped.")
        finally:
            await self.sender.close()


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="EtherCAT/UDP CNC Sensor Simulator")
    parser.add_argument(
        "--target-host",
        type=str,
        default="127.0.0.1",
        help="UDP target host (default: 127.0.0.1)",
    )
    parser.add_argument(
        "--target-port",
        type=int,
        default=9001,
        help="UDP target port (default: 9001)",
    )
    parser.add_argument(
        "--num-machines",
        type=int,
        default=40,
        help="Number of CNC machines to simulate (default: 40)",
    )
    parser.add_argument(
        "--degradation-rate",
        type=float,
        default=1.0,
        help="Bearing degradation rate multiplier (default: 1.0)",
    )
    parser.add_argument(
        "--binary",
        action="store_true",
        default=False,
        help="Use binary struct-packed protocol instead of JSON",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    sim = Simulator(args)
    asyncio.run(sim.run())


if __name__ == "__main__":
    main()
