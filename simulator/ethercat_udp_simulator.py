#!/usr/bin/env python3
import socket
import struct
import time
import random
import math
import threading
import argparse
from datetime import datetime

UDP_HOST = '127.0.0.1'
UDP_PORT = 5555
MACHINE_COUNT = 40
VIBRATION_SENSORS = 8
TEMPERATURE_SENSORS = 4
DISPLACEMENT_SENSORS = 2
SAMPLE_INTERVAL = 0.1  # 100ms

class MachineSimulator:
    def __init__(self, machine_id):
        self.machine_id = machine_id
        self.spindle_id = 1
        self.rpm_base = 3000 + random.uniform(-500, 500)
        self.vibration_base = [random.uniform(1.0, 3.0) for _ in range(VIBRATION_SENSORS)]
        self.temp_base = [random.uniform(35.0, 50.0) for _ in range(TEMPERATURE_SENSORS)]
        self.disp_base = [random.uniform(-0.02, 0.02) for _ in range(DISPLACEMENT_SENSORS)]
        self.health_degradation = max(0, (machine_id - 30) * 0.001) if machine_id > 30 else 0
        self.fault_mode = machine_id > 37  # 最后3台有严重故障
        self.warning_mode = 34 < machine_id <= 37  # 4台有警告
        self.time_counter = 0

    def generate_data(self):
        self.time_counter += SAMPLE_INTERVAL
        
        rpm = self.rpm_base + math.sin(self.time_counter * 0.5) * 50
        
        vibration = []
        for i in range(VIBRATION_SENSORS):
            base = self.vibration_base[i]
            if self.fault_mode:
                base += 5.0 + math.sin(self.time_counter * 10 + i) * 2.0
            elif self.warning_mode:
                base += 2.0 + math.sin(self.time_counter * 8 + i) * 1.5
            else:
                base += math.sin(self.time_counter * 5 + i) * 0.3
            
            noise = random.gauss(0, 0.2)
            vibration_val = abs(base + noise + self.health_degradation * self.time_counter * 0.01)
            vibration.append(round(vibration_val, 4))

        temperature = []
        for i in range(TEMPERATURE_SENSORS):
            base = self.temp_base[i]
            if self.fault_mode:
                base += 15.0
            elif self.warning_mode:
                base += 8.0
            base += math.sin(self.time_counter * 0.1 + i) * 2.0
            noise = random.gauss(0, 0.5)
            temperature.append(round(base + noise + self.health_degradation * self.time_counter * 0.005, 2))

        displacement = []
        for i in range(DISPLACEMENT_SENSORS):
            base = self.disp_base[i]
            if self.fault_mode:
                base += 0.1
            base += math.sin(self.time_counter * 2 + i) * 0.01
            noise = random.gauss(0, 0.002)
            displacement.append(round(base + noise, 6))

        return {
            'timestamp': int(time.time() * 1000),
            'machine_id': self.machine_id,
            'spindle_id': self.spindle_id,
            'vibration': vibration,
            'temperature': temperature,
            'displacement': displacement,
            'rpm': round(rpm, 1)
        }

    def pack_data(self, data):
        packet = bytearray()
        
        packet.extend(struct.pack('<q', data['timestamp']))
        packet.extend(struct.pack('<H', data['machine_id']))
        packet.extend(struct.pack('<B', data['spindle_id']))
        packet.extend(struct.pack('<B', VIBRATION_SENSORS))
        packet.extend(struct.pack('<B', TEMPERATURE_SENSORS))
        packet.extend(struct.pack('<B', DISPLACEMENT_SENSORS))
        packet.extend(struct.pack('<d', data['rpm']))
        
        for v in data['vibration']:
            packet.extend(struct.pack('<d', v))
        
        for t in data['temperature']:
            packet.extend(struct.pack('<d', t))
        
        for d in data['displacement']:
            packet.extend(struct.pack('<d', d))
        
        return bytes(packet)

def send_packet(sock, machine, packet_count):
    data = machine.generate_data()
    packet = machine.pack_data(data)
    
    try:
        sock.sendto(packet, (UDP_HOST, UDP_PORT))
        if packet_count % 1000 == 0:
            status = "FAULT" if machine.fault_mode else ("WARN" if machine.warning_mode else "OK")
            print(f"[{datetime.now().strftime('%H:%M:%S')}] Machine {machine.machine_id:02d} [{status}] "
                  f"RMS: {data['vibration'][0]:.2f} mm/s, Temp: {data['temperature'][0]:.1f}°C")
    except Exception as e:
        print(f"Error sending packet for machine {machine.machine_id}: {e}")

def main():
    parser = argparse.ArgumentParser(description='EtherCAT/UDP CNC Machine Data Simulator')
    parser.add_argument('--host', default=UDP_HOST, help='UDP server host')
    parser.add_argument('--port', type=int, default=UDP_PORT, help='UDP server port')
    parser.add_argument('--machines', type=int, default=MACHINE_COUNT, help='Number of machines')
    parser.add_argument('--interval', type=float, default=SAMPLE_INTERVAL, help='Sample interval in seconds')
    args = parser.parse_args()

    global UDP_HOST, UDP_PORT, MACHINE_COUNT, SAMPLE_INTERVAL
    UDP_HOST = args.host
    UDP_PORT = args.port
    MACHINE_COUNT = args.machines
    SAMPLE_INTERVAL = args.interval

    print("=" * 60)
    print("EtherCAT/UDP CNC Machine Data Simulator")
    print("=" * 60)
    print(f"Target: {UDP_HOST}:{UDP_PORT}")
    print(f"Machines: {MACHINE_COUNT}")
    print(f"Sample Interval: {SAMPLE_INTERVAL * 1000}ms")
    print(f"Vibration Sensors: {VIBRATION_SENSORS}")
    print(f"Temperature Sensors: {TEMPERATURE_SENSORS}")
    print(f"Displacement Sensors: {DISPLACEMENT_SENSORS}")
    print("-" * 60)
    print("Machine Status:")
    print(f"  - Machines 01-34: Normal operation")
    print(f"  - Machines 35-37: Warning level (elevated vibration/temp)")
    print(f"  - Machines 38-40: Fault mode (severe vibration/temp)")
    print("=" * 60)
    print("Press Ctrl+C to stop")
    print()

    machines = [MachineSimulator(i + 1) for i in range(MACHINE_COUNT)]
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    packet_count = 0

    try:
        while True:
            cycle_start = time.time()
            
            for machine in machines:
                send_packet(sock, machine, packet_count)
                packet_count += 1
            
            elapsed = time.time() - cycle_start
            sleep_time = max(0, SAMPLE_INTERVAL - elapsed)
            time.sleep(sleep_time)

    except KeyboardInterrupt:
        print("\n\nSimulation stopped by user")
        print(f"Total packets sent: {packet_count}")
        sock.close()

if __name__ == '__main__':
    main()
