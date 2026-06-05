function closeSensorModal() {
    document.getElementById('sensorModal').classList.remove('show');
}

async function showSensorDetail(sensor) {
    const modal = document.getElementById('sensorModal');
    document.getElementById('modalTitle').textContent = `${sensor.position_name} - 传感器详情`;
    
    const data = await api.getSensorHistory(sensor.sensor_id);
    
    const infoHtml = `
        <div class="sensor-info-item">
            <div class="sensor-info-label">传感器ID</div>
            <div class="sensor-info-value">${sensor.sensor_id}</div>
        </div>
        <div class="sensor-info-item">
            <div class="sensor-info-label">类型</div>
            <div class="sensor-info-value">${sensor.sensor_type === 1 ? '振动' : (sensor.sensor_type === 2 ? '温度' : '位移')}</div>
        </div>
        <div class="sensor-info-item">
            <div class="sensor-info-label">单位</div>
            <div class="sensor-info-value">${data.sensor_config.unit}</div>
        </div>
        <div class="sensor-info-item">
            <div class="sensor-info-label">位置</div>
            <div class="sensor-info-value">(${sensor.position_x.toFixed(1)}, ${sensor.position_y.toFixed(1)}, ${sensor.position_z.toFixed(1)})</div>
        </div>
    `;
    document.getElementById('sensorInfo').innerHTML = infoHtml;
    
    drawTimeDomain(data.recent_data, sensor);
    drawFrequencyDomain(data.spectrum);
    drawHistoryTrend(data.history_trend, sensor);
    
    modal.classList.add('show');
}

function drawTimeDomain(data, sensor) {
    const canvas = document.getElementById('timeDomainCanvas');
    const ctx = canvas.getContext('2d');
    
    const w = canvas.width;
    const h = canvas.height;
    
    ctx.clearRect(0, 0, w, h);
    
    const padding = { top: 20, right: 20, bottom: 30, left: 50 };
    const chartWidth = w - padding.left - padding.right;
    const chartHeight = h - padding.top - padding.bottom;
    
    ctx.fillStyle = 'rgba(0, 0, 0, 0.3)';
    ctx.fillRect(padding.left, padding.top, chartWidth, chartHeight);
    
    if (data.length < 2) return;
    
    let minVal = Infinity, maxVal = -Infinity;
    data.forEach(d => {
        minVal = Math.min(minVal, d.value);
        maxVal = Math.max(maxVal, d.value);
    });
    
    const range = maxVal - minVal || 1;
    minVal -= range * 0.1;
    maxVal += range * 0.1;
    
    ctx.strokeStyle = 'rgba(0, 212, 255, 0.9)';
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    
    data.forEach((d, i) => {
        const x = padding.left + (i / (data.length - 1)) * chartWidth;
        const y = padding.top + chartHeight - ((d.value - minVal) / (maxVal - minVal)) * chartHeight;
        
        if (i === 0) {
            ctx.moveTo(x, y);
        } else {
            ctx.lineTo(x, y);
        }
    });
    ctx.stroke();
    
    ctx.fillStyle = 'rgba(0, 212, 255, 0.1)';
    ctx.lineTo(padding.left + chartWidth, padding.top + chartHeight);
    ctx.lineTo(padding.left, padding.top + chartHeight);
    ctx.closePath();
    ctx.fill();
    
    ctx.strokeStyle = 'rgba(255, 255, 255, 0.2)';
    ctx.lineWidth = 1;
    
    for (let i = 0; i <= 4; i++) {
        const y = padding.top + (i / 4) * chartHeight;
        const val = maxVal - (i / 4) * (maxVal - minVal);
        
        ctx.beginPath();
        ctx.moveTo(padding.left, y);
        ctx.lineTo(padding.left + chartWidth, y);
        ctx.stroke();
        
        ctx.fillStyle = 'rgba(255, 255, 255, 0.6)';
        ctx.font = '10px Arial';
        ctx.textAlign = 'right';
        ctx.fillText(val.toFixed(2), padding.left - 5, y + 3);
    }
    
    ctx.fillStyle = 'rgba(255, 255, 255, 0.6)';
    ctx.textAlign = 'center';
    ctx.fillText('时间 (近1小时)', padding.left + chartWidth / 2, h - 8);
    ctx.save();
    ctx.translate(15, padding.top + chartHeight / 2);
    ctx.rotate(-Math.PI / 2);
    ctx.fillText(`幅值 (${sensor.unit || ''})`, 0, 0);
    ctx.restore();
}

function drawFrequencyDomain(spectrum) {
    const canvas = document.getElementById('freqDomainCanvas');
    const ctx = canvas.getContext('2d');
    
    const w = canvas.width;
    const h = canvas.height;
    
    ctx.clearRect(0, 0, w, h);
    
    const padding = { top: 20, right: 20, bottom: 30, left: 50 };
    const chartWidth = w - padding.left - padding.right;
    const chartHeight = h - padding.top - padding.bottom;
    
    ctx.fillStyle = 'rgba(0, 0, 0, 0.3)';
    ctx.fillRect(padding.left, padding.top, chartWidth, chartHeight);
    
    if (!spectrum || !spectrum.frequency || spectrum.frequency.length < 2) return;
    
    const maxAmp = Math.max(...spectrum.amplitude) * 1.2;
    
    ctx.strokeStyle = 'rgba(124, 58, 237, 0.9)';
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    
    spectrum.amplitude.forEach((amp, i) => {
        const x = padding.left + (i / (spectrum.frequency.length - 1)) * chartWidth;
        const y = padding.top + chartHeight - (amp / maxAmp) * chartHeight;
        
        if (i === 0) {
            ctx.moveTo(x, y);
        } else {
            ctx.lineTo(x, y);
        }
    });
    ctx.stroke();
    
    ctx.fillStyle = 'rgba(124, 58, 237, 0.2)';
    ctx.lineTo(padding.left + chartWidth, padding.top + chartHeight);
    ctx.lineTo(padding.left, padding.top + chartHeight);
    ctx.closePath();
    ctx.fill();
    
    ctx.strokeStyle = 'rgba(255, 255, 255, 0.2)';
    ctx.lineWidth = 1;
    
    for (let i = 0; i <= 4; i++) {
        const y = padding.top + (i / 4) * chartHeight;
        const val = maxAmp - (i / 4) * maxAmp;
        
        ctx.beginPath();
        ctx.moveTo(padding.left, y);
        ctx.lineTo(padding.left + chartWidth, y);
        ctx.stroke();
        
        ctx.fillStyle = 'rgba(255, 255, 255, 0.6)';
        ctx.font = '10px Arial';
        ctx.textAlign = 'right';
        ctx.fillText(val.toFixed(1), padding.left - 5, y + 3);
    }
    
    const maxFreq = spectrum.frequency[spectrum.frequency.length - 1];
    for (let i = 0; i <= 4; i++) {
        const freq = (i / 4) * maxFreq;
        const x = padding.left + (i / 4) * chartWidth;
        
        ctx.fillStyle = 'rgba(255, 255, 255, 0.6)';
        ctx.textAlign = 'center';
        ctx.fillText(`${Math.round(freq)}Hz`, x, h - 8);
    }
    
    ctx.fillStyle = 'rgba(255, 255, 255, 0.6)';
    ctx.textAlign = 'center';
    ctx.fillText('频率 (Hz)', padding.left + chartWidth / 2, h - 8);
    ctx.save();
    ctx.translate(15, padding.top + chartHeight / 2);
    ctx.rotate(-Math.PI / 2);
    ctx.fillText('幅值', 0, 0);
    ctx.restore();
}

function drawHistoryTrend(data, sensor) {
    const canvas = document.getElementById('historyCanvas');
    const ctx = canvas.getContext('2d');
    
    const w = canvas.width;
    const h = canvas.height;
    
    ctx.clearRect(0, 0, w, h);
    
    const padding = { top: 20, right: 20, bottom: 30, left: 60 };
    const chartWidth = w - padding.left - padding.right;
    const chartHeight = h - padding.top - padding.bottom;
    
    ctx.fillStyle = 'rgba(0, 0, 0, 0.3)';
    ctx.fillRect(padding.left, padding.top, chartWidth, chartHeight);
    
    if (data.length < 2) return;
    
    let minVal = Infinity, maxVal = -Infinity;
    data.forEach(d => {
        minVal = Math.min(minVal, d.value);
        maxVal = Math.max(maxVal, d.value);
    });
    
    const range = maxVal - minVal || 1;
    minVal -= range * 0.1;
    maxVal += range * 0.1;
    
    const startTime = data[0].timestamp;
    const endTime = data[data.length - 1].timestamp;
    const timeRange = endTime - startTime || 1;
    
    if (sensor.sensor_type === 1) {
        ctx.strokeStyle = 'rgba(245, 158, 11, 0.5)';
        ctx.setLineDash([5, 5]);
        const y28 = padding.top + chartHeight - ((2.8 - minVal) / (maxVal - minVal)) * chartHeight;
        ctx.beginPath();
        ctx.moveTo(padding.left, y28);
        ctx.lineTo(padding.left + chartWidth, y28);
        ctx.stroke();
        
        const y71 = padding.top + chartHeight - ((7.1 - minVal) / (maxVal - minVal)) * chartHeight;
        ctx.beginPath();
        ctx.moveTo(padding.left, y71);
        ctx.lineTo(padding.left + chartWidth, y71);
        ctx.stroke();
        ctx.setLineDash([]);
    }
    
    ctx.strokeStyle = 'rgba(16, 185, 129, 0.9)';
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    
    data.forEach((d, i) => {
        const x = padding.left + ((d.timestamp - startTime) / timeRange) * chartWidth;
        const y = padding.top + chartHeight - ((d.value - minVal) / (maxVal - minVal)) * chartHeight;
        
        if (i === 0) {
            ctx.moveTo(x, y);
        } else {
            ctx.lineTo(x, y);
        }
    });
    ctx.stroke();
    
    ctx.strokeStyle = 'rgba(255, 255, 255, 0.2)';
    ctx.lineWidth = 1;
    
    for (let i = 0; i <= 4; i++) {
        const y = padding.top + (i / 4) * chartHeight;
        const val = maxVal - (i / 4) * (maxVal - minVal);
        
        ctx.beginPath();
        ctx.moveTo(padding.left, y);
        ctx.lineTo(padding.left + chartWidth, y);
        ctx.stroke();
        
        ctx.fillStyle = 'rgba(255, 255, 255, 0.6)';
        ctx.font = '10px Arial';
        ctx.textAlign = 'right';
        ctx.fillText(val.toFixed(2), padding.left - 5, y + 3);
    }
    
    ctx.fillStyle = 'rgba(255, 255, 255, 0.6)';
    ctx.textAlign = 'center';
    ctx.fillText('时间 (近7天)', padding.left + chartWidth / 2, h - 8);
    ctx.save();
    ctx.translate(20, padding.top + chartHeight / 2);
    ctx.rotate(-Math.PI / 2);
    ctx.fillText(`RMS值 (${sensor.unit || ''})`, 0, 0);
    ctx.restore();
    
    for (let i = 0; i <= 7; i++) {
        const x = padding.left + (i / 7) * chartWidth;
        const day = Math.floor((i / 7) * 7);
        
        ctx.fillStyle = 'rgba(255, 255, 255, 0.6)';
        ctx.textAlign = 'center';
        ctx.fillText(`-${7 - day}天`, x, padding.top + chartHeight + 15);
    }
}

document.getElementById('sensorModal').addEventListener('click', (e) => {
    if (e.target.id === 'sensorModal') {
        closeSensorModal();
    }
});

document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
        closeSensorModal();
    }
});

window.showSensorDetail = showSensorDetail;
