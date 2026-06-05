var SensorPanel = (function () {
    var panelContent;
    var canvases = {};
    var currentSensorId = null;

    function init() {
        panelContent = document.getElementById('sensor-panel-content');
    }

    function show(machine, sensorId) {
        currentSensorId = sensorId;
        var sensor = findSensor(machine, sensorId);
        if (!sensor) return;

        var statusClass = getStatusClass(sensor);
        var statusText = getStatusText(sensor);
        var locationName = getLocationName(sensor.location);

        var html = '';

        html += '<div class="sensor-info-section">';
        html += '<div class="sensor-info-grid">';
        html += '<div class="sensor-info-item"><span class="sensor-info-label">传感器ID</span><span class="sensor-info-value">' + sensor.id + '</span></div>';
        html += '<div class="sensor-info-item"><span class="sensor-info-label">类型</span><span class="sensor-info-value">' + getTypeName(sensor.type) + '</span></div>';
        html += '<div class="sensor-info-item"><span class="sensor-info-label">位置</span><span class="sensor-info-value">' + locationName + '</span></div>';
        html += '<div class="sensor-info-item"><span class="sensor-info-label">当前值</span><span class="sensor-info-value ' + statusClass + '">' + sensor.value.toFixed(2) + ' ' + sensor.unit + '</span></div>';
        if (sensor.type === 'vibration') {
            html += '<div class="sensor-info-item"><span class="sensor-info-label">RMS</span><span class="sensor-info-value ' + statusClass + '">' + (sensor.rms || 0).toFixed(2) + ' mm/s</span></div>';
            html += '<div class="sensor-info-item"><span class="sensor-info-label">峰值</span><span class="sensor-info-value">' + (sensor.peak || 0).toFixed(2) + ' mm/s</span></div>';
        }
        html += '<div class="sensor-info-item"><span class="sensor-info-label">状态</span><span class="sensor-info-value ' + statusClass + '">' + statusText + '</span></div>';
        html += '</div></div>';

        if (sensor.type === 'vibration') {
            html += '<div class="chart-section">';
            html += '<div class="chart-section-title">时域波形 (最近1小时)</div>';
            html += '<div class="chart-canvas-container"><canvas id="waveform-canvas"></canvas></div>';
            html += '</div>';

            html += '<div class="chart-section">';
            html += '<div class="chart-section-title">频谱分析 (FFT)</div>';
            html += '<div class="chart-canvas-container"><canvas id="freq-spectrum-canvas"></canvas></div>';
            html += '</div>';
        }

        html += '<div class="chart-section">';
        html += '<div class="chart-section-title">历史趋势 (7天)</div>';
        html += '<div class="chart-canvas-container"><canvas id="trend-canvas"></canvas></div>';
        html += '</div>';

        html += '<div class="health-indicators">';
        html += buildHealthIndicators(sensor);
        html += '</div>';

        panelContent.innerHTML = html;

        if (sensor.type === 'vibration') {
            drawWaveform(sensor);
            drawFreqSpectrum(sensor);
        }
        drawTrend(sensor);
    }

    function update(machine, sensorId) {
        if (sensorId !== currentSensorId) return;
        show(machine, sensorId);
    }

    function clear() {
        currentSensorId = null;
        panelContent.innerHTML =
            '<div class="no-sensor-selected">' +
            '<div class="no-sensor-icon">◉</div>' +
            '<div class="no-sensor-text">点击主轴图上的传感器节点<br>查看详细监测数据</div>' +
            '</div>';
        canvases = {};
    }

    function resize() {
        if (!currentSensorId) return;
        var machine = App.getSelectedMachine();
        if (machine) {
            show(machine, currentSensorId);
        }
    }

    function findSensor(machine, sensorId) {
        for (var i = 0; i < machine.sensors.length; i++) {
            if (machine.sensors[i].id === sensorId) return machine.sensors[i];
        }
        return null;
    }

    function getStatusClass(sensor) {
        if (sensor.type === 'vibration') {
            var v = sensor.rms !== undefined ? sensor.rms : sensor.value;
            if (v < 2.8) return 'status-good';
            if (v < 7.1) return 'status-warning';
            return 'status-danger';
        }
        if (sensor.type === 'temperature') {
            if (sensor.value < 45) return 'status-good';
            if (sensor.value < 65) return 'status-warning';
            return 'status-danger';
        }
        if (sensor.type === 'displacement') {
            if (sensor.value < 5) return 'status-good';
            if (sensor.value < 8) return 'status-warning';
            return 'status-danger';
        }
        return '';
    }

    function getStatusText(sensor) {
        if (sensor.type === 'vibration') {
            var v = sensor.rms !== undefined ? sensor.rms : sensor.value;
            if (v < 2.8) return '正常';
            if (v < 7.1) return '警告';
            return '危险';
        }
        if (sensor.type === 'temperature') {
            if (sensor.value < 45) return '正常';
            if (sensor.value < 65) return '警告';
            return '危险';
        }
        if (sensor.type === 'displacement') {
            if (sensor.value < 5) return '正常';
            if (sensor.value < 8) return '警告';
            return '危险';
        }
        return '未知';
    }

    function getTypeName(type) {
        var names = { vibration: '振动', temperature: '温度', displacement: '位移' };
        return names[type] || type;
    }

    function getLocationName(loc) {
        var names = {
            front_bearing: '前轴承',
            rear_bearing: '后轴承',
            mid_shaft: '主轴中段',
            motor: '电机',
            shaft_end: '轴端'
        };
        return names[loc] || loc;
    }

    function buildHealthIndicators(sensor) {
        var html = '';
        var thresholds = getThresholds(sensor);
        for (var i = 0; i < thresholds.length; i++) {
            var t = thresholds[i];
            var pct = Math.min(100, (sensor.value / t.max) * 100);
            var color = pct < t.warnPct ? '#00ff88' : pct < t.dangerPct ? '#ffaa00' : '#ff3366';
            html += '<div class="health-indicator-row">';
            html += '<span class="health-indicator-label">' + t.label + '</span>';
            html += '<div class="health-indicator-bar"><div class="health-indicator-fill" style="width:' + pct + '%;background:' + color + '"></div></div>';
            html += '<span class="health-indicator-value" style="color:' + color + '">' + sensor.value.toFixed(1) + '</span>';
            html += '</div>';
        }
        return html;
    }

    function getThresholds(sensor) {
        if (sensor.type === 'vibration') {
            return [
                { label: '振动 RMS', max: 10, warnPct: 28, dangerPct: 71 },
                { label: 'ISO 10816', max: 10, warnPct: 28, dangerPct: 71 }
            ];
        }
        if (sensor.type === 'temperature') {
            return [
                { label: '温度', max: 90, warnPct: 50, dangerPct: 72 }
            ];
        }
        if (sensor.type === 'displacement') {
            return [
                { label: '位移', max: 15, warnPct: 33, dangerPct: 53 }
            ];
        }
        return [];
    }

    function drawWaveform(sensor) {
        var c = document.getElementById('waveform-canvas');
        if (!c) return;
        var container = c.parentElement;
        var w = container.clientWidth;
        var h = container.clientHeight;
        c.width = w * window.devicePixelRatio;
        c.height = h * window.devicePixelRatio;
        c.style.width = w + 'px';
        c.style.height = h + 'px';
        var cx = c.getContext('2d');
        cx.setTransform(window.devicePixelRatio, 0, 0, window.devicePixelRatio, 0, 0);

        cx.fillStyle = '#0a0e17';
        cx.fillRect(0, 0, w, h);

        var m = { top: 10, right: 10, bottom: 20, left: 40 };
        var pw = w - m.left - m.right;
        var ph = h - m.top - m.bottom;

        drawGrid(cx, m, pw, ph, sensor.waveform);

        if (!sensor.waveform) return;
        var data = sensor.waveform;
        var maxVal = 0;
        for (var i = 0; i < data.length; i++) {
            if (Math.abs(data[i]) > maxVal) maxVal = Math.abs(data[i]);
        }
        maxVal = Math.max(maxVal, 1);

        cx.beginPath();
        for (var j = 0; j < data.length; j++) {
            var x = m.left + (j / data.length) * pw;
            var y = m.top + ph / 2 - (data[j] / maxVal) * (ph / 2) * 0.9;
            if (j === 0) cx.moveTo(x, y);
            else cx.lineTo(x, y);
        }
        cx.strokeStyle = '#3b82f6';
        cx.lineWidth = 1.2;
        cx.stroke();

        drawThresholdLines(cx, m, pw, ph, maxVal, sensor);
    }

    function drawFreqSpectrum(sensor) {
        var c = document.getElementById('freq-spectrum-canvas');
        if (!c) return;
        var container = c.parentElement;
        var w = container.clientWidth;
        var h = container.clientHeight;
        c.width = w * window.devicePixelRatio;
        c.height = h * window.devicePixelRatio;
        c.style.width = w + 'px';
        c.style.height = h + 'px';
        var cx = c.getContext('2d');
        cx.setTransform(window.devicePixelRatio, 0, 0, window.devicePixelRatio, 0, 0);

        cx.fillStyle = '#0a0e17';
        cx.fillRect(0, 0, w, h);

        var m = { top: 10, right: 10, bottom: 20, left: 40 };
        var pw = w - m.left - m.right;
        var ph = h - m.top - m.bottom;

        if (!sensor.spectrum) return;
        var data = sensor.spectrum;
        var maxVal = 0;
        for (var i = 0; i < data.length; i++) {
            if (data[i] > maxVal) maxVal = data[i];
        }
        maxVal = Math.max(maxVal, 1);

        cx.beginPath();
        cx.moveTo(m.left, m.top + ph);
        for (var j = 0; j < data.length; j++) {
            var x = m.left + (j / data.length) * pw;
            var y = m.top + ph - (data[j] / maxVal) * ph * 0.9;
            cx.lineTo(x, y);
        }
        cx.lineTo(m.left + pw, m.top + ph);
        cx.closePath();
        var grad = cx.createLinearGradient(0, m.top, 0, m.top + ph);
        grad.addColorStop(0, 'rgba(59, 130, 246, 0.4)');
        grad.addColorStop(1, 'rgba(59, 130, 246, 0.05)');
        cx.fillStyle = grad;
        cx.fill();

        cx.beginPath();
        for (var k = 0; k < data.length; k++) {
            var x2 = m.left + (k / data.length) * pw;
            var y2 = m.top + ph - (data[k] / maxVal) * ph * 0.9;
            if (k === 0) cx.moveTo(x2, y2);
            else cx.lineTo(x2, y2);
        }
        cx.strokeStyle = '#3b82f6';
        cx.lineWidth = 1.2;
        cx.stroke();

        var bearingFreqs = [
            { label: 'BPFI', freq: 156.2, color: '#ff3366' },
            { label: 'BPFO', freq: 103.8, color: '#ffaa00' },
            { label: 'BSF', freq: 67.5, color: '#3b82f6' },
            { label: 'FTF', freq: 11.7, color: '#00ff88' }
        ];
        var freqMax = 2000;
        for (var b = 0; b < bearingFreqs.length; b++) {
            var bf = bearingFreqs[b];
            var bx = m.left + (bf.freq / freqMax) * pw;
            if (bx > m.left && bx < m.left + pw) {
                cx.strokeStyle = bf.color;
                cx.lineWidth = 1;
                cx.setLineDash([3, 3]);
                cx.beginPath();
                cx.moveTo(bx, m.top);
                cx.lineTo(bx, m.top + ph);
                cx.stroke();
                cx.setLineDash([]);
                cx.fillStyle = bf.color;
                cx.font = '8px JetBrains Mono, monospace';
                cx.textAlign = 'center';
                cx.fillText(bf.label, bx, m.top + 9);
            }
        }

        cx.fillStyle = '#64748b';
        cx.font = '9px JetBrains Mono, monospace';
        cx.textAlign = 'center';
        for (var f = 0; f <= 2000; f += 500) {
            var fx = m.left + (f / freqMax) * pw;
            cx.fillText(f + '', fx, m.top + ph + 14);
        }
    }

    function drawTrend(sensor) {
        var c = document.getElementById('trend-canvas');
        if (!c) return;
        var container = c.parentElement;
        var w = container.clientWidth;
        var h = container.clientHeight;
        c.width = w * window.devicePixelRatio;
        c.height = h * window.devicePixelRatio;
        c.style.width = w + 'px';
        c.style.height = h + 'px';
        var cx = c.getContext('2d');
        cx.setTransform(window.devicePixelRatio, 0, 0, window.devicePixelRatio, 0, 0);

        cx.fillStyle = '#0a0e17';
        cx.fillRect(0, 0, w, h);

        var m = { top: 10, right: 10, bottom: 20, left: 40 };
        var pw = w - m.left - m.right;
        var ph = h - m.top - m.bottom;

        if (!sensor.trend) return;
        var data = sensor.trend;
        var maxVal = 0;
        for (var i = 0; i < data.length; i++) {
            if (data[i] > maxVal) maxVal = data[i];
        }
        maxVal = Math.max(maxVal * 1.2, 1);

        cx.beginPath();
        cx.moveTo(m.left, m.top + ph);
        var step = Math.max(1, Math.floor(data.length / pw));
        for (var j = 0; j < data.length; j += step) {
            var x = m.left + (j / data.length) * pw;
            var y = m.top + ph - (data[j] / maxVal) * ph * 0.9;
            cx.lineTo(x, y);
        }
        cx.lineTo(m.left + pw, m.top + ph);
        cx.closePath();
        var grad = cx.createLinearGradient(0, m.top, 0, m.top + ph);
        grad.addColorStop(0, 'rgba(0, 255, 136, 0.2)');
        grad.addColorStop(1, 'rgba(0, 255, 136, 0.02)');
        cx.fillStyle = grad;
        cx.fill();

        cx.beginPath();
        for (var k = 0; k < data.length; k += step) {
            var x2 = m.left + (k / data.length) * pw;
            var y2 = m.top + ph - (data[k] / maxVal) * ph * 0.9;
            if (k === 0) cx.moveTo(x2, y2);
            else cx.lineTo(x2, y2);
        }
        cx.strokeStyle = '#00ff88';
        cx.lineWidth = 1.5;
        cx.stroke();

        drawThresholdLines(cx, m, pw, ph, maxVal, sensor);

        cx.fillStyle = '#64748b';
        cx.font = '9px Inter, sans-serif';
        cx.textAlign = 'center';
        var days = ['7天前', '6天前', '5天前', '4天前', '3天前', '2天前', '1天前', '现在'];
        for (var d = 0; d < days.length; d++) {
            var dx = m.left + (d / (days.length - 1)) * pw;
            cx.fillText(days[d], dx, m.top + ph + 14);
        }
    }

    function drawGrid(cx, m, pw, ph, data) {
        cx.strokeStyle = 'rgba(45, 58, 77, 0.4)';
        cx.lineWidth = 0.5;
        for (var i = 0; i <= 4; i++) {
            var y = m.top + (ph / 4) * i;
            cx.beginPath();
            cx.moveTo(m.left, y);
            cx.lineTo(m.left + pw, y);
            cx.stroke();
        }
        for (var j = 0; j <= 8; j++) {
            var x = m.left + (pw / 8) * j;
            cx.beginPath();
            cx.moveTo(x, m.top);
            cx.lineTo(x, m.top + ph);
            cx.stroke();
        }
    }

    function drawThresholdLines(cx, m, pw, ph, maxVal, sensor) {
        var thresholds = [];
        if (sensor.type === 'vibration') {
            thresholds = [
                { value: 2.8, color: '#ffaa00', label: '2.8' },
                { value: 7.1, color: '#ff3366', label: '7.1' }
            ];
        } else if (sensor.type === 'temperature') {
            thresholds = [
                { value: 45, color: '#ffaa00', label: '45°C' },
                { value: 65, color: '#ff3366', label: '65°C' }
            ];
        } else if (sensor.type === 'displacement') {
            thresholds = [
                { value: 5, color: '#ffaa00', label: '5μm' },
                { value: 8, color: '#ff3366', label: '8μm' }
            ];
        }
        for (var i = 0; i < thresholds.length; i++) {
            var t = thresholds[i];
            var ty = m.top + ph - (t.value / maxVal) * ph * 0.9;
            if (ty > m.top && ty < m.top + ph) {
                cx.strokeStyle = t.color;
                cx.lineWidth = 1;
                cx.setLineDash([4, 4]);
                cx.beginPath();
                cx.moveTo(m.left, ty);
                cx.lineTo(m.left + pw, ty);
                cx.stroke();
                cx.setLineDash([]);
                cx.fillStyle = t.color;
                cx.font = '9px JetBrains Mono, monospace';
                cx.textAlign = 'right';
                cx.fillText(t.label, m.left - 4, ty + 3);
            }
        }
    }

    return {
        init: init,
        show: show,
        update: update,
        clear: clear,
        resize: resize
    };
})();

window.addEventListener('DOMContentLoaded', function () {
    SensorPanel.init();
});
