#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
EtherCAT/UDP 数据模拟器
模拟40台五轴数控机床的传感器数据上报
"""

import socket
import struct
import time
import random
import threading
import math
from typing import List, Dict
from dataclasses import dataclass
from datetime import datetime

UDP_HOST = "127.0.0.1"
UDP_PORT = 9876
MACHINE_COUNT = 40
SAMPLE_INTERVAL = 0.1  # 100ms

ECAT_MAGIC = 0xECAT

@dataclass
class VibrationSensor:
    base_rms: float
    noise_level: float
    fault_factor: float = 1.0

@dataclass
class TemperatureSensor:
    base_temp: float
    noise_level: float
    rising_rate: float = 0.0

@dataclass
class MachineState:
    machine_id: int
    is_running: bool
    spindle_speed: float
    base_speed: float
    vibration_sensors: List[VibrationSensor]
    temp_sensors: List[TemperatureSensor]
    health_degradation: float = 0.0
    runtime_hours: float = 0.0


def create_machine(machine_id: int) -> MachineState:
    base_speed = 8000 + random.uniform(-1000, 2000)
    
    vibration_sensors = []
    for i in range(8):
        base_rms = 1.0 + random.uniform(0, 1.5)
        if i in [0, 1, 5, 6]:
            base_rms *= 1.3
        vibration_sensors.append(VibrationSensor(
            base_rms=base_rms,
            noise_level=0.2 + random.uniform(0, 0.3),
        ))
    
    temp_sensors = []
    base_temp = 35.0 + random.uniform(0, 10)
    for i in range(4):
        temp_sensors.append(TemperatureSensor(
            base_temp=base_temp + i * 3.0,
            noise_level=0.3,
            rising_rate=0.001 + random.uniform(0, 0.002),
        ))
    
    if machine_id in [5, 12, 28, 35]:
        vibration_sensors[0].fault_factor = 2.5
        vibration_sensors[1].fault_factor = 2.0
        vibration_sensors[5].fault_factor = 1.8
        temp_sensors[0].base_temp += 15.0
    
    return MachineState(
        machine_id=machine_id,
        is_running=True,
        spindle_speed=base_speed,
        base_speed=base_speed,
        vibration_sensors=vibration_sensors,
        temp_sensors=temp_sensors,
        health_degradation=random.uniform(0, 0.3),
        runtime_hours=random.uniform(1000, 5000),
    )


def generate_packet(machine: MachineState, timestamp: float) -> bytes:
    packet = bytearray()
    
    packet += struct.pack('<H', ECAT_MAGIC)
    packet += struct.pack('<B', 1)
    packet += struct.pack('<B', 0)
    packet += struct.pack('<H', machine.machine_id)
    packet += struct.pack('<d', machine.spindle_speed)
    packet += struct.pack('<B', 8)
    packet += struct.pack('<B', 4)
    packet += struct.pack('<B', 2)
    
    t = timestamp
    for i, vib in enumerate(machine.vibration_sensors):
        freq_base = 50 + i * 20
        x = math.sin(2 * math.pi * freq_base * t) * vib.base_rms * 0.3
        y = math.sin(2 * math.pi * (freq_base * 1.5) * t) * vib.base_rms * 0.3
        z = math.sin(2 * math.pi * (freq_base * 0.7) * t) * vib.base_rms * 0.2
        
        x += random.gauss(0, vib.noise_level * 0.1)
        y += random.gauss(0, vib.noise_level * 0.1)
        z += random.gauss(0, vib.noise_level * 0.1)
        
        amplitude = vib.base_rms * vib.fault_factor * (1 + machine.health_degradation)
        rms = amplitude + random.gauss(0, vib.noise_level * 0.2)
        peak = rms * (2.5 + random.uniform(0, 1.5))
        crest_factor = peak / max(rms, 0.01)
        
        packet += struct.pack('<B', i + 1)
        packet += struct.pack('<ddd', x, y, z)
        packet += struct.pack('<ddd', rms, peak, crest_factor)
    
    for i, temp in enumerate(machine.temp_sensors):
        runtime_factor = min(machine.runtime_hours / 10000.0, 1.0)
        value = temp.base_temp + temp.rising_rate * machine.runtime_hours
        value += random.gauss(0, temp.noise_level)
        value += runtime_factor * 5.0
        
        packet += struct.pack('<B', i + 1)
        packet += struct.pack('<d', value)
    
    for i in range(2):
        axial = random.gauss(0.005, 0.002) * (1 + machine.health_degradation * 2)
        radial = random.gauss(0.01, 0.003) * (1 + machine.health_degradation * 2)
        
        packet += struct.pack('<B', i + 1)
        packet += struct.pack('<dd', axial, radial)
    
    return bytes(packet)


def machine_worker(machine: MachineState, sock: socket.socket, stop_event: threading.Event):
    start_time = time.time()
    
    while not stop_event.is_set():
        try:
            current_time = time.time()
            elapsed = current_time - start_time
            
            if machine.is_running:
                speed_variation = math.sin(elapsed * 0.1) * 200
                machine.spindle_speed = machine.base_speed + speed_variation
                machine.runtime_hours += SAMPLE_INTERVAL / 3600
            
            if random.random() < 0.0001:
                machine.health_degradation = min(machine.health_degradation + 0.001, 0.8)
            
            packet = generate_packet(machine, elapsed)
            sock.sendto(packet, (UDP_HOST, UDP_PORT))
            
            time.sleep(SAMPLE_INTERVAL)
        except Exception as e:
            print(f"机床 {machine.machine_id} 发送失败: {e}")
            time.sleep(SAMPLE_INTERVAL)


def main():
    print("=" * 60)
    print("EtherCAT/UDP 数据模拟器启动")
    print(f"目标地址: {UDP_HOST}:{UDP_PORT}")
    print(f"模拟机床数量: {MACHINE_COUNT}")
    print(f"采样间隔: {SAMPLE_INTERVAL * 1000}ms")
    print("=" * 60)
    
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    
    machines = [create_machine(i) for i in range(1, MACHINE_COUNT + 1)]
    
    stop_event = threading.Event()
    threads = []
    
    for machine in machines:
        t = threading.Thread(target=machine_worker, args=(machine, sock, stop_event), daemon=True)
        t.start()
        threads.append(t)
    
    print(f"{MACHINE_COUNT} 个机床模拟器线程已启动")
    print("按 Ctrl+C 停止...")
    print("-" * 60)
    
    try:
        packet_count = 0
        last_report = time.time()
        
        while True:
            time.sleep(1)
            packet_count += MACHINE_COUNT * int(1 / SAMPLE_INTERVAL)
            
            if time.time() - last_report >= 5:
                speed = packet_count / 5
                print(f"[{datetime.now().strftime('%H:%M:%S')}] 已发送 {packet_count} 包 (约 {speed:.0f} 包/秒)")
                print(f"  机床 5 (故障模拟): RMS={machines[4].vibration_sensors[0].base_rms * machines[4].vibration_sensors[0].fault_factor:.2f} mm/s")
                last_report = time.time()
                packet_count = 0
                
    except KeyboardInterrupt:
        print("\n正在停止...")
        stop_event.set()
        
        for t in threads:
            t.join(timeout=2)
        
        sock.close()
        print("模拟器已停止")


if __name__ == "__main__":
    main()
