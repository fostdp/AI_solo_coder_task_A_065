import socket
import struct
import time
import random
import math
import os
import json
from datetime import datetime
from collections import defaultdict
import threading

UDP_TARGET = os.getenv("UDP_TARGET", "127.0.0.1:5555")
MACHINE_COUNT = int(os.getenv("MACHINE_COUNT", "40"))
SAMPLE_INTERVAL_MS = int(os.getenv("SAMPLE_INTERVAL_MS", "100"))
ANOMALY_INJECT = os.getenv("ANOMALY_INJECT", "true").lower() == "true"
DEGRADATION_INJECT = os.getenv("DEGRADATION_INJECT", "true").lower() == "true"
SAMPLES_PER_PACKET = int(os.getenv("SAMPLES_PER_PACKET", "10"))

VIBRATION_SENSORS = 8
TEMPERATURE_SENSORS = 4
DISPLACEMENT_SENSORS = 2
TOTAL_SENSORS = VIBRATION_SENSORS + TEMPERATURE_SENSORS + DISPLACEMENT_SENSORS


class BearingDegradation:
    def __init__(self, machine_id):
        self.machine_id = machine_id
        self.degradation_level = 0.0
        self.degradation_rate = random.uniform(0.00001, 0.00005)
        self.start_time = time.time()
        self.bearing_failures = set()
        
        if DEGRADATION_INJECT and random.random() < 0.3:
            self.bearing_failures.add(random.randint(0, 3))
            self.degradation_rate = random.uniform(0.0001, 0.0005)

    def step(self):
        elapsed = time.time() - self.start_time
        self.degradation_level = min(1.0, elapsed * self.degradation_rate)
        return self.degradation_level

    def get_vibration_multiplier(self, sensor_idx):
        if sensor_idx < 4 and sensor_idx in self.bearing_failures:
            base = 1.0 + self.degradation_level * 4.0
            if self.degradation_level > 0.5:
                base += random.uniform(0, 2.0)
            return base
        return 1.0 + self.degradation_level * 0.3


class AnomalyInjector:
    def __init__(self):
        self.active_anomalies = {}
        self.anomaly_history = []

    def inject_anomaly(self, machine_id, sensor_idx, anomaly_type, duration_sec=5):
        key = (machine_id, sensor_idx)
        self.active_anomalies[key] = {
            "type": anomaly_type,
            "start": time.time(),
            "duration": duration_sec,
            "amplitude": random.uniform(2.0, 8.0)
        }
        self.anomaly_history.append({
            "machine_id": machine_id,
            "sensor_idx": sensor_idx,
            "type": anomaly_type,
            "timestamp": datetime.now().isoformat()
        })
        print(f"[ANOMALY] Machine {machine_id}, Sensor {sensor_idx}: {anomaly_type} injected")

    def get_effect(self, machine_id, sensor_idx):
        key = (machine_id, sensor_idx)
        if key not in self.active_anomalies:
            return 1.0, 0.0

        anomaly = self.active_anomalies[key]
        elapsed = time.time() - anomaly["start"]

        if elapsed > anomaly["duration"]:
            del self.active_anomalies[key]
            return 1.0, 0.0

        progress = elapsed / anomaly["duration"]
        envelope = math.sin(progress * math.pi)

        if anomaly["type"] == "impact":
            return 1.0 + anomaly["amplitude"] * envelope, 0.0
        elif anomaly["type"] == "unbalance":
            return 1.0 + anomaly["amplitude"] * 0.5 * envelope, 0.0
        elif anomaly["type"] == "misalignment":
            return 1.0 + anomaly["amplitude"] * 0.3 * envelope, anomaly["amplitude"] * 0.2
        elif anomaly["type"] == "cage_fault":
            return 1.0 + anomaly["amplitude"] * envelope * (0.8 + 0.4 * math.sin(elapsed * 20)), 0.0

        return 1.0, 0.0

    def maybe_inject_random(self, machine_id):
        if not ANOMALY_INJECT:
            return
        if random.random() < 0.001:
            sensor_idx = random.randint(0, VIBRATION_SENSORS - 1)
            anomaly_types = ["impact", "unbalance", "misalignment", "cage_fault"]
            self.inject_anomaly(
                machine_id,
                sensor_idx,
                random.choice(anomaly_types),
                duration_sec=random.randint(3, 15)
            )


class EtherCATSimulator:
    def __init__(self):
        self.sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        target_host, target_port = UDP_TARGET.split(":")
        self.target = (target_host, int(target_port))

        self.degradations = {mid: BearingDegradation(mid) for mid in range(1, MACHINE_COUNT + 1)}
        self.anomaly_injector = AnomalyInjector()

        self.machine_rpm = {}
        for mid in range(1, MACHINE_COUNT + 1):
            self.machine_rpm[mid] = random.choice([1500, 3000, 4500])

        self.stats = defaultdict(int)
        self.running = True

        print(f"[INIT] EtherCAT Simulator started")
        print(f"[INIT] Target: {UDP_TARGET}")
        print(f"[INIT] Machines: {MACHINE_COUNT}")
        print(f"[INIT] Sample interval: {SAMPLE_INTERVAL_MS}ms")
        print(f"[INIT] Sensors per machine: {TOTAL_SENSORS} (8V+4T+2D)")
        print(f"[INIT] Anomaly injection: {ANOMALY_INJECT}")
        print(f"[INIT] Degradation injection: {DEGRADATION_INJECT}")

    def generate_vibration(self, machine_id, sensor_idx, rpm):
        deg = self.degradations[machine_id]
        mult = deg.get_vibration_multiplier(sensor_idx)
        vib_mult, offset = self.anomaly_injector.get_effect(machine_id, sensor_idx)

        base_freq = rpm / 60.0
        t = time.time()

        waveform = 0.0
        waveform += math.sin(2 * math.pi * base_freq * t) * 0.5
        waveform += math.sin(2 * math.pi * base_freq * 2 * t) * 0.25
        waveform += math.sin(2 * math.pi * base_freq * 3 * t) * 0.1
        waveform += random.gauss(0, 0.1)

        base_rms = 0.8 + sensor_idx * 0.15
        rms = base_rms * mult * vib_mult + offset
        rms += random.gauss(0, 0.05)

        return max(0.1, min(20.0, rms))

    def generate_temperature(self, machine_id, sensor_idx, rpm):
        base_temp = 35.0 + sensor_idx * 2.0
        rpm_factor = (rpm / 3000.0) * 5.0
        deg = self.degradations[machine_id]
        deg_factor = deg.degradation_level * 15.0

        temp = base_temp + rpm_factor + deg_factor
        temp += random.gauss(0, 0.3)
        return max(20.0, min(120.0, temp))

    def generate_displacement(self, machine_id, sensor_idx, rpm):
        base_disp = 5.0 + sensor_idx * 1.0
        rpm_factor = (rpm / 3000.0) * 3.0
        deg = self.degradations[machine_id]
        deg_factor = deg.degradation_level * 20.0

        disp = base_disp + rpm_factor + deg_factor
        disp += random.gauss(0, 0.2)
        return max(1.0, min(200.0, disp))

    def build_packet(self, machine_id):
        rpm = self.machine_rpm[machine_id]
        timestamp = int(time.time() * 1e6)

        self.anomaly_injector.maybe_inject_random(machine_id)
        self.degradations[machine_id].step()

        samples = []
        for _ in range(SAMPLES_PER_PACKET):
            sample_data = []

            for s in range(VIBRATION_SENSORS):
                sample_data.append(self.generate_vibration(machine_id, s, rpm))
            for s in range(TEMPERATURE_SENSORS):
                sample_data.append(self.generate_temperature(machine_id, s, rpm))
            for s in range(DISPLACEMENT_SENSORS):
                sample_data.append(self.generate_displacement(machine_id, s, rpm))

            samples.extend(sample_data)

        format_str = f"<HQI{TOTAL_SENSORS * SAMPLES_PER_PACKET}f"
        payload = struct.pack(
            format_str,
            machine_id,
            timestamp,
            SAMPLES_PER_PACKET,
            *samples
        )

        return payload

    def send_loop(self):
        interval = SAMPLE_INTERVAL_MS / 1000.0
        packet_count = 0
        last_report = time.time()

        while self.running:
            cycle_start = time.time()

            for mid in range(1, MACHINE_COUNT + 1):
                try:
                    packet = self.build_packet(mid)
                    self.sock.sendto(packet, self.target)
                    packet_count += 1
                    self.stats["total_packets"] += 1
                except Exception as e:
                    self.stats["send_errors"] += 1
                    if self.stats["send_errors"] < 10:
                        print(f"[ERROR] Send failed: {e}")

            if time.time() - last_report >= 10:
                pps = packet_count / (time.time() - last_report)
                total = self.stats["total_packets"]
                print(f"[STATS] {pps:.1f} packets/sec, total: {total}, target: {UDP_TARGET}")
                packet_count = 0
                last_report = time.time()

            elapsed = time.time() - cycle_start
            sleep_time = max(0, interval - elapsed)
            time.sleep(sleep_time)

    def control_shell(self):
        time.sleep(2)
        print("\n[CONTROL] Type 'help' for available commands")

        while self.running:
            try:
                cmd = input("> ").strip().lower()
                if not cmd:
                    continue

                parts = cmd.split()
                if parts[0] == "help":
                    print("\nCommands:")
                    print("  status                    - Show simulator status")
                    print("  list                      - List all machines with degradation")
                    print("  inject <mid> <sensor> <type>  - Inject anomaly (types: impact, unbalance, misalignment, cage_fault)")
                    print("  rpm <mid> <rpm>           - Set machine RPM (1000-6000)")
                    print("  degrade <mid> <level>     - Force degradation level (0-1)")
                    print("  stats                     - Show statistics")
                    print("  quit                      - Exit simulator\n")

                elif parts[0] == "status":
                    print(f"\nStatus: Running={self.running}")
                    print(f"Machines: {MACHINE_COUNT}, Interval: {SAMPLE_INTERVAL_MS}ms")
                    print(f"Active anomalies: {len(self.anomaly_injector.active_anomalies)}")
                    print(f"Total packets: {self.stats['total_packets']}")

                elif parts[0] == "list":
                    print(f"\n{'Machine':<10} {'RPM':<8} {'Degradation':<12} {'Faulty Bearings'}")
                    print("-" * 50)
                    for mid in sorted(self.degradations.keys()):
                        d = self.degradations[mid]
                        faults = ",".join(str(b) for b in d.bearing_failures) or "None"
                        print(f"{mid:<10} {self.machine_rpm[mid]:<8} {d.degradation_level:<12.4f} {faults}")

                elif parts[0] == "inject" and len(parts) >= 4:
                    mid = int(parts[1])
                    sensor = int(parts[2])
                    atype = parts[3]
                    duration = int(parts[4]) if len(parts) > 4 else 10
                    self.anomaly_injector.inject_anomaly(mid, sensor, atype, duration)

                elif parts[0] == "rpm" and len(parts) >= 3:
                    mid = int(parts[1])
                    rpm = int(parts[2])
                    if mid in self.machine_rpm:
                        self.machine_rpm[mid] = max(1000, min(6000, rpm))
                        print(f"Machine {mid} RPM set to {self.machine_rpm[mid]}")

                elif parts[0] == "degrade" and len(parts) >= 3:
                    mid = int(parts[1])
                    level = float(parts[2])
                    if mid in self.degradations:
                        self.degradations[mid].degradation_level = max(0.0, min(1.0, level))
                        print(f"Machine {mid} degradation forced to {level}")

                elif parts[0] == "stats":
                    print(json.dumps(dict(self.stats), indent=2))

                elif parts[0] == "quit":
                    print("Shutting down...")
                    self.running = False
                    break

            except EOFError:
                break
            except Exception as e:
                print(f"Command error: {e}")

    def run(self):
        send_thread = threading.Thread(target=self.send_loop, daemon=True)
        send_thread.start()

        try:
            self.control_shell()
        except KeyboardInterrupt:
            print("\nInterrupted")
        finally:
            self.running = False
            send_thread.join(timeout=2)
            print("Simulator stopped")


if __name__ == "__main__":
    sim = EtherCATSimulator()
    sim.run()
