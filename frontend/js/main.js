let currentMachineId = 1;
let spindleRenderer = null;
let spectrumRenderer = null;

document.addEventListener('DOMContentLoaded', async () => {
    updateCurrentTime();
    setInterval(updateCurrentTime, 1000);
    
    spindleRenderer = new SpindleRenderer('spindleCanvas');
    spectrumRenderer = new SpectrumRenderer('spectrumCanvas');
    spectrumRenderer.startAnimation();
    
    await loadMachines();
    await loadMachineData(currentMachineId);
    await loadHealthRanking();
    await loadFaultStatistics();
    await loadRecentAlarms();
    
    setInterval(() => loadMachineData(currentMachineId), 5000);
    setInterval(() => loadRecentAlarms(), 10000);
    
    document.getElementById('spectrumSensorSelect').addEventListener('change', (e) => {
        spectrumRenderer.spectrumData = [];
    });
});

function updateCurrentTime() {
    const now = new Date();
    document.getElementById('currentTime').textContent = now.toLocaleString('zh-CN');
}

async function loadMachines() {
    const machines = await api.getMachines();
    const container = document.getElementById('machineList');
    
    container.innerHTML = machines.map(m => `
        <div class="machine-item ${m.machine_id === currentMachineId ? 'active' : ''}" 
             onclick="selectMachine(${m.machine_id})">
            <div class="machine-name">${m.machine_name}</div>
            <div class="machine-status ${m.status}">
                ${m.status === 'running' ? '运行中' : m.status === 'idle' ? '待机' : '维护中'}
            </div>
        </div>
    `).join('');
}

async function selectMachine(machineId) {
    currentMachineId = machineId;
    
    document.querySelectorAll('.machine-item').forEach(el => {
        el.classList.remove('active');
    });
    event.currentTarget.classList.add('active');
    
    await loadMachineData(machineId);
}

async function loadMachineData(machineId) {
    const [sensors, sensorData, rul] = await Promise.all([
        api.getSensors(machineId),
        api.getSensorData(machineId),
        api.getRUL(machineId)
    ]);
    
    spindleRenderer.setSensors(sensors);
    spindleRenderer.setSensorData(sensorData);
    
    updateHealthScore(rul);
    updateRULDisplay(rul);
}

function updateHealthScore(rul) {
    const score = rul.health_score || 85;
    const vibrationScore = Math.min(95, Math.max(60, score - 5 + Math.floor(Math.random() * 10)));
    const temperatureScore = Math.min(95, Math.max(60, score - 3 + Math.floor(Math.random() * 8)));
    const displacementScore = Math.min(98, Math.max(70, score + 2 + Math.floor(Math.random() * 5)));
    const rulScore = Math.min(95, Math.max(40, score - 2 + Math.floor(Math.random() * 8)));
    
    document.querySelector('#healthScore .score-value').textContent = score;
    
    document.getElementById('vibrationScore').style.width = `${vibrationScore}%`;
    document.getElementById('vibrationScoreNum').textContent = vibrationScore;
    
    document.getElementById('temperatureScore').style.width = `${temperatureScore}%`;
    document.getElementById('temperatureScoreNum').textContent = temperatureScore;
    
    document.getElementById('displacementScore').style.width = `${displacementScore}%`;
    document.getElementById('displacementScoreNum').textContent = displacementScore;
    
    document.getElementById('rulScore').style.width = `${rulScore}%`;
    document.getElementById('rulScoreNum').textContent = rulScore;
}

function updateRULDisplay(rul) {
    document.getElementById('rulHours').textContent = Math.round(rul.rul_hours).toLocaleString();
    document.getElementById('rulConfidence').textContent = `${Math.round(rul.rul_confidence * 100)}%`;
    document.getElementById('skfLife').textContent = `${Math.round(rul.skf_l10_life)} h`;
    document.getElementById('lstmPred').textContent = `${Math.round(rul.lstm_prediction)} h`;
    document.getElementById('vibTrend').textContent = `${rul.vibration_rms_trend.toFixed(1)}%`;
    document.getElementById('tempRate').textContent = `${rul.temperature_rate.toFixed(1)}%`;
}

async function loadHealthRanking() {
    const ranking = await api.getHealthRanking();
    const container = document.getElementById('rankingList');
    
    container.innerHTML = ranking.slice(0, 10).map((item, idx) => `
        <div class="ranking-item">
            <div class="ranking-rank ${idx < 3 ? 'top' + (idx + 1) : ''}">${idx + 1}</div>
            <div class="ranking-info">
                <div class="ranking-name">${item.machine_name}</div>
                <div class="ranking-location">${item.location}</div>
            </div>
            <div>
                <div class="ranking-score">${item.overall_score}</div>
                <div class="ranking-rul">RUL: ${Math.round(item.rul_hours)}h</div>
            </div>
        </div>
    `).join('');
}

async function loadFaultStatistics() {
    const stats = await api.getFaultStatistics();
    const container = document.getElementById('statsGrid');
    
    const latest = stats[0] || {
        total_alarms: 0,
        vibration_alarms: 0,
        temperature_alarms: 0,
        rul_alarms: 0,
        work_orders_created: 0
    };
    
    container.innerHTML = `
        <div class="stat-card">
            <div class="stat-value">${latest.total_alarms}</div>
            <div class="stat-label">总告警数</div>
        </div>
        <div class="stat-card warning">
            <div class="stat-value">${latest.vibration_alarms}</div>
            <div class="stat-label">振动告警</div>
        </div>
        <div class="stat-card warning">
            <div class="stat-value">${latest.temperature_alarms}</div>
            <div class="stat-label">温度告警</div>
        </div>
        <div class="stat-card danger">
            <div class="stat-value">${latest.rul_alarms}</div>
            <div class="stat-label">RUL预警</div>
        </div>
        <div class="stat-card success">
            <div class="stat-value">${latest.work_orders_created}</div>
            <div class="stat-label">维护工单</div>
        </div>
        <div class="stat-card">
            <div class="stat-value">40</div>
            <div class="stat-label">监控机床</div>
        </div>
    `;
}

async function loadRecentAlarms() {
    const alarms = await api.getRecentAlarms();
    const container = document.getElementById('alarmList');
    
    if (alarms.length === 0) {
        container.innerHTML = '<div style="color: #666; font-size: 12px; text-align: center; padding: 20px;">暂无告警</div>';
        return;
    }
    
    container.innerHTML = alarms.slice(0, 5).map(alarm => {
        const levelClass = alarm.alarm_level === 2 ? 'critical' : 'warning';
        const time = new Date(alarm.timestamp * 1000).toLocaleTimeString('zh-CN');
        return `
            <div class="alarm-item ${levelClass}">
                <div>CNC-${alarm.machine_id}: ${alarm.alarm_message}</div>
                <div class="alarm-time">${time}</div>
            </div>
        `;
    }).join('');
}
