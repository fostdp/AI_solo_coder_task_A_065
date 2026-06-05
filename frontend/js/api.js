const API_BASE = 'http://localhost:8080/api';

const api = {
    async getMachines() {
        try {
            const response = await fetch(`${API_BASE}/machines`);
            return await response.json();
        } catch (e) {
            return this.getMockMachines();
        }
    },

    async getMachine(id) {
        try {
            const response = await fetch(`${API_BASE}/machines/${id}`);
            return await response.json();
        } catch (e) {
            return this.getMockMachine(id);
        }
    },

    async getSensors(machineId) {
        try {
            const response = await fetch(`${API_BASE}/machines/${machineId}/sensors`);
            return await response.json();
        } catch (e) {
            return this.getMockSensors(machineId);
        }
    },

    async getSensorData(machineId) {
        try {
            const response = await fetch(`${API_BASE}/machines/${machineId}/sensors/data`);
            return await response.json();
        } catch (e) {
            return this.getMockSensorData(machineId);
        }
    },

    async getSensorHistory(sensorId) {
        try {
            const response = await fetch(`${API_BASE}/sensors/${sensorId}/history`);
            return await response.json();
        } catch (e) {
            return this.getMockSensorHistory(sensorId);
        }
    },

    async getHealthRanking() {
        try {
            const response = await fetch(`${API_BASE}/ranking`);
            return await response.json();
        } catch (e) {
            return this.getMockRanking();
        }
    },

    async getFaultStatistics() {
        try {
            const response = await fetch(`${API_BASE}/statistics`);
            return await response.json();
        } catch (e) {
            return this.getMockStatistics();
        }
    },

    async getRUL(machineId) {
        try {
            const response = await fetch(`${API_BASE}/machines/${machineId}/rul`);
            return await response.json();
        } catch (e) {
            return this.getMockRUL(machineId);
        }
    },

    async getRecentAlarms() {
        try {
            const response = await fetch(`${API_BASE}/alarms`);
            return await response.json();
        } catch (e) {
            return this.getMockAlarms();
        }
    },

    getMockMachines() {
        const machines = [];
        for (let i = 1; i <= 40; i++) {
            machines.push({
                machine_id: i,
                machine_name: `CNC-${i}`,
                model: 'DMG MORI DMU 50',
                install_date: '2022-01-15',
                location: `Line-${Math.floor((i - 1) / 10) + 1}`,
                operator: `Operator-${(i % 10) + 1}`,
                status: i % 7 === 0 ? 'idle' : 'running'
            });
        }
        return machines;
    },

    getMockMachine(id) {
        return {
            machine_id: id,
            machine_name: `CNC-${id}`,
            model: 'DMG MORI DMU 50',
            install_date: '2022-01-15',
            location: `Line-${Math.floor((id - 1) / 10) + 1}`,
            operator: `Operator-${(id % 10) + 1}`,
            status: id % 7 === 0 ? 'idle' : 'running'
        };
    },

    getMockSensors(machineId) {
        const sensors = [];
        const positions = [
            { name: '前轴承径向X', x: -20, y: 0, z: 100, type: 1, unit: 'mm/s' },
            { name: '前轴承径向Y', x: 0, y: -20, z: 100, type: 1, unit: 'mm/s' },
            { name: '前轴承轴向', x: 0, y: 0, z: 120, type: 1, unit: 'mm/s' },
            { name: '后轴承径向X', x: -20, y: 0, z: -100, type: 1, unit: 'mm/s' },
            { name: '后轴承径向Y', x: 0, y: -20, z: -100, type: 1, unit: 'mm/s' },
            { name: '后轴承轴向', x: 0, y: 0, z: -120, type: 1, unit: 'mm/s' },
            { name: '电机端径向', x: -15, y: 0, z: 150, type: 1, unit: 'mm/s' },
            { name: '刀具端径向', x: -15, y: 0, z: -150, type: 1, unit: 'mm/s' },
            { name: '前轴承座', x: 0, y: 0, z: 100, type: 2, unit: '°C' },
            { name: '后轴承座', x: 0, y: 0, z: -100, type: 2, unit: '°C' },
            { name: '定子绕组', x: 0, y: 0, z: 50, type: 2, unit: '°C' },
            { name: '环境温度', x: 50, y: 0, z: 0, type: 2, unit: '°C' },
            { name: '轴向位移', x: 0, y: 0, z: 0, type: 3, unit: 'mm' },
            { name: '径向跳动', x: 0, y: 0, z: 0, type: 3, unit: 'mm' },
        ];

        positions.forEach((pos, idx) => {
            sensors.push({
                sensor_id: idx + 1,
                machine_id: machineId,
                sensor_type: pos.type,
                position_name: pos.name,
                position_x: pos.x,
                position_y: pos.y,
                position_z: pos.z,
                axis: 'x',
                unit: pos.unit,
                status: 1
            });
        });

        return sensors;
    },

    getMockSensorData(machineId) {
        const data = [];
        const now = Date.now() / 1000;
        
        for (let i = 1; i <= 14; i++) {
            let value;
            if (i <= 8) {
                value = 1.5 + Math.random() * 3.5;
            } else if (i <= 12) {
                value = 40 + Math.random() * 20;
            } else {
                value = (Math.random() - 0.5) * 0.1;
            }
            
            data.push({
                timestamp: now,
                machine_id: machineId,
                sensor_id: i,
                sensor_type: i <= 8 ? 1 : (i <= 12 ? 2 : 3),
                value_min: value * 0.9,
                value_max: value * 1.1,
                value_avg: value,
                value_rms: value,
                value_std: value * 0.1,
                value_peak: value * 1.5,
                spindle_speed_avg: 8000,
                load_avg: 45,
                temperature_avg: 45,
                sample_count: 10
            });
        }
        return data;
    },

    getMockSensorHistory(sensorId) {
        const recentData = [];
        const historyTrend = [];
        const now = Date.now() / 1000;

        for (let i = 3600; i >= 0; i -= 60) {
            const baseValue = sensorId <= 8 ? 2.0 : (sensorId <= 12 ? 45 : 0.02);
            recentData.push({
                timestamp: now - i,
                value: baseValue + (Math.random() - 0.5) * baseValue * 0.3
            });
        }

        for (let i = 7 * 24 * 3600; i >= 0; i -= 3600) {
            const baseValue = sensorId <= 8 ? 2.0 : (sensorId <= 12 ? 45 : 0.02);
            const trend = 1 + (7 * 24 * 3600 - i) / (7 * 24 * 3600) * 0.3;
            historyTrend.push({
                timestamp: now - i,
                value: baseValue * trend + (Math.random() - 0.5) * baseValue * 0.2
            });
        }

        const frequencies = [];
        const amplitudes = [];
        for (let i = 0; i < 100; i++) {
            frequencies.push(i * 10);
            amplitudes.push(i % 17 === 0 ? 2.5 + Math.random() : 0.5 + Math.random() * 0.3);
        }

        return {
            sensor_config: {
                sensor_id: sensorId,
                machine_id: 1,
                sensor_type: sensorId <= 8 ? 1 : (sensorId <= 12 ? 2 : 3),
                position_name: `传感器 ${sensorId}`,
                position_x: 0,
                position_y: 0,
                position_z: 0,
                axis: 'x',
                unit: sensorId <= 8 ? 'mm/s' : (sensorId <= 12 ? '°C' : 'mm'),
                status: 1
            },
            recent_data: recentData,
            spectrum: {
                timestamp: now,
                machine_id: 1,
                sensor_id: sensorId,
                frequency: frequencies,
                amplitude: amplitudes,
                rpm: 8000
            },
            history_trend: historyTrend
        };
    },

    getMockRanking() {
        const ranking = [];
        for (let i = 1; i <= 40; i++) {
            ranking.push({
                machine_id: i,
                machine_name: `CNC-${i}`,
                overall_score: Math.floor(60 + Math.random() * 40),
                rul_hours: 1000 + Math.random() * 5000,
                location: `Line-${Math.floor((i - 1) / 10) + 1}`
            });
        }
        return ranking.sort((a, b) => b.overall_score - a.overall_score);
    },

    getMockStatistics() {
        return [{
            month: '202606',
            total_alarms: 156,
            vibration_alarms: 68,
            temperature_alarms: 45,
            rul_alarms: 23,
            work_orders_created: 12
        }];
    },

    getMockRUL(machineId) {
        const baseRUL = 5000 + Math.random() * 3000;
        return {
            timestamp: Date.now() / 1000,
            machine_id: machineId,
            bearing_id: 1,
            rul_hours: baseRUL,
            rul_confidence: 0.85 + Math.random() * 0.1,
            vibration_rms_trend: 5 + Math.random() * 10,
            temperature_rate: 2 + Math.random() * 5,
            skf_l10_life: baseRUL * 1.1,
            lstm_prediction: baseRUL * 0.95,
            health_score: Math.floor(70 + Math.random() * 25)
        };
    },

    getMockAlarms() {
        const alarms = [];
        const now = Date.now() / 1000;
        
        for (let i = 0; i < 5; i++) {
            alarms.push({
                alarm_id: 'xxxx-xxxx-xxxx-xxxx',
                timestamp: now - i * 3600,
                machine_id: Math.floor(Math.random() * 40) + 1,
                sensor_id: Math.floor(Math.random() * 8) + 1,
                alarm_level: i === 0 ? 2 : 1,
                alarm_type: Math.floor(Math.random() * 4) + 1,
                alarm_message: i === 0 ? '振动烈度超限告警' : '温度异常预警',
                value: 8.5 + Math.random(),
                threshold: 7.1,
                duration_ms: 10000
            });
        }
        return alarms;
    }
};
