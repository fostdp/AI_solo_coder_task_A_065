class Dashboard {
    constructor() {
        this.currentMachineId = 1;
        this.sensorPositions = [];
        this.machines = [];
    }

    async init() {
        await this.loadSensorPositions();
        await this.loadMachines();
        this.initMachineSelector();
        this.initSensorSelectors();
        await this.updateDashboard();
        
        setInterval(() => this.updateTime(), 1000);
        setInterval(() => this.updateDashboard(), 3000);
        
        this.updateTime();
    }

    updateTime() {
        const now = new Date();
        document.getElementById('current-time').textContent = now.toLocaleString('zh-CN', {
            year: 'numeric',
            month: '2-digit',
            day: '2-digit',
            hour: '2-digit',
            minute: '2-digit',
            second: '2-digit'
        });
    }

    async loadSensorPositions() {
        this.sensorPositions = await API.getSensorPositions();
    }

    async loadMachines() {
        this.machines = await API.getMachines();
    }

    initMachineSelector() {
        const select = document.getElementById('machine-select');
        select.innerHTML = '';
        for (let i = 1; i <= 40; i++) {
            const option = document.createElement('option');
            option.value = i;
            option.textContent = `CNC-${i.toString().padStart(2, '0')}`;
            select.appendChild(option);
        }
        select.value = this.currentMachineId;
        
        select.addEventListener('change', (e) => {
            this.currentMachineId = parseInt(e.target.value);
            this.updateMachineView();
        });
    }

    initSensorSelectors() {
        const selects = ['waveform-sensor', 'spectrum-sensor', 'waterfall-sensor'];
        selects.forEach(id => {
            const select = document.getElementById(id);
            if (!select) return;
            select.innerHTML = '';
            this.sensorPositions.forEach(pos => {
                const option = document.createElement('option');
                option.value = pos.id;
                option.textContent = `${pos.id}. ${pos.name}`;
                select.appendChild(option);
            });
        });
    }

    async updateDashboard() {
        await Promise.all([
            this.updateMonthlyStats(),
            this.updateRanking(),
            this.updateAlarms(),
            this.updateMachineView()
        ]);
    }

    async updateMonthlyStats() {
        const stats = await API.getMonthlyStats();
        document.getElementById('stat-total-alarms').textContent = stats.total_alarms;
        document.getElementById('stat-critical').textContent = stats.critical_alarms;
        document.getElementById('stat-warning').textContent = stats.warning_alarms;
        document.getElementById('stat-health').textContent = stats.avg_health_score.toFixed(1);
    }

    async updateRanking() {
        const ranking = await API.getRanking();
        const container = document.getElementById('ranking-list');
        container.innerHTML = '';
        
        ranking.slice(0, 15).forEach((machine, index) => {
            const scoreClass = machine.health_score >= 80 ? 'score-good' :
                              machine.health_score >= 60 ? 'score-warning' : 'score-critical';
            
            const item = document.createElement('div');
            item.className = `ranking-item ${index < 3 ? 'top-3' : ''}`;
            item.innerHTML = `
                <span class="rank-number">${index + 1}</span>
                <span class="rank-machine">CNC-${machine.machine_id.toString().padStart(2, '0')}</span>
                <span class="rank-score ${scoreClass}">${machine.health_score.toFixed(1)}</span>
            `;
            
            item.addEventListener('click', () => {
                this.currentMachineId = machine.machine_id;
                document.getElementById('machine-select').value = machine.machine_id;
                this.updateMachineView();
            });
            
            container.appendChild(item);
        });
    }

    async updateAlarms() {
        const alarms = await API.getAlarms(10);
        const container = document.getElementById('alarm-list');
        container.innerHTML = '';
        
        alarms.forEach(alarm => {
            const levelClass = alarm.level === 2 ? 'critical' : 'warning';
            const levelText = alarm.level === 2 ? '严重' : '预警';
            const time = new Date(alarm.timestamp).toLocaleTimeString('zh-CN');
            
            const item = document.createElement('div');
            item.className = `alarm-item ${levelClass}`;
            item.innerHTML = `
                <div class="alarm-title">[${levelText}] CNC-${alarm.machine_id} ${alarm.message}</div>
                <div class="alarm-time">${time} · 当前值: ${alarm.value.toFixed(2)}</div>
            `;
            container.appendChild(item);
        });
        
        if (alarms.length === 0) {
            container.innerHTML = '<div style="color:#64748b;text-align:center;padding:20px;">暂无告警</div>';
        }
    }

    async updateMachineView() {
        const status = await API.getMachineStatus(this.currentMachineId);
        this.updateMachineInfo(status);
        this.updateSpindleCanvas(status);
        this.updateCharts();
    }

    updateMachineInfo(status) {
        const container = document.getElementById('machine-info');
        
        const alarmColors = {
            0: { text: '正常', color: '#4ade80' },
            1: { text: '预警', color: '#fbbf24' },
            2: { text: '告警', color: '#f87171' }
        };
        const alarm = alarmColors[status.alarm_status] || alarmColors[0];
        
        container.innerHTML = `
            <div class="info-item">
                <span class="info-label">健康评分</span>
                <span class="info-value" style="color:${status.health_score >= 80 ? '#4ade80' : status.health_score >= 60 ? '#fbbf24' : '#f87171'}">${status.health_score.toFixed(1)}</span>
            </div>
            <div class="info-item">
                <span class="info-label">RUL</span>
                <span class="info-value" style="color:${status.rul_hours >= 500 ? '#4ade80' : status.rul_hours >= 200 ? '#fbbf24' : '#f87171'}">${status.rul_hours.toFixed(0)}h</span>
            </div>
            <div class="info-item">
                <span class="info-label">最大振动</span>
                <span class="info-value" style="color:${status.max_vibration_rms < 2.8 ? '#4ade80' : status.max_vibration_rms < 7.1 ? '#fbbf24' : '#f87171'}">${status.max_vibration_rms.toFixed(2)}mm/s</span>
            </div>
            <div class="info-item">
                <span class="info-label">最高温度</span>
                <span class="info-value" style="color:${status.max_temperature < 60 ? '#4ade80' : '#fbbf24'}">${status.max_temperature.toFixed(1)}°C</span>
            </div>
            <div class="info-item">
                <span class="info-label">状态</span>
                <span class="info-value" style="color:${alarm.color}">${alarm.text}</span>
            </div>
        `;
    }

    updateSpindleCanvas(status) {
        if (!window.spindleCanvas) return;
        
        const vibrationData = this.sensorPositions.map((pos, i) => ({
            rms: this.generateSensorRMS(pos.id, status)
        }));
        
        window.spindleCanvas.updateAllSensors(vibrationData);
    }

    generateSensorRMS(sensorId, status) {
        const hasFault = [5, 12, 28, 35].includes(this.currentMachineId);
        const base = hasFault ? 3.5 : 1.5;
        const variation = Math.sin(Date.now() / 1000 + sensorId) * 0.3;
        let rms = base + variation + (sensorId % 3) * 0.2;
        
        if (hasFault && (sensorId === 1 || sensorId === 2 || sensorId === 6)) {
            rms *= 1.8;
        }
        
        return rms;
    }

    updateCharts() {
        if (window.waveformChart) {
            window.waveformChart.generateMockWaveform();
        }
        if (window.spectrumChart) {
            window.spectrumChart.generateMockSpectrum();
        }
        if (window.waterfallChart && window.waterfallChart.frames.length === 0) {
            window.waterfallChart.generateMockFrames();
        }
        if (window.rulChart) {
            window.rulChart.generateMockRUL();
        }
    }
}

function openSensorModal(sensorPos, rmsValue) {
    const modal = document.getElementById('sensor-modal');
    const title = document.getElementById('modal-title');
    const body = document.getElementById('modal-body');
    
    title.textContent = `${sensorPos.name} (ID: ${sensorPos.id})`;
    
    const level = rmsValue < 2.8 ? { text: '正常', color: '#4ade80' } :
                  rmsValue < 7.1 ? { text: '预警', color: '#fbbf24' } :
                  { text: '告警', color: '#f87171' };
    
    body.innerHTML = `
        <div class="sensor-summary">
            <div class="summary-item">
                <div class="label">当前RMS值</div>
                <div class="value" style="color:${level.color}">${rmsValue.toFixed(3)} mm/s</div>
            </div>
            <div class="summary-item">
                <div class="label">状态</div>
                <div class="value" style="color:${level.color}">${level.text}</div>
            </div>
            <div class="summary-item">
                <div class="label">安装位置</div>
                <div class="value" style="font-size:14px">${sensorPos.location}</div>
            </div>
        </div>
        
        <div class="modal-chart">
            <h4>振动时域波形 (最近60秒)</h4>
            <canvas id="modal-waveform" width="620" height="100"></canvas>
        </div>
        
        <div class="modal-chart">
            <h4>振动频谱分析</h4>
            <canvas id="modal-spectrum" width="620" height="100"></canvas>
        </div>
        
        <div class="modal-chart">
            <h4>RMS历史趋势 (最近24小时)</h4>
            <canvas id="modal-trend" width="620" height="100"></canvas>
        </div>
    `;
    
    setTimeout(() => {
        const waveformCanvas = document.getElementById('modal-waveform');
        if (waveformCanvas) {
            const ctx = waveformCanvas.getContext('2d');
            drawMiniWaveform(ctx, waveformCanvas.width, waveformCanvas.height);
        }
        
        const spectrumCanvas = document.getElementById('modal-spectrum');
        if (spectrumCanvas) {
            const ctx = spectrumCanvas.getContext('2d');
            drawMiniSpectrum(ctx, spectrumCanvas.width, spectrumCanvas.height);
        }
        
        const trendCanvas = document.getElementById('modal-trend');
        if (trendCanvas) {
            const ctx = trendCanvas.getContext('2d');
            drawMiniTrend(ctx, trendCanvas.width, trendCanvas.height, rmsValue);
        }
    }, 50);
    
    modal.classList.add('active');
}

function closeSensorModal() {
    document.getElementById('sensor-modal').classList.remove('active');
}

function drawMiniWaveform(ctx, width, height) {
    ctx.clearRect(0, 0, width, height);
    
    ctx.strokeStyle = 'rgba(71, 85, 105, 0.3)';
    ctx.lineWidth = 1;
    for (let x = 0; x < width; x += 50) {
        ctx.beginPath();
        ctx.moveTo(x, 0);
        ctx.lineTo(x, height);
        ctx.stroke();
    }
    for (let y = 0; y < height; y += 25) {
        ctx.beginPath();
        ctx.moveTo(0, y);
        ctx.lineTo(width, y);
        ctx.stroke();
    }
    
    ctx.beginPath();
    ctx.strokeStyle = '#3b82f6';
    ctx.lineWidth = 1.5;
    for (let x = 0; x < width; x++) {
        const t = x / width * 2;
        const val = Math.sin(2 * Math.PI * 50 * t) * 0.3
                  + Math.sin(2 * Math.PI * 120 * t) * 0.2
                  + (Math.random() - 0.5) * 0.1;
        const y = height / 2 - val * height * 0.4;
        if (x === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
    }
    ctx.stroke();
}

function drawMiniSpectrum(ctx, width, height) {
    ctx.clearRect(0, 0, width, height);
    
    const bins = 60;
    const barWidth = width / bins - 1;
    
    for (let i = 0; i < bins; i++) {
        const freq = (i / bins) * 1000;
        let val = 0;
        if (freq > 40 && freq < 60) val += 0.8;
        if (freq > 110 && freq < 130) val += 0.6;
        if (freq > 290 && freq < 310) val += 0.4;
        val += Math.random() * 0.2;
        val = Math.min(1, val);
        
        const barHeight = val * (height - 10);
        const x = i * (width / bins);
        const y = height - barHeight;
        
        const hue = 240 - val * 200;
        ctx.fillStyle = `hsl(${hue}, 70%, 55%)`;
        ctx.fillRect(x, y, barWidth, barHeight);
    }
}

function drawMiniTrend(ctx, width, height, currentRms) {
    ctx.clearRect(0, 0, width, height);
    
    ctx.strokeStyle = 'rgba(234, 179, 8, 0.5)';
    ctx.lineWidth = 1;
    ctx.setLineDash([3, 3]);
    const warningY = height - (2.8 / 10) * height;
    ctx.beginPath();
    ctx.moveTo(0, warningY);
    ctx.lineTo(width, warningY);
    ctx.stroke();
    
    ctx.setLineDash([]);
    ctx.beginPath();
    ctx.strokeStyle = '#10b981';
    ctx.lineWidth = 2;
    
    const points = 50;
    for (let i = 0; i < points; i++) {
        const x = (i / (points - 1)) * width;
        const base = currentRms * 0.8;
        const variation = Math.sin(i * 0.3) * 0.3 + Math.random() * 0.2;
        const val = Math.max(0.5, base + variation);
        const y = height - (val / 10) * height;
        
        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
    }
    ctx.stroke();
    
    ctx.fillStyle = '#64748b';
    ctx.font = '9px Arial';
    ctx.textAlign = 'left';
    ctx.fillText('2.8mm/s 预警线', 5, warningY - 3);
}
