class SpindleProfile {
    constructor(canvasId) {
        this.canvas = document.getElementById(canvasId);
        this.ctx = this.canvas.getContext('2d');
        this.width = this.canvas.width;
        this.height = this.canvas.height;
        this.spindleData = null;
        this.selectedSensor = -1;
        this.dirtyRects = [];

        this.initStyles();
        this.bindEvents();
    }

    initStyles() {
        this.colors = {
            spindleBody: '#4a5568',
            spindleHighlight: '#718096',
            bearing: '#2d3748',
            sensorNormal: '#48bb78',
            sensorWarning: '#ecc94b',
            sensorAlarm: '#f56565',
            sensorSelected: '#3182ce',
            label: '#e2e8f0',
            grid: '#2d3748',
            tooltip: '#1a202c',
            tooltipBorder: '#4a5568'
        };

        this.fonts = {
            label: '12px sans-serif',
            small: '10px sans-serif',
            tooltip: '11px sans-serif'
        };
    }

    bindEvents() {
        this.canvas.addEventListener('click', (e) => this.handleClick(e));
        this.canvas.addEventListener('mousemove', (e) => this.handleMouseMove(e));
        this.canvas.addEventListener('mouseleave', () => this.hideTooltip());
    }

    update(spindleData) {
        this.spindleData = spindleData;
        this.render();
    }

    selectSensor(sensorIndex) {
        this.selectedSensor = sensorIndex;
        this.render();
    }

    handleClick(e) {
        const rect = this.canvas.getBoundingClientRect();
        const x = e.clientX - rect.left;
        const y = e.clientY - rect.top;
        
        const sensorPositions = this.getSensorPositions();
        for (let i = 0; i < sensorPositions.length; i++) {
            const pos = sensorPositions[i];
            const dist = Math.sqrt((x - pos.x) ** 2 + (y - pos.y) ** 2);
            if (dist < 12) {
                this.selectedSensor = i;
                if (this.onSensorSelect) {
                    this.onSensorSelect(i);
                }
                break;
            }
        }
    }

    handleMouseMove(e) {
        const rect = this.canvas.getBoundingClientRect();
        const x = e.clientX - rect.left;
        const y = e.clientY - rect.top;
        
        const sensorPositions = this.getSensorPositions();
        let hovered = false;
        
        for (let i = 0; i < sensorPositions.length; i++) {
            const pos = sensorPositions[i];
            const dist = Math.sqrt((x - pos.x) ** 2 + (y - pos.y) ** 2);
            if (dist < 12) {
                this.showSensorTooltip(i, e.clientX, e.clientY);
                hovered = true;
                break;
            }
        }
        
        if (!hovered) {
            this.hideTooltip();
        }
    }

    getSensorPositions() {
        const positions = [];
        const sensorSpacing = this.width / 10;
        const startX = sensorSpacing * 1.5;
        const yTop = this.height * 0.25;
        const yBottom = this.height * 0.75;

        for (let i = 0; i < 4; i++) {
            positions.push({ x: startX + i * sensorSpacing * 2, y: yTop });
            positions.push({ x: startX + i * sensorSpacing * 2, y: yBottom });
        }
        
        return positions;
    }

    getSensorColor(sensorIndex) {
        if (!this.spindleData || !this.spindleData.sensorReadings) {
            return this.colors.sensorNormal;
        }
        
        const reading = this.spindleData.sensorReadings[sensorIndex];
        if (!reading) return this.colors.sensorNormal;

        const value = reading.rms || 0;
        if (value > 4.5) return this.colors.sensorAlarm;
        if (value > 2.8) return this.colors.sensorWarning;
        return this.colors.sensorNormal;
    }

    showSensorTooltip(sensorIndex, x, y) {
        let tooltip = document.getElementById('sensor-tooltip');
        if (!tooltip) {
            tooltip = document.createElement('div');
            tooltip.id = 'sensor-tooltip';
            tooltip.style.cssText = `
                position: fixed;
                background: ${this.colors.tooltip};
                border: 1px solid ${this.colors.tooltipBorder};
                color: ${this.colors.label};
                padding: 8px 12px;
                border-radius: 6px;
                pointer-events: none;
                z-index: 1000;
                font: ${this.fonts.tooltip};
                box-shadow: 0 4px 12px rgba(0,0,0,0.3);
            `;
            document.body.appendChild(tooltip);
        }

        const reading = this.spindleData?.sensorReadings?.[sensorIndex] || {};
        const types = ['振动', '振动', '振动', '振动', '温度', '温度', '温度', '温度', '位移', '位移'];
        const units = ['mm/s', 'mm/s', 'mm/s', 'mm/s', '°C', '°C', '°C', '°C', 'μm', 'μm'];
        const bearings = ['轴承A', '轴承B', '轴承C', '轴承D'];
        
        let label = '';
        if (sensorIndex < 4) {
            label = `${bearings[sensorIndex]} ${types[sensorIndex]}`;
        } else if (sensorIndex < 8) {
            label = `${types[sensorIndex]} 传感器${sensorIndex - 3}`;
        } else {
            label = `${types[sensorIndex]} 传感器${sensorIndex - 7}`;
        }

        const value = reading.rms || reading.value || 0;
        tooltip.innerHTML = `
            <div style="font-weight:600;margin-bottom:4px">${label}</div>
            <div>值: ${value.toFixed(2)} ${units[sensorIndex] || ''}</div>
            <div>状态: ${value > 4.5 ? '告警' : value > 2.8 ? '警告' : '正常'}</div>
        `;
        tooltip.style.left = (x + 15) + 'px';
        tooltip.style.top = (y + 15) + 'px';
        tooltip.style.display = 'block';
    }

    hideTooltip() {
        const tooltip = document.getElementById('sensor-tooltip');
        if (tooltip) {
            tooltip.style.display = 'none';
        }
    }

    addDirtyRect(x, y, w, h) {
        this.dirtyRects.push({
            x: Math.floor(x),
            y: Math.floor(y),
            w: Math.ceil(w),
            h: Math.ceil(h)
        });
    }

    mergeDirtyRects() {
        if (this.dirtyRects.length === 0) return;
        
        const merged = this.dirtyRects.reduce((acc, rect) => {
            const x1 = Math.min(acc.x, rect.x);
            const y1 = Math.min(acc.y, rect.y);
            const x2 = Math.max(acc.x + acc.w, rect.x + rect.w);
            const y2 = Math.max(acc.y + acc.h, rect.y + rect.h);
            return { x: x1, y: y1, w: x2 - x1, h: y2 - y1 };
        }, this.dirtyRects[0]);
        
        this.ctx.clearRect(merged.x, merged.y, merged.w, merged.h);
        this.dirtyRects = [];
    }

    render() {
        this.ctx.clearRect(0, 0, this.width, this.height);
        this.drawSpindle();
        this.drawSensors();
        this.drawLabels();
    }

    drawSpindle() {
        const centerY = this.height / 2;
        const spindleLength = this.width * 0.8;
        const spindleX = (this.width - spindleLength) / 2;

        const gradient = this.ctx.createLinearGradient(0, centerY - 30, 0, centerY + 30);
        gradient.addColorStop(0, this.colors.spindleHighlight);
        gradient.addColorStop(0.3, this.colors.spindleBody);
        gradient.addColorStop(0.7, this.colors.spindleBody);
        gradient.addColorStop(1, this.colors.spindleHighlight);

        this.ctx.fillStyle = gradient;
        this.ctx.beginPath();
        this.ctx.roundRect(spindleX, centerY - 30, spindleLength, 60, 8);
        this.ctx.fill();

        this.ctx.strokeStyle = '#2d3748';
        this.ctx.lineWidth = 2;
        this.ctx.stroke();

        this.drawBearing(spindleX + spindleLength * 0.15, centerY);
        this.drawBearing(spindleX + spindleLength * 0.4, centerY);
        this.drawBearing(spindleX + spindleLength * 0.65, centerY);
        this.drawBearing(spindleX + spindleLength * 0.85, centerY);

        this.ctx.fillStyle = this.colors.spindleHighlight;
        this.ctx.beginPath();
        this.ctx.arc(spindleX + spindleLength + 15, centerY, 20, 0, Math.PI * 2);
        this.ctx.fill();
        this.ctx.strokeStyle = '#2d3748';
        this.ctx.lineWidth = 2;
        this.ctx.stroke();
    }

    drawBearing(x, centerY) {
        this.ctx.fillStyle = this.colors.bearing;
        this.ctx.beginPath();
        this.ctx.roundRect(x - 18, centerY - 35, 36, 70, 4);
        this.ctx.fill();
        this.ctx.strokeStyle = '#1a202c';
        this.ctx.lineWidth = 1;
        this.ctx.stroke();

        this.ctx.strokeStyle = '#4a5568';
        this.ctx.lineWidth = 2;
        this.ctx.beginPath();
        this.ctx.moveTo(x - 14, centerY - 30);
        this.ctx.lineTo(x - 14, centerY + 30);
        this.ctx.moveTo(x + 14, centerY - 30);
        this.ctx.lineTo(x + 14, centerY + 30);
        this.ctx.stroke();
    }

    drawSensors() {
        const positions = this.getSensorPositions();
        const sensorTypes = ['V', 'V', 'V', 'V', 'T', 'T', 'T', 'T', 'D', 'D'];

        positions.forEach((pos, index) => {
            const isSelected = index === this.selectedSensor;
            const color = this.getSensorColor(index);
            const radius = isSelected ? 14 : 11;

            if (isSelected) {
                this.ctx.fillStyle = this.colors.sensorSelected + '40';
                this.ctx.beginPath();
                this.ctx.arc(pos.x, pos.y, radius + 6, 0, Math.PI * 2);
                this.ctx.fill();
            }

            this.ctx.fillStyle = color;
            this.ctx.beginPath();
            this.ctx.arc(pos.x, pos.y, radius, 0, Math.PI * 2);
            this.ctx.fill();

            this.ctx.strokeStyle = '#1a202c';
            this.ctx.lineWidth = isSelected ? 2 : 1;
            this.ctx.stroke();

            this.ctx.fillStyle = '#fff';
            this.ctx.font = this.fonts.small;
            this.ctx.textAlign = 'center';
            this.ctx.textBaseline = 'middle';
            this.ctx.fillText(sensorTypes[index] || '?', pos.x, pos.y);
        });
    }

    drawLabels() {
        const centerY = this.height / 2;
        const spindleLength = this.width * 0.8;
        const spindleX = (this.width - spindleLength) / 2;

        this.ctx.fillStyle = this.colors.label;
        this.ctx.font = this.fonts.label;
        this.ctx.textAlign = 'center';

        const bearingNames = ['轴承A', '轴承B', '轴承C', '轴承D'];
        const positions = this.getSensorPositions();
        
        for (let i = 0; i < 4; i++) {
            const pos = positions[i];
            this.ctx.fillText(bearingNames[i], pos.x, centerY - 55);
        }

        this.ctx.font = this.fonts.small;
        this.ctx.fillStyle = '#a0aec0';
        this.ctx.fillText('主轴剖面图', this.width / 2, 20);

        this.ctx.font = this.fonts.small;
        this.ctx.textAlign = 'left';
        const legendY = this.height - 15;
        
        const legendItems = [
            { color: this.colors.sensorNormal, label: '正常' },
            { color: this.colors.sensorWarning, label: '警告' },
            { color: this.colors.sensorAlarm, label: '告警' }
        ];
        
        let legendX = 15;
        legendItems.forEach(item => {
            this.ctx.fillStyle = item.color;
            this.ctx.beginPath();
            this.ctx.arc(legendX + 6, legendY, 6, 0, Math.PI * 2);
            this.ctx.fill();
            
            this.ctx.fillStyle = this.colors.label;
            this.ctx.fillText(item.label, legendX + 18, legendY + 4);
            legendX += 80;
        });
    }

    resize() {
        this.width = this.canvas.width;
        this.height = this.canvas.height;
        this.render();
    }

    destroy() {
        this.hideTooltip();
        const tooltip = document.getElementById('sensor-tooltip');
        if (tooltip) tooltip.remove();
    }
}

if (typeof module !== 'undefined' && module.exports) {
    module.exports = SpindleProfile;
}
