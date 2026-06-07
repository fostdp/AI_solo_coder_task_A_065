const App = {
    currentMachineId: 1,
    machineStatuses: {},
    mockMode: true,
    spindleProfile: null,
    waterfallPlot: null,
    selectedSensorIndex: 0,

    init() {
        this.setupTabs();
        this.setupMachineList();
        this.initComponents();
        this.setupSensorListener();
        this.startTimeUpdater();
        this.loadInitialData();
        this.connectWebSocket();
        this.startDataUpdate();
    },

    setupTabs() {
        const tabBtns = document.querySelectorAll('.tab-btn');
        tabBtns.forEach(btn => {
            btn.addEventListener('click', () => {
                const tabId = btn.dataset.tab;
                this.switchTab(tabId);
            });
        });
    },

    switchTab(tabId) {
        document.querySelectorAll('.tab-btn').forEach(btn => btn.classList.remove('active'));
        document.querySelectorAll('.tab-content').forEach(content => content.classList.remove('active'));
        
        document.querySelector(`[data-tab="${tabId}"]`).classList.add('active');
        document.getElementById(`tab-${tabId}`).classList.add('active');

        if (tabId === 'ranking') {
            Ranking.loadRanking();
        } else if (tabId === 'stats') {
            Stats.loadStats();
        }
    },

    setupMachineList() {
        const list = document.getElementById('machine-list');
        list.innerHTML = '';
        
        for (let i = 1; i <= 40; i++) {
            const item = document.createElement('div');
            item.className = `machine-item ${i === this.currentMachineId ? 'active' : ''}`;
            item.textContent = `机${i.toString().padStart(2, '0')}`;
            item.dataset.machineId = i;
            
            item.addEventListener('click', () => {
                this.selectMachine(i);
            });
            
            list.appendChild(item);
        }
    },

    selectMachine(machineId) {
        this.currentMachineId = machineId;
        
        document.querySelectorAll('.machine-item').forEach(item => {
            item.classList.remove('active');
            if (parseInt(item.dataset.machineId) === machineId) {
                item.classList.add('active');
            }
        });

        document.getElementById('detail-machine-id').textContent = machineId;
        this.updateMachineDetail(machineId);
        
        const status = this.machineStatuses[machineId];
        if (status && this.spindleProfile) {
            const spindleData = this.buildSpindleData(status);
            this.spindleProfile.update(spindleData);
        }
    },

    initComponents() {
        this.spindleProfile = new SpindleProfile('spindle-canvas');
        this.spindleProfile.onSensorSelect = (index) => {
            this.selectedSensorIndex = index;
            if (this.waterfallPlot) {
                this.waterfallPlot.setSensorIndex(index);
            }
        };
        
        this.waterfallPlot = new WaterfallPlot('waterfall-canvas', {
            maxHistory: 60,
            maxFrequency: 500,
            frequencyBins: 128,
            sensorIndex: 0
        });
        
        SensorCharts.init();
        Stats.init();
    },

    buildSpindleData(status) {
        const sensorReadings = [];
        for (let i = 0; i < 8; i++) {
            sensorReadings.push({
                rms: (status.vibration_severity && status.vibration_severity[i]) || 0,
                type: i < 4 ? 'vibration' : 'temperature'
            });
        }
        for (let i = 0; i < 2; i++) {
            sensorReadings.push({
                value: (status.avg_temperature && status.avg_temperature[i]) || 0,
                type: 'displacement'
            });
        }
        return { sensorReadings, machineId: status.machine_id };
    },

    setupSensorListener() {
        document.addEventListener('sensorSelected', async (e) => {
            const { machineId, sensorIndex, type } = e.detail;
            
            document.getElementById('sensor-modal').classList.add('show');
            await SensorCharts.loadSensorData(machineId, sensorIndex);
        });
    },

    startTimeUpdater() {
        const updateTime = () => {
            const now = new Date();
            document.getElementById('current-time').textContent = now.toLocaleTimeString('zh-CN');
        };
        updateTime();
        setInterval(updateTime, 1000);
    },

    async loadInitialData() {
        const machines = await API.getAllMachines();
        
        if (machines && machines.length > 0) {
            this.mockMode = false;
            machines.forEach(m => {
                this.machineStatuses[m.machine_id] = m;
            });
        } else {
            this.generateMockData();
        }

        this.updateOverview();
        this.selectMachine(1);
    },

    generateMockData() {
        for (let i = 1; i <= 40; i++) {
            const health = 70 + Math.random() * 30;
            const vibration = [];
            for (let j = 0; j < 8; j++) {
                vibration.push(1 + Math.random() * 6);
            }
            
            this.machineStatuses[i] = {
                machine_id: i,
                health_score: health,
                rul_hours: health > 90 ? 8000 + Math.random() * 4000 : 
                           health > 70 ? 3000 + Math.random() * 5000 : 
                           500 + Math.random() * 2500,
                vibration_severity: vibration,
                avg_temperature: [35, 40, 38, 42].map(t => t + Math.random() * 10),
                alarm_level: i > 37 ? 2 : i > 34 ? 1 : 0,
                total_runtime_hours: 5000 + Math.random() * 10000
            };
        }
    },

    updateOverview() {
        const statuses = Object.values(this.machineStatuses);
        const avgHealth = statuses.reduce((sum, s) => sum + s.health_score, 0) / statuses.length;
        const avgRUL = statuses.reduce((sum, s) => sum + s.rul_hours, 0) / statuses.length;
        const criticalCount = statuses.filter(s => s.alarm_level === 2).length;
        const warningCount = statuses.filter(s => s.alarm_level === 1).length;

        document.getElementById('avg-health-score').textContent = avgHealth.toFixed(1);
        document.getElementById('machines-online').textContent = statuses.length;
        document.getElementById('avg-rul').textContent = Math.floor(avgRUL);
        document.getElementById('critical-alarm-count').textContent = criticalCount;
        document.getElementById('warning-alarm-count').textContent = warningCount;

        this.updateMachineListAlarms();
    },

    updateMachineListAlarms() {
        document.querySelectorAll('.machine-item').forEach(item => {
            const id = parseInt(item.dataset.machineId);
            const status = this.machineStatuses[id];
            if (status) {
                item.classList.remove('alarm-1', 'alarm-2');
                if (status.alarm_level === 2) {
                    item.classList.add('alarm-2');
                } else if (status.alarm_level === 1) {
                    item.classList.add('alarm-1');
                }
            }
        });
    },

    updateMachineDetail(machineId) {
        const status = this.machineStatuses[machineId];
        if (!status) return;

        document.getElementById('detail-health-score').textContent = status.health_score.toFixed(1);
        document.getElementById('detail-rul').textContent = status.rul_hours.toFixed(0) + ' 小时';
        document.getElementById('detail-runtime').textContent = status.total_runtime_hours.toFixed(0) + ' 小时';
        
        let alarmText = '<span style="color: #69f0ae;">正常</span>';
        if (status.alarm_level === 2) {
            alarmText = '<span style="color: #f44336;">二级更换预警</span>';
        } else if (status.alarm_level === 1) {
            alarmText = '<span style="color: #ff9800;">一级振动告警</span>';
        }
        document.getElementById('detail-alarm-level').innerHTML = alarmText;
    },

    connectWebSocket() {
        API.connectWebSocket((data) => {
            if (data.type_ === 'status_update' && Array.isArray(data.data)) {
                data.data.forEach(status => {
                    this.machineStatuses[status.machine_id] = status;
                });
                this.updateOverview();
                
                const status = this.machineStatuses[this.currentMachineId];
                if (status && this.spindleProfile) {
                    const spindleData = this.buildSpindleData(status);
                    this.spindleProfile.update(spindleData);
                    this.updateMachineDetail(this.currentMachineId);
                }
            }
        });
    },

    startDataUpdate() {
        setInterval(() => {
            if (this.mockMode) {
                this.updateMockData();
            }
            
            const status = this.machineStatuses[this.currentMachineId];
            if (status && this.waterfallPlot) {
                const mockSpectrum = [];
                for (let i = 0; i < 128; i++) {
                    const base = Math.exp(-i / 40) * 3;
                    const noise = Math.random() * 1.5;
                    mockSpectrum.push(base + noise + 0.3);
                }
                this.waterfallPlot.update(mockSpectrum, this.selectedSensorIndex);
            }
        }, 2000);
    },

    updateMockData() {
        for (let i = 1; i <= 40; i++) {
            if (this.machineStatuses[i]) {
                this.machineStatuses[i].vibration_severity = this.machineStatuses[i].vibration_severity.map(v => {
                    const delta = (Math.random() - 0.5) * 0.5;
                    return Math.max(0.5, Math.min(10, v + delta));
                });
                this.machineStatuses[i].total_runtime_hours += 2 / 3600;
            }
        }
        this.updateOverview();
        
        const status = this.machineStatuses[this.currentMachineId];
        if (status && this.spindleProfile) {
            const spindleData = this.buildSpindleData(status);
            this.spindleProfile.update(spindleData);
        }
    }
};

document.addEventListener('DOMContentLoaded', () => {
    App.init();
});
