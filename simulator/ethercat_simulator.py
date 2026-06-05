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
import math
import argparse
from datetime import datetime

# 配置
DEFAULT_UDP_HOST = "127.0.0.1"
DEFAULT_UDP_PORT = 9999
NUM_MACHINES = 40
SENSORS_PER_MACHINE = 14  # 8振动 + 4温度 + 2位移
INTERVAL_MS = 100  # 每100ms上报一次

# 传感器类型定义
SENSOR_TYPE_VIBRATION = 1
SENSOR_TYPE_TEMPERATURE = 2
SENSOR_TYPE_DISPLACEMENT = 3

# 传感器配置 (位置名称, 类型, 基准值, 波动范围)
SENSOR_CONFIG = [
    # 8个振动传感器
    ("前轴承径向X", SENSOR_TYPE_VIBRATION, 1.2, 0.8),
    ("前轴承径向Y", SENSOR_TYPE_VIBRATION, 1.0, 0.7),
    ("前轴承轴向", SENSOR_TYPE_VIBRATION, 0.8, 0.6),
    ("后轴承径向X", SENSOR_TYPE_VIBRATION, 1.5, 0.9),
    ("后轴承径向Y", SENSOR_TYPE_VIBRATION, 1.3, 0.8),
    ("后轴承轴向", SENSOR_TYPE_VIBRATION, 0.9, 0.5),
    ("电机端径向", SENSOR_TYPE_VIBRATION, 1.8, 1.0),
    ("刀具端径向", SENSOR_TYPE_VIBRATION, 2.0, 1.2),
    # 4个温度传感器
    ("前轴承座", SENSOR_TYPE_TEMPERATURE, 45.0, 5.0),
    ("后轴承座", SENSOR_TYPE_TEMPERATURE, 42.0, 4.0),
    ("定子绕组", SENSOR_TYPE_TEMPERATURE, 55.0, 8.0),
    ("环境温度", SENSOR_TYPE_TEMPERATURE, 25.0, 3.0),
    # 2个位移传感器
    ("轴向位移", SENSOR_TYPE_DISPLACEMENT, 0.02, 0.01),
    ("径向跳动", SENSOR_TYPE_DISPLACEMENT, 0.05, 0.02),
]


class EtherCATSimulator:
    def __init__(self, host=DEFAULT_UDP_HOST, port=DEFAULT_UDP_PORT, 
                 num_machines=NUM_MACHINES, anomaly_mode=False):
        self.host = host
        self.port = port
        self.num_machines = num_machines
        self.anomaly_mode = anomaly_mode
        self.sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.machine_states = {}
        self.anomaly_machines = set()
        
        # 初始化机床状态
        for i in range(1, num_machines + 1):
            self.machine_states[i] = {
                "spindle_speed": 8000.0,
                "load": 40.0,
                "degradation": 0.0,  # 退化程度 0-1
                "anomaly_vibration": False,
                "anomaly_temperature": False,
            }
        
        # 异常模式下，随机选择几台机床模拟故障
        if anomaly_mode:
            num_anomaly = min(5, num_machines // 8)
            self.anomaly_machines = set(random.sample(range(1, num_machines + 1), num_anomaly))
            print(f"异常模式: 机床 {self.anomaly_machines} 将模拟故障状态")

    def generate_sensor_value(self, machine_id, sensor_idx, timestamp):
        """生成单个传感器的模拟数据"""
        config = SENSOR_CONFIG[sensor_idx]
        name, sensor_type, base_value, range_value = config
        state = self.machine_states[machine_id]
        
        # 基础波动
        noise = random.gauss(0, range_value * 0.3)
        value = base_value + noise
        
        # 添加周期性波动（模拟主轴旋转）
        if sensor_type == SENSOR_TYPE_VIBRATION:
            freq = state["spindle_speed"] / 60.0  # Hz
            phase = (timestamp * freq * 2 * math.pi) % (2 * math.pi)
            value += math.sin(phase) * range_value * 0.5
            
            # 高次谐波
            for harmonic in [2, 3, 5]:
                value += math.sin(phase * harmonic) * range_value * 0.15
        
        # 温度随负载变化
        if sensor_type == SENSOR_TYPE_TEMPERATURE:
            value += (state["load"] - 40.0) * 0.2
        
        # 退化影响
        if state["degradation"] > 0:
            if sensor_type == SENSOR_TYPE_VIBRATION:
                value *= (1 + state["degradation"] * 2.0)
            elif sensor_type == SENSOR_TYPE_TEMPERATURE:
                value += state["degradation"] * 20.0
            elif sensor_type == SENSOR_TYPE_DISPLACEMENT:
                value *= (1 + state["degradation"] * 3.0)
        
        # 异常模式下的特殊处理
        if machine_id in self.anomaly_machines:
            if sensor_type == SENSOR_TYPE_VIBRATION and sensor_idx in [0, 1, 3, 4]:
                # 前/后轴承振动异常增大
                anomaly_factor = 3.0 + random.random() * 2.0
                value *= anomaly_factor
            if sensor_type == SENSOR_TYPE_TEMPERATURE and sensor_idx in [8, 9]:
                # 轴承温度异常
                value += 20.0 + random.random() * 10.0
        
        return value

    def build_packet(self, machine_id, timestamp_ms):
        """构建EtherCAT模拟数据包"""
        # 包头: 传感器数量(2字节) + 机床ID(2字节) + 时间戳(8字节)
        packet = struct.pack("<HHq", SENSORS_PER_MACHINE, machine_id, timestamp_ms)
        
        state = self.machine_states[machine_id]
        
        for sensor_idx in range(SENSORS_PER_MACHINE):
            value = self.generate_sensor_value(machine_id, sensor_idx, timestamp_ms / 1000.0)
            sensor_type = SENSOR_CONFIG[sensor_idx][1]
            
            # 每个传感器数据: 传感器ID(2字节) + 类型(1字节) + 保留(1字节) + 
            #              数值(4字节) + 转速(4字节) + 负载(4字节) + 温度(4字节)
            sensor_id = sensor_idx + 1
            packet += struct.pack("<HHB B ffff", 
                                  sensor_id, 0,  # sensor_id, padding
                                  sensor_type, 0,  # type, padding
                                  value,
                                  state["spindle_speed"],
                                  state["load"],
                                  45.0)  # 参考温度
        
        return packet

    def update_machine_states(self):
        """更新机床状态（模拟工况变化）"""
        for machine_id, state in self.machine_states.items():
            # 转速波动
            state["spindle_speed"] = max(1000, min(15000, 
                state["spindle_speed"] + random.gauss(0, 50)))
            
            # 负载波动
            state["load"] = max(10, min(95, 
                state["load"] + random.gauss(0, 2)))
            
            # 缓慢退化（模拟长期磨损）
            state["degradation"] = min(1.0, 
                state["degradation"] + random.random() * 0.0001)
            
            # 异常机床退化更快
            if machine_id in self.anomaly_machines:
                state["degradation"] = min(1.0, 
                    state["degradation"] + random.random() * 0.001)

    def run(self, duration_minutes=0):
        """运行模拟器"""
        print(f"EtherCAT/UDP 模拟器启动")
        print(f"目标地址: {self.host}:{self.port}")
        print(f"机床数量: {self.num_machines}")
        print(f"上报间隔: {INTERVAL_MS}ms")
        print(f"传感器/机床: {SENSORS_PER_MACHINE}")
        print(f"数据点数/秒: {self.num_machines * SENSORS_PER_MACHINE * (1000 // INTERVAL_MS)}")
        
        if duration_minutes > 0:
            print(f"运行时长: {duration_minutes} 分钟")
        
        start_time = time.time()
        packet_count = 0
        
        try:
            while True:
                # 检查是否超时
                if duration_minutes > 0:
                    elapsed = time.time() - start_time
                    if elapsed >= duration_minutes * 60:
                        print(f"\n已运行 {duration_minutes} 分钟，停止模拟")
                        break
                
                timestamp_ms = int(time.time() * 1000)
                
                # 更新状态
                self.update_machine_states()
                
                # 为每台机床生成并发送数据包
                for machine_id in range(1, self.num_machines + 1):
                    packet = self.build_packet(machine_id, timestamp_ms)
                    self.sock.sendto(packet, (self.host, self.port))
                    packet_count += 1
                
                # 状态输出
                if packet_count % 1000 == 0:
                    elapsed = time.time() - start_time
                    rate = packet_count / elapsed if elapsed > 0 else 0
                    print(f"\r已发送 {packet_count} 个数据包, 速率: {rate:.1f} 包/秒", end="")
                
                # 等待下一个周期
                next_time = (timestamp_ms + INTERVAL_MS) / 1000.0
                sleep_time = next_time - time.time()
                if sleep_time > 0:
                    time.sleep(sleep_time)
        
        except KeyboardInterrupt:
            print(f"\n\n用户中断")
        
        finally:
            elapsed = time.time() - start_time
            print(f"总计发送 {packet_count} 个数据包")
            print(f"运行时间: {elapsed:.1f} 秒")
            print(f"平均速率: {packet_count / elapsed:.1f} 包/秒")
            self.sock.close()


def main():
    parser = argparse.ArgumentParser(description="EtherCAT/UDP 数据模拟器")
    parser.add_argument("--host", default=DEFAULT_UDP_HOST, help="UDP目标主机")
    parser.add_argument("--port", type=int, default=DEFAULT_UDP_PORT, help="UDP目标端口")
    parser.add_argument("--machines", type=int, default=NUM_MACHINES, help="模拟机床数量")
    parser.add_argument("--duration", type=int, default=0, help="运行时长（分钟），0表示无限运行")
    parser.add_argument("--anomaly", action="store_true", help="启用异常模式，模拟部分机床故障")
    
    args = parser.parse_args()
    
    simulator = EtherCATSimulator(
        host=args.host,
        port=args.port,
        num_machines=args.machines,
        anomaly_mode=args.anomaly
    )
    
    simulator.run(duration_minutes=args.duration)


if __name__ == "__main__":
    main()
