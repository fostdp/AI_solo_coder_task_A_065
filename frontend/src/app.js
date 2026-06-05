var App = (function () {
    var API_BASE = 'http://localhost:8080/api';
    var WS_URL = 'ws://localhost:8080/ws';
    var MACHINE_COUNT = 40;
    var POLL_HEALTH_MS = 1000;
    var POLL_SENSOR_MS = 100;

    var state = {
        machines: {},
        selectedMachineId: null,
        selectedSensorId: null,
        ws: null,
        wsConnected: false,
        healthTimer: null,
        sensorTimer: null,
        alerts: [],
        alertCount: 0
    };

    var machineTypes = ['CNC-5AX', 'CNC-3AX', 'CNC-TURN', 'CNC-GRIND', 'CNC-DRILL'];

    function init() {
        initMachines();
        initUI();
        initWebSocket();
        startPolling();
        startClock();
        loadInitialData();
    }

    function initMachines() {
        for (var i = 1; i <= MACHINE_COUNT; i++) {
            var typeIdx = Math.floor(Math.random() * machineTypes.length);
            state.machines[i] = {
                id: i,
                name: 'CNC-' + String(i).padStart(3, '0'),
                type: machineTypes[typeIdx],
                healthScore: 70 + Math.floor(Math.random() * 30),
                rpm: 3000 + Math.floor(Math.random() * 9000),
                sensors: generateSensors(i),
                online: Math.random() > 0.1,
                lastUpdate: Date.now()
            };
        }
    }

    function generateSensors(machineId) {
        var sensors = [];
        var vibPositions = [
            { name: '前轴承-X', loc: 'front_bearing', axis: 'X', px: 0.15, py: 0.3 },
            { name: '前轴承-Y', loc: 'front_bearing', axis: 'Y', px: 0.15, py: 0.7 },
            { name: '后轴承-X', loc: 'rear_bearing', axis: 'X', px: 0.75, py: 0.3 },
            { name: '后轴承-Y', loc: 'rear_bearing', axis: 'Y', px: 0.75, py: 0.7 },
            { name: '主轴中段-X', loc: 'mid_shaft', axis: 'X', px: 0.42, py: 0.25 },
            { name: '主轴中段-Y', loc: 'mid_shaft', axis: 'Y', px: 0.42, py: 0.75 },
            { name: '电机-X', loc: 'motor', axis: 'X', px: 0.88, py: 0.3 },
            { name: '电机-Y', loc: 'motor', axis: 'Y', px: 0.88, py: 0.7 }
        ];
        for (var vi = 0; vi < vibPositions.length; vi++) {
            var vp = vibPositions[vi];
            sensors.push({
                id: 'V-' + machineId + '-' + (vi + 1),
                type: 'vibration',
                name: vp.name,
                location: vp.loc,
                axis: vp.axis,
                px: vp.px,
                py: vp.py,
                value: 1 + Math.random() * 5,
                rms: 1 + Math.random() * 5,
                peak: 2 + Math.random() * 10,
                unit: 'mm/s',
                waveform: generateWaveform(512),
                spectrum: generateSpectrum(256),
                trend: generateTrend(7 * 24)
            });
        }
        var tempPositions = [
            { name: '前轴承温度-1', loc: 'front_bearing', px: 0.15, py: 0.15 },
            { name: '前轴承温度-2', loc: 'front_bearing', px: 0.15, py: 0.85 },
            { name: '后轴承温度-1', loc: 'rear_bearing', px: 0.75, py: 0.15 },
            { name: '后轴承温度-2', loc: 'rear_bearing', px: 0.75, py: 0.85 }
        ];
        for (var ti = 0; ti < tempPositions.length; ti++) {
            var tp = tempPositions[ti];
            sensors.push({
                id: 'T-' + machineId + '-' + (ti + 1),
                type: 'temperature',
                name: tp.name,
                location: tp.loc,
                px: tp.px,
                py: tp.py,
                value: 35 + Math.random() * 30,
                unit: '°C',
                trend: generateTrend(7 * 24)
            });
        }
        var dispPositions = [
            { name: '轴端位移-X', loc: 'shaft_end', px: 0.04, py: 0.4 },
            { name: '轴端位移-Y', loc: 'shaft_end', px: 0.04, py: 0.6 }
        ];
        for (var di = 0; di < dispPositions.length; di++) {
            var dp = dispPositions[di];
            sensors.push({
                id: 'D-' + machineId + '-' + (di + 1),
                type: 'displacement',
                name: dp.name,
                location: dp.loc,
                px: dp.px,
                py: dp.py,
                value: 1 + Math.random() * 8,
                unit: 'μm',
                trend: generateTrend(7 * 24)
            });
        }
        return sensors;
    }

    function generateWaveform(len) {
        var data = new Float32Array(len);
        for (var i = 0; i < len; i++) {
            data[i] = Math.sin(i * 0.3) * 2 + Math.sin(i * 0.7) * 1.5 + (Math.random() - 0.5) * 2;
        }
        return data;
    }

    function generateSpectrum(len) {
        var data = new Float32Array(len);
        for (var i = 0; i < len; i++) {
            var f = i / len * 2000;
            data[i] = Math.exp(-f / 500) * 80 + Math.random() * 10;
            if (Math.abs(f - 156) < 5) data[i] += 40;
            if (Math.abs(f - 312) < 5) data[i] += 30;
            if (Math.abs(f - 624) < 5) data[i] += 20;
        }
        return data;
    }

    function generateTrend(len) {
        var data = new Float32Array(len);
        var base = 2 + Math.random() * 3;
        for (var i = 0; i < len; i++) {
            data[i] = base + Math.sin(i * 0.05) * 0.5 + (Math.random() - 0.5) * 0.8;
        }
        return data;
    }

    function initUI() {
        renderMachineList();
        selectMachine(1);

        document.getElementById('close-panel-btn').addEventListener('click', function () {
            state.selectedSensorId = null;
            SensorPanel.clear();
        });

        document.getElementById('alert-badge').addEventListener('click', function () {
            state.alerts = [];
            updateAlertBadge();
        });

        window.addEventListener('resize', debounce(function () {
            SpindleDiagram.resize();
            SpectrumChart.resize();
            RankingChart.resize();
            if (state.selectedSensorId) {
                SensorPanel.resize();
            }
        }, 200));
    }

    function renderMachineList() {
        var listEl = document.getElementById('machine-list');
        listEl.innerHTML = '';
        var keys = Object.keys(state.machines);
        document.getElementById('machine-count').textContent = keys.length + ' 台';
        var onlineCount = 0;

        for (var ki = 0; ki < keys.length; ki++) {
            var m = state.machines[keys[ki]];
            if (m.online) onlineCount++;
            var item = document.createElement('div');
            item.className = 'machine-item' + (m.id === state.selectedMachineId ? ' active' : '');
            if (!m.online) item.style.opacity = '0.5';
            if (m.healthScore < 60) item.classList.add('alert-active');
            item.dataset.machineId = m.id;

            var healthClass = m.healthScore > 80 ? 'good' : m.healthScore > 60 ? 'warning' : 'critical';

            item.innerHTML =
                '<div class="machine-info">' +
                '<div class="machine-name">' + m.name + '</div>' +
                '<div class="machine-type">' + m.type + ' | ' + m.rpm + ' RPM</div>' +
                '</div>' +
                '<span class="health-badge ' + healthClass + '">' + m.healthScore + '</span>';

            (function (id) {
                item.addEventListener('click', function () {
                    selectMachine(id);
                });
            })(m.id);

            listEl.appendChild(item);
        }

        document.getElementById('online-count').textContent = onlineCount;
    }

    function selectMachine(id) {
        state.selectedMachineId = id;
        state.selectedSensorId = null;
        var items = document.querySelectorAll('.machine-item');
        for (var i = 0; i < items.length; i++) {
            items[i].classList.toggle('active', parseInt(items[i].dataset.machineId) === id);
        }
        var machine = state.machines[id];
        if (machine) {
            SpindleDiagram.draw(machine);
            SpectrumChart.reset();
        }
        SensorPanel.clear();
    }

    function initWebSocket() {
        try {
            state.ws = new WebSocket(WS_URL);
            state.ws.onopen = function () {
                state.wsConnected = true;
                updateWsStatus(true);
            };
            state.ws.onmessage = function (event) {
                try {
                    var data = JSON.parse(event.data);
                    handleWsMessage(data);
                } catch (e) { }
            };
            state.ws.onclose = function () {
                state.wsConnected = false;
                updateWsStatus(false);
                setTimeout(initWebSocket, 5000);
            };
            state.ws.onerror = function () {
                state.wsConnected = false;
                updateWsStatus(false);
            };
        } catch (e) {
            state.wsConnected = false;
            updateWsStatus(false);
        }
    }

    function updateWsStatus(connected) {
        var dot = document.getElementById('ws-status-dot');
        var text = document.getElementById('ws-status-text');
        if (connected) {
            dot.classList.remove('disconnected');
            text.textContent = '已连接';
        } else {
            dot.classList.add('disconnected');
            text.textContent = '未连接';
        }
    }

    function handleWsMessage(data) {
        if (data.type === 'alert') {
            state.alerts.push(data);
            state.alertCount++;
            updateAlertBadge();
            showToast(data.message || '新告警', data.level || 'danger');
        } else if (data.type === 'sensor_update' && data.machineId && data.sensorId) {
            var m = state.machines[data.machineId];
            if (m) {
                for (var i = 0; i < m.sensors.length; i++) {
                    if (m.sensors[i].id === data.sensorId) {
                        if (data.value !== undefined) m.sensors[i].value = data.value;
                        if (data.rms !== undefined) m.sensors[i].rms = data.rms;
                        break;
                    }
                }
            }
        }
    }

    function startPolling() {
        state.healthTimer = setInterval(function () {
            pollHealthData();
        }, POLL_HEALTH_MS);

        state.sensorTimer = setInterval(function () {
            pollSensorData();
        }, POLL_SENSOR_MS);
    }

    function pollHealthData() {
        fetch(API_BASE + '/machines/health', { method: 'GET' })
            .then(function (res) { return res.json(); })
            .then(function (data) {
                if (Array.isArray(data)) {
                    for (var i = 0; i < data.length; i++) {
                        var d = data[i];
                        if (state.machines[d.id]) {
                            state.machines[d.id].healthScore = d.healthScore || state.machines[d.id].healthScore;
                            state.machines[d.id].rpm = d.rpm || state.machines[d.id].rpm;
                            state.machines[d.id].lastUpdate = Date.now();
                        }
                    }
                    renderMachineList();
                    RankingChart.draw(state.machines);
                }
            })
            .catch(function () {
                simulateHealthUpdate();
            });

        fetch(API_BASE + '/alerts/count', { method: 'GET' })
            .then(function (res) { return res.json(); })
            .then(function (data) {
                if (data.count !== undefined) {
                    state.alertCount = data.count;
                    updateAlertBadge();
                }
            })
            .catch(function () { });
    }

    function simulateHealthUpdate() {
        var keys = Object.keys(state.machines);
        for (var i = 0; i < keys.length; i++) {
            var m = state.machines[keys[i]];
            if (m.online) {
                m.healthScore = Math.max(10, Math.min(100, m.healthScore + (Math.random() - 0.48) * 2));
                m.healthScore = Math.round(m.healthScore);
                m.rpm = Math.max(0, m.rpm + Math.floor((Math.random() - 0.5) * 200));
            }
        }
        renderMachineList();
        RankingChart.draw(state.machines);
    }

    function pollSensorData() {
        var machine = state.machines[state.selectedMachineId];
        if (!machine) return;

        fetch(API_BASE + '/machines/' + state.selectedMachineId + '/sensors/realtime', { method: 'GET' })
            .then(function (res) { return res.json(); })
            .then(function (data) {
                if (Array.isArray(data)) {
                    updateSensorValues(machine, data);
                }
            })
            .catch(function () {
                simulateSensorUpdate(machine);
            });
    }

    function updateSensorValues(machine, data) {
        for (var i = 0; i < data.length; i++) {
            for (var j = 0; j < machine.sensors.length; j++) {
                if (machine.sensors[j].id === data[i].id) {
                    if (data[i].value !== undefined) machine.sensors[j].value = data[i].value;
                    if (data[i].rms !== undefined) machine.sensors[j].rms = data[i].rms;
                    if (data[i].waveform) machine.sensors[j].waveform = data[i].waveform;
                    if (data[i].spectrum) machine.sensors[j].spectrum = data[i].spectrum;
                    break;
                }
            }
        }
        SpindleDiagram.draw(machine);
        SpectrumChart.addData(machine.sensors[0] ? machine.sensors[0].spectrum : null);
        if (state.selectedSensorId) {
            SensorPanel.update(machine, state.selectedSensorId);
        }
    }

    function simulateSensorUpdate(machine) {
        for (var i = 0; i < machine.sensors.length; i++) {
            var s = machine.sensors[i];
            if (s.type === 'vibration') {
                s.value = Math.max(0.1, s.value + (Math.random() - 0.5) * 0.4);
                s.rms = s.value;
                s.peak = s.value * 1.8;
                s.waveform = generateWaveform(512);
                s.spectrum = generateSpectrum(256);
            } else if (s.type === 'temperature') {
                s.value = Math.max(20, Math.min(90, s.value + (Math.random() - 0.5) * 0.3));
            } else if (s.type === 'displacement') {
                s.value = Math.max(0.1, s.value + (Math.random() - 0.5) * 0.2);
            }
        }
        SpindleDiagram.draw(machine);
        SpectrumChart.addData(machine.sensors[0] ? machine.sensors[0].spectrum : null);
        if (state.selectedSensorId) {
            SensorPanel.update(machine, state.selectedSensorId);
        }
    }

    function loadInitialData() {
        fetch(API_BASE + '/machines', { method: 'GET' })
            .then(function (res) { return res.json(); })
            .then(function (data) {
                if (Array.isArray(data)) {
                    for (var i = 0; i < data.length; i++) {
                        if (state.machines[data[i].id]) {
                            Object.assign(state.machines[data[i].id], data[i]);
                        }
                    }
                    renderMachineList();
                }
            })
            .catch(function () { });

        fetch(API_BASE + '/faults/monthly', { method: 'GET' })
            .then(function (res) { return res.json(); })
            .then(function (data) {
                RankingChart.drawFaultStats(data);
            })
            .catch(function () {
                var simData = [
                    { type: '轴承外圈故障', count: 12 },
                    { type: '轴承内圈故障', count: 8 },
                    { type: '滚动体故障', count: 5 },
                    { type: '保持架故障', count: 3 },
                    { type: '不平衡', count: 7 },
                    { type: '不对中', count: 4 },
                    { type: '松动', count: 2 }
                ];
                RankingChart.drawFaultStats(simData);
            });

        fetch(API_BASE + '/alerts/timeline', { method: 'GET' })
            .then(function (res) { return res.json(); })
            .then(function (data) {
                RankingChart.drawAlertTimeline(data);
            })
            .catch(function () {
                var simTimeline = [];
                for (var d = 30; d >= 0; d--) {
                    simTimeline.push({
                        date: new Date(Date.now() - d * 86400000).toISOString().slice(0, 10),
                        count: Math.floor(Math.random() * 6)
                    });
                }
                RankingChart.drawAlertTimeline(simTimeline);
            });

        RankingChart.draw(state.machines);
    }

    function updateAlertBadge() {
        var badge = document.getElementById('alert-badge');
        var countEl = document.getElementById('alert-count');
        countEl.textContent = state.alertCount;
        badge.className = 'alert-badge';
        if (state.alertCount === 0) {
            badge.classList.add('normal');
        } else if (state.alertCount < 5) {
            badge.classList.add('warning');
        } else {
            badge.classList.add('danger');
        }
    }

    function startClock() {
        function tick() {
            var now = new Date();
            document.getElementById('current-time').textContent =
                now.getFullYear() + '-' +
                String(now.getMonth() + 1).padStart(2, '0') + '-' +
                String(now.getDate()).padStart(2, '0') + ' ' +
                String(now.getHours()).padStart(2, '0') + ':' +
                String(now.getMinutes()).padStart(2, '0') + ':' +
                String(now.getSeconds()).padStart(2, '0');
        }
        tick();
        setInterval(tick, 1000);
    }

    function showToast(message, level) {
        var container = document.getElementById('toast-container');
        var toast = document.createElement('div');
        toast.className = 'toast ' + (level || 'info');
        var icon = level === 'danger' ? '🔴' : level === 'warning' ? '🟡' : '🔵';
        toast.innerHTML = '<span>' + icon + '</span><span>' + message + '</span>';
        container.appendChild(toast);
        setTimeout(function () {
            toast.classList.add('fade-out');
            setTimeout(function () {
                if (toast.parentNode) toast.parentNode.removeChild(toast);
            }, 300);
        }, 4000);
    }

    function debounce(fn, ms) {
        var timer;
        return function () {
            clearTimeout(timer);
            timer = setTimeout(fn, ms);
        };
    }

    function getSelectedMachine() {
        return state.machines[state.selectedMachineId];
    }

    function selectSensor(sensorId) {
        state.selectedSensorId = sensorId;
        var machine = getSelectedMachine();
        if (machine) {
            SensorPanel.show(machine, sensorId);
        }
    }

    return {
        init: init,
        getSelectedMachine: getSelectedMachine,
        selectSensor: selectSensor,
        showToast: showToast,
        state: state
    };
})();

window.addEventListener('DOMContentLoaded', function () {
    App.init();
});
