class SpindleCanvas {
    constructor(canvasId) {
        this.canvas = document.getElementById(canvasId);
        this.ctx = this.canvas.getContext('2d');
        this.sensorPositions = [];
        this.sensorData = new Map();
        this.selectedSensor = null;
        this.onSensorClick = null;
        
        this.init();
    }

    init() {
        this.canvas.addEventListener('click', (e) => this.handleClick(e));
        this.canvas.addEventListener('mousemove', (e) => this.handleMouseMove(e));
    }

    setSensorPositions(positions) {
        this.sensorPositions = positions;
        this.draw();
    }

    updateSensorData(sensorId, rmsValue) {
        this.sensorData.set(sensorId, rmsValue);
        this.draw();
    }

    updateAllSensors(vibrationData) {
        this.sensorData.clear();
        if (vibrationData) {
            vibrationData.forEach((sensor, index) => {
                this.sensorData.set(index + 1, sensor.rms || 2.0);
            });
        }
        this.draw();
    }

    getSensorColor(rms) {
        if (rms < 2.8) return { fill: '#22c55e', stroke: '#16a34a', glow: 'rgba(34, 197, 94, 0.5)' };
        if (rms < 7.1) return { fill: '#eab308', stroke: '#ca8a04', glow: 'rgba(234, 179, 8, 0.5)' };
        return { fill: '#ef4444', stroke: '#dc2626', glow: 'rgba(239, 68, 68, 0.6)' };
    }

    draw() {
        const ctx = this.ctx;
        const width = this.canvas.width;
        const height = this.canvas.height;
        
        ctx.clearRect(0, 0, width, height);
        
        this.drawSpindleOutline();
        this.drawBearingHousings();
        this.drawSensors();
    }

    drawSpindleOutline() {
        const ctx = this.ctx;
        const centerY = 100;
        
        ctx.save();
        
        const gradient = ctx.createLinearGradient(0, centerY - 30, 0, centerY + 30);
        gradient.addColorStop(0, '#64748b');
        gradient.addColorStop(0.3, '#94a3b8');
        gradient.addColorStop(0.7, '#94a3b8');
        gradient.addColorStop(1, '#475569');
        
        ctx.fillStyle = gradient;
        ctx.strokeStyle = '#334155';
        ctx.lineWidth = 2;
        
        ctx.beginPath();
        ctx.moveTo(20, centerY - 20);
        ctx.lineTo(60, centerY - 25);
        ctx.lineTo(150, centerY - 28);
        ctx.lineTo(220, centerY - 30);
        ctx.lineTo(300, centerY - 30);
        ctx.lineTo(380, centerY - 28);
        ctx.lineTo(450, centerY - 25);
        ctx.lineTo(450, centerY + 25);
        ctx.lineTo(380, centerY + 28);
        ctx.lineTo(300, centerY + 30);
        ctx.lineTo(220, centerY + 30);
        ctx.lineTo(150, centerY + 28);
        ctx.lineTo(60, centerY + 25);
        ctx.lineTo(20, centerY + 20);
        ctx.closePath();
        ctx.fill();
        ctx.stroke();
        
        ctx.fillStyle = '#475569';
        ctx.beginPath();
        ctx.ellipse(20, centerY, 8, 20, 0, 0, Math.PI * 2);
        ctx.fill();
        ctx.stroke();
        
        ctx.fillStyle = '#1e293b';
        ctx.beginPath();
        ctx.ellipse(20, centerY, 5, 12, 0, 0, Math.PI * 2);
        ctx.fill();
        
        ctx.fillStyle = '#0f172a';
        for (let x = 80; x < 440; x += 40) {
            ctx.fillRect(x, centerY - 2, 20, 4);
        }
        
        ctx.fillStyle = '#0ea5e9';
        ctx.font = '11px "Microsoft YaHei"';
        ctx.fillText('刀具接口', 5, centerY - 35);
        ctx.fillText('电机端', 430, centerY - 35);
        
        ctx.restore();
    }

    drawBearingHousings() {
        const ctx = this.ctx;
        const centerY = 100;
        const positions = [120, 250, 380];
        const labels = ['前轴承座', '中间支撑', '后轴承座'];
        
        positions.forEach((x, i) => {
            ctx.save();
            
            const gradient = ctx.createLinearGradient(x - 30, centerY - 45, x - 30, centerY + 45);
            gradient.addColorStop(0, '#475569');
            gradient.addColorStop(0.5, '#64748b');
            gradient.addColorStop(1, '#334155');
            
            ctx.fillStyle = gradient;
            ctx.strokeStyle = '#1e293b';
            ctx.lineWidth = 2;
            
            ctx.beginPath();
            ctx.roundRect(x - 35, centerY - 42, 70, 84, 8);
            ctx.fill();
            ctx.stroke();
            
            ctx.fillStyle = '#1e293b';
            ctx.beginPath();
            ctx.ellipse(x, centerY, 28, 32, 0, 0, Math.PI * 2);
            ctx.fill();
            
            ctx.fillStyle = '#f59e0b';
            ctx.beginPath();
            ctx.ellipse(x, centerY, 6, 8, 0, 0, Math.PI * 2);
            ctx.fill();
            
            for (let j = 0; j < 8; j++) {
                const angle = (j / 8) * Math.PI * 2;
                const bx = x + Math.cos(angle) * 18;
                const by = centerY + Math.sin(angle) * 22;
                ctx.fillStyle = '#94a3b8';
                ctx.beginPath();
                ctx.arc(bx, by, 4, 0, Math.PI * 2);
                ctx.fill();
            }
            
            ctx.fillStyle = '#94a3b8';
            ctx.font = '10px "Microsoft YaHei"';
            ctx.textAlign = 'center';
            ctx.fillText(labels[i], x, centerY + 58);
            ctx.textAlign = 'left';
            
            ctx.restore();
        });
    }

    drawSensors() {
        const ctx = this.ctx;
        
        this.sensorPositions.forEach((pos, index) => {
            const sensorId = pos.id;
            const rms = this.sensorData.get(sensorId) || 1.5;
            const colors = this.getSensorColor(rms);
            const isSelected = this.selectedSensor === sensorId;
            
            ctx.save();
            
            if (isSelected) {
                ctx.shadowColor = colors.glow;
                ctx.shadowBlur = 20;
            }
            
            ctx.beginPath();
            ctx.arc(pos.x, pos.y, isSelected ? 12 : 10, 0, Math.PI * 2);
            ctx.fillStyle = colors.fill;
            ctx.fill();
            ctx.strokeStyle = colors.stroke;
            ctx.lineWidth = isSelected ? 3 : 2;
            ctx.stroke();
            
            ctx.fillStyle = '#fff';
            ctx.font = 'bold 10px Arial';
            ctx.textAlign = 'center';
            ctx.textBaseline = 'middle';
            ctx.fillText(sensorId.toString(), pos.x, pos.y);
            
            if (rms >= 7.1) {
                const pulseSize = 12 + Math.sin(Date.now() / 200) * 3;
                ctx.beginPath();
                ctx.arc(pos.x, pos.y, pulseSize, 0, Math.PI * 2);
                ctx.strokeStyle = `rgba(239, 68, 68, ${0.5 + Math.sin(Date.now() / 200) * 0.3})`;
                ctx.lineWidth = 2;
                ctx.stroke();
            }
            
            ctx.restore();
        });
        
        if (this.hoveredSensor) {
            const pos = this.sensorPositions.find(p => p.id === this.hoveredSensor);
            if (pos) {
                this.drawTooltip(pos);
            }
        }
    }

    drawTooltip(pos) {
        const ctx = this.ctx;
        const rms = this.sensorData.get(pos.id) || 1.5;
        const colors = this.getSensorColor(rms);
        
        const tooltipX = pos.x + 15;
        const tooltipY = pos.y - 30;
        const padding = 8;
        const fontSize = 11;
        
        ctx.font = `${fontSize}px "Microsoft YaHei"`;
        const textWidth = Math.max(
            ctx.measureText(pos.name).width,
            ctx.measureText(`RMS: ${rms.toFixed(2)} mm/s`).width
        );
        
        ctx.save();
        ctx.fillStyle = 'rgba(15, 23, 42, 0.95)';
        ctx.strokeStyle = colors.stroke;
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.roundRect(tooltipX, tooltipY, textWidth + padding * 2, 48, 6);
        ctx.fill();
        ctx.stroke();
        
        ctx.fillStyle = '#f1f5f9';
        ctx.fillText(pos.name, tooltipX + padding, tooltipY + padding + fontSize);
        
        ctx.fillStyle = colors.fill;
        ctx.fillText(`RMS: ${rms.toFixed(2)} mm/s`, tooltipX + padding, tooltipY + padding + fontSize * 2 + 4);
        
        ctx.fillStyle = '#64748b';
        ctx.font = '10px "Microsoft YaHei"';
        ctx.fillText(pos.location, tooltipX + padding, tooltipY + padding + fontSize * 3 + 6);
        
        ctx.restore();
    }

    handleClick(e) {
        const rect = this.canvas.getBoundingClientRect();
        const scaleX = this.canvas.width / rect.width;
        const scaleY = this.canvas.height / rect.height;
        const x = (e.clientX - rect.left) * scaleX;
        const y = (e.clientY - rect.top) * scaleY;
        
        for (const pos of this.sensorPositions) {
            const dist = Math.sqrt((x - pos.x) ** 2 + (y - pos.y) ** 2);
            if (dist < 15) {
                this.selectedSensor = pos.id;
                if (this.onSensorClick) {
                    this.onSensorClick(pos, this.sensorData.get(pos.id) || 1.5);
                }
                this.draw();
                return;
            }
        }
        
        this.selectedSensor = null;
        this.draw();
    }

    handleMouseMove(e) {
        const rect = this.canvas.getBoundingClientRect();
        const scaleX = this.canvas.width / rect.width;
        const scaleY = this.canvas.height / rect.height;
        const x = (e.clientX - rect.left) * scaleX;
        const y = (e.clientY - rect.top) * scaleY;
        
        let found = null;
        for (const pos of this.sensorPositions) {
            const dist = Math.sqrt((x - pos.x) ** 2 + (y - pos.y) ** 2);
            if (dist < 15) {
                found = pos.id;
                break;
            }
        }
        
        if (found !== this.hoveredSensor) {
            this.hoveredSensor = found;
            this.canvas.style.cursor = found ? 'pointer' : 'default';
            this.draw();
        }
    }
}
