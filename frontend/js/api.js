const API_BASE = 'http://localhost:8080/api';

const API = {
    async getMachines() {
        try {
            const response = await fetch(`${API_BASE}/machines`);
            if (!response.ok) return this.getMockMachines();
            return await response.json();
        } catch (e) {
            console.warn('API连接失败，使用Mock数据');
            return this.getMockMachines();
        }
    },

    async getMachineStatus(machineId) {
        try {
            const response = await fetch(`${API_BASE}/machines/${machineId}/status`);
            if (!response.ok) return this.getMockMachineStatus(machineId);
            return await response.json();
        } catch (e) {
            return this.getMockMachineStatus(machineId);
        }
    },

    async getSensorHistory(machineId, sensorType, sensorId, hours = 1) {
        try {
            const response = await fetch(
                `${API_BASE}/machines/${machineId}/sensors/${sensorType}/${sensorId}/history?hours=${hours}`
            );
            if (!response.ok) return this.getMockHistory();
            return await response.json();
        } catch (e) {
            return this.getMockHistory();
        }
    },

    async getSensorPositions() {
        try {
            const response = await fetch(`${API_BASE}/sensors/positions`);
            if (!response.ok) return this.getMockSensorPositions();
            return await response.json();
        } catch (e) {
            return this.getMockSensorPositions();
        }
    },

    async getAlarms(limit = 20) {
        try {
            const response = await fetch(`${API_BASE}/alarms?limit=${limit}`);
            if (!response.ok) return this.getMockAlarms();
            return await response.json();
        } catch (e) {
            return this.getMockAlarms();
        }
    },

    async getMonthlyStats() {
        try {
            const response = await fetch(`${API_BASE}/stats/monthly`);
            if (!response.ok) return this.getMockMonthlyStats();
            return await response.json();
        } catch (e) {
            return this.getMockMonthlyStats();
        }
    },

    async getRanking() {
        try {
            const response = await fetch(`${API_BASE}/ranking`);
            if (!response.ok) return this.getMockRanking();
            return await response.json();
        } catch (e) {
            return this.getMockRanking();
        }
    },

    getMockMachines() {
        const machines = [];
        for (let i = 1; i <= 40; i++) {
            machines.push(this.getMockMachineStatus(i));
        }
        return machines;
    },

    getMockMachineStatus(id) {
        const hasFault = [5, 12, 28, 35].includes(id);
        const baseRms = hasFault ? 5 + Math.random() * 4 : 1 + Math.random() * 2;
        const baseRul = hasFault ? 150 + Math.random() * 200 : 5000 + Math.random() * 10000;
        const baseTemp = hasFault ? 55 + Math.random() * 15 : 35 + Math.random() * 15;
        
        let alarmStatus = 0;
        if (baseRms > 7.1 || baseRul < 200) alarmStatus = 2;
        else if (baseRms > 2.8 || baseRul < 500) alarmStatus = 1;

        return {
            machine_id: id,
            health_score: hasFault ? 50 + Math.random() * 20 : 80 + Math.random() * 20,
            rul_hours: baseRul,
            max_vibration_rms: baseRms,
            max_temperature: baseTemp,
            alarm_status: alarmStatus,
            last_update: new Date().toISOString()
        };
    },

    getMockHistory() {
        const data = [];
        const now = Date.now();
        for (let i = 0; i < 60; i++) {
            data.push({
                timestamp: { 0: now - (60 - i) * 60000 },
                value: 1.5 + Math.sin(i * 0.2) * 0.5 + Math.random() * 0.3
            });
        }
        return data;
    },

    getMockSensorPositions() {
        return [
            { id: 1, name: '前轴承径向X', x: 120, y: 80, location: '前端轴承座' },
            { id: 2, name: '前轴承径向Y', x: 120, y: 120, location: '前端轴承座' },
            { id: 3, name: '前轴承轴向', x: 80, y: 100, location: '前端轴承座' },
            { id: 4, name: '中轴承径向X', x: 250, y: 80, location: '中间支撑' },
            { id: 5, name: '中轴承径向Y', x: 250, y: 120, location: '中间支撑' },
            { id: 6, name: '后轴承径向X', x: 380, y: 80, location: '后端轴承座' },
            { id: 7, name: '后轴承径向Y', x: 380, y: 120, location: '后端轴承座' },
            { id: 8, name: '刀柄位置', x: 40, y: 100, location: '刀具接口' },
        ];
    },

    getMockAlarms() {
        return [
            {
                id: 'ALARM-001',
                timestamp: new Date(Date.now() - 300000).toISOString(),
                machine_id: 5,
                sensor_type: 'vibration',
                sensor_id: 1,
                level: 2,
                message: '振动烈度超过阈值7.1mm/s持续10秒',
                value: 8.5,
                threshold: 7.1,
                acknowledged: false
            },
            {
                id: 'ALARM-002',
                timestamp: new Date(Date.now() - 600000).toISOString(),
                machine_id: 12,
                sensor_type: 'rul',
                sensor_id: null,
                level: 2,
                message: '主轴剩余寿命低于200小时，需立即更换轴承',
                value: 185,
                threshold: 200,
                acknowledged: false
            },
            {
                id: 'ALARM-003',
                timestamp: new Date(Date.now() - 1800000).toISOString(),
                machine_id: 28,
                sensor_type: 'rul',
                sensor_id: null,
                level: 1,
                message: '主轴剩余寿命低于500小时，建议安排维护',
                value: 420,
                threshold: 500,
                acknowledged: false
            }
        ];
    },

    getMockMonthlyStats() {
        return {
            month: '2026-06',
            total_alarms: 47,
            critical_alarms: 8,
            warning_alarms: 39,
            avg_health_score: 87.3,
            machines_maintained: 5
        };
    },

    getMockRanking() {
        const ranking = [];
        for (let i = 1; i <= 40; i++) {
            const hasFault = [5, 12, 28, 35].includes(i);
            ranking.push({
                machine_id: i,
                health_score: hasFault ? 45 + Math.random() * 25 : 70 + Math.random() * 30,
                rul_hours: hasFault ? 100 + Math.random() * 400 : 6000 + Math.random() * 10000,
                max_vibration_rms: hasFault ? 4 + Math.random() * 5 : 1 + Math.random() * 2,
                max_temperature: hasFault ? 50 + Math.random() * 20 : 35 + Math.random() * 15,
                alarm_status: hasFault ? (Math.random() > 0.5 ? 2 : 1) : 0,
                last_update: new Date().toISOString()
            });
        }
        ranking.sort((a, b) => b.health_score - a.health_score);
        return ranking;
    }
};
