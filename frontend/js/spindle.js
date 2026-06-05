class SpindleRenderer {
    constructor(canvasId) {
        this.canvas = document.getElementById(canvasId);
        this.ctx = this.canvas.getContext('2d');
        this.sensors = [];
        this.sensorData = {};
        this.selectedSensor = null;
        this.animationTime = 0;
        
        this.setupCanvas();
        this.bindEvents();
    }

    setupCanvas() {
        const rect = this.canvas.parentElement.getBoundingClientRect();
        this.canvas.width = rect.width;
        this.canvas.height = 400;
    }

    setSensors(sensors) {
        this.sensors = sensors;
        this.draw();
    }

    setSensorData(data) {
        data.forEach(d => {
            this.sensorData[d.sensor_id] = d;
        });
        this.draw();
    }

    bindEvents() {
        this.canvas.addEventListener('click', (e) => this.handleClick(e));
        this.canvas.addEventListener('mousemove', (e) => this.handleHover(e));
        
        window.addEventListener('resize', () => {
            this.setupCanvas();
            this.draw();
        });
    }

    getSensorColor(value, sensorType) {
        if (sensorType === 1) {
            if (value < 2.8) return { fill: '#10b981', glow: 'rgba(16, 185, 129, 0.5)' };
            if (value < 7.1) return { fill: '#f59e0b', glow: 'rgba(245, 158, 11, 0.5)' };
            return { fill: '#ef4444', glow: 'rgba(239, 68, 68, 0.6)' };
        } else if (sensorType === 2) {
            if (value < 55) return { fill: '#10b981', glow: 'rgba(16, 185, 129, 0.5)' };
            if (value < 75) return { fill: '#f59e0b', glow: 'rgba(245, 158, 11, 0.5)' };
            return { fill: '#ef4444', glow: 'rgba(239, 68, 68, 0.6)' };
        }
        return { fill: '#7c3aed', glow: 'rgba(124, 58, 237, 0.5)' };
    }

    mapPosition(x, y, z) {
        const centerX = this.canvas.width / 2;
        const centerY = this.canvas.height / 2;
        const scale = 1.2;
        
        const mappedX = centerX + z * scale;
        const mappedY = centerY - x * scale * 0.5;
        
        return { x: mappedX, y: mappedY };
    }

    draw() {
        const ctx = this.ctx;
        const w = this.canvas.width;
        const h = this.canvas.height;
        
        ctx.clearRect(0, 0, w, h);
        
        ctx.fillStyle = 'rgba(0, 0, 0, 0.3)';
        ctx.fillRect(0, 0, w, h);
        
        ctx.strokeStyle = 'rgba(255, 255, 255, 0.03)';
        ctx.lineWidth = 1;
        for (let x = 0; x < w; x += 40) {
            ctx.beginPath();
            ctx.moveTo(x, 0);
            ctx.lineTo(x, h);
            ctx.stroke();
        }
        for (let y = 0; y < h; y += 40) {
            ctx.beginPath();
            ctx.moveTo(0, y);
            ctx.lineTo(w, y);
            ctx.stroke();
        }
        
        this.drawSpindleCrossSection();
        
        this.sensors.forEach(sensor => {
            this.drawSensor(sensor);
        });
        
        this.drawLabels();
    }

    drawSpindleCrossSection() {
        const ctx = this.ctx;
        const centerX = this.canvas.width / 2;
        const centerY = this.canvas.height / 2;
        
        const spindleLength = 350;
        const spindleRadius = 40;
        const leftX = centerX - spindleLength / 2;
        const rightX = centerX + spindleLength / 2;
        
        const gradient = ctx.createLinearGradient(leftX, centerY - spindleRadius, leftX, centerY + spindleRadius);
        gradient.addColorStop(0, 'rgba(100, 100, 120, 0.8)');
        gradient.addColorStop(0.5, 'rgba(150, 150, 170, 0.6)');
        gradient.addColorStop(1, 'rgba(80, 80, 100, 0.8)');
        
        ctx.fillStyle = gradient;
        ctx.strokeStyle = 'rgba(0, 212, 255, 0.5)';
        ctx.lineWidth = 2;
        
        ctx.beginPath();
        ctx.moveTo(leftX, centerY - spindleRadius);
        ctx.lineTo(rightX - 60, centerY - spindleRadius);
        ctx.quadraticCurveTo(rightX - 30, centerY - spindleRadius, rightX, centerY);
        ctx.quadraticCurveTo(rightX - 30, centerY + spindleRadius, rightX - 60, centerY + spindleRadius);
        ctx.lineTo(leftX, centerY + spindleRadius);
        ctx.closePath();
        ctx.fill();
        ctx.stroke();
        
        ctx.strokeStyle = 'rgba(255, 255, 255, 0.2)';
        ctx.lineWidth = 1;
        ctx.setLineDash([5, 5]);
        ctx.beginPath();
        ctx.moveTo(leftX, centerY);
        ctx.lineTo(rightX, centerY);
        ctx.stroke();
        ctx.setLineDash([]);
        
        const frontBearingX = centerX - 80;
        const rearBearingX = centerX + 80;
        
        this.drawBearing(frontBearingX, centerY, 35);
        this.drawBearing(rearBearingX, centerY, 35);
        
        ctx.fillStyle = 'rgba(124, 58, 237, 0.3)';
        ctx.strokeStyle = 'rgba(124, 58, 237, 0.6)';
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.arc(leftX + 30, centerY, 45, -Math.PI / 2, Math.PI / 2);
        ctx.lineTo(leftX + 30, centerY + 45);
        ctx.lineTo(leftX + 30, centerY - 45);
        ctx.closePath();
        ctx.fill();
        ctx.stroke();
        
        ctx.fillStyle = 'rgba(0, 212, 255, 0.3)';
        ctx.strokeStyle = 'rgba(0, 212, 255, 0.6)';
        ctx.beginPath();
        ctx.arc(rightX - 10, centerY, 20, 0, Math.PI * 2);
        ctx.fill();
        ctx.stroke();
    }

    drawBearing(x, y, radius) {
        const ctx = this.ctx;
        
        ctx.strokeStyle = 'rgba(245, 158, 11, 0.6)';
        ctx.lineWidth = 3;
        ctx.beginPath();
        ctx.arc(x, y, radius, 0, Math.PI * 2);
        ctx.stroke();
        
        ctx.strokeStyle = 'rgba(245, 158, 11, 0.3)';
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.arc(x, y, radius - 8, 0, Math.PI * 2);
        ctx.stroke();
        
        const ballCount = 8;
        const ballRadius = 4;
        for (let i = 0; i < ballCount; i++) {
            const angle = (i / ballCount) * Math.PI * 2;
            const bx = x + Math.cos(angle) * (radius - 5);
            const by = y + Math.sin(angle) * (radius - 5);
            
            ctx.fillStyle = 'rgba(200, 200, 220, 0.8)';
            ctx.beginPath();
            ctx.arc(bx, by, ballRadius, 0, Math.PI * 2);
            ctx.fill();
        }
    }

    drawSensor(sensor) {
        const ctx = this.ctx;
        const pos = this.mapPosition(sensor.position_x, sensor.position_y, sensor.position_z);
        const data = this.sensorData[sensor.sensor_id];
        const value = data ? data.value_rms : 2.0;
        
        const colors = this.getSensorColor(value, sensor.sensor_type);
        
        const radius = sensor.sensor_type === 1 ? 10 : (sensor.sensor_type === 2 ? 8 : 6);
        
        ctx.shadowColor = colors.glow;
        ctx.shadowBlur = 15;
        
        ctx.fillStyle = colors.fill;
        ctx.beginPath();
        ctx.arc(pos.x, pos.y, radius, 0, Math.PI * 2);
        ctx.fill();
        
        ctx.shadowBlur = 0;
        
        ctx.strokeStyle = 'rgba(255, 255, 255, 0.8)';
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.arc(pos.x, pos.y, radius, 0, Math.PI * 2);
        ctx.stroke();
        
        if (sensor.sensor_type === 1) {
            ctx.fillStyle = '#fff';
            ctx.font = 'bold 10px Arial';
            ctx.textAlign = 'center';
            ctx.textBaseline = 'middle';
            ctx.fillText(sensor.sensor_id, pos.x, pos.y);
        } else if (sensor.sensor_type === 2) {
            ctx.fillStyle = '#fff';
            ctx.font = 'bold 10px Arial';
            ctx.textAlign = 'center';
            ctx.textBaseline = 'middle';
            ctx.fillText('T', pos.x, pos.y);
        } else {
            ctx.fillStyle = '#fff';
            ctx.font = 'bold 10px Arial';
            ctx.textAlign = 'center';
            ctx.textBaseline = 'middle';
            ctx.fillText('D', pos.x, pos.y);
        }
        
        sensor._screenPos = pos;
        sensor._radius = radius;
    }

    drawLabels() {
        const ctx = this.ctx;
        const centerX = this.canvas.width / 2;
        const centerY = this.canvas.height / 2;
        
        ctx.fillStyle = 'rgba(255, 255, 255, 0.7)';
        ctx.font = '12px Arial';
        ctx.textAlign = 'center';
        
        ctx.fillText('电机端', centerX - 150, centerY - 60);
        ctx.fillText('前轴承', centerX - 80, centerY - 60);
        ctx.fillText('后轴承', centerX + 80, centerY - 60);
        ctx.fillText('刀具端', centerX + 150, centerY - 60);
    }

    handleClick(e) {
        const rect = this.canvas.getBoundingClientRect();
        const x = e.clientX - rect.left;
        const y = e.clientY - rect.top;
        
        for (const sensor of this.sensors) {
            if (sensor._screenPos) {
                const dx = x - sensor._screenPos.x;
                const dy = y - sensor._screenPos.y;
                const dist = Math.sqrt(dx * dx + dy * dy);
                
                if (dist <= sensor._radius + 5) {
                    this.selectedSensor = sensor;
                    if (window.showSensorDetail) {
                        window.showSensorDetail(sensor);
                    }
                    break;
                }
            }
        }
    }

    handleHover(e) {
        const rect = this.canvas.getBoundingClientRect();
        const x = e.clientX - rect.left;
        const y = e.clientY - rect.top;
        
        let hovered = false;
        for (const sensor of this.sensors) {
            if (sensor._screenPos) {
                const dx = x - sensor._screenPos.x;
                const dy = y - sensor._screenPos.y;
                const dist = Math.sqrt(dx * dx + dy * dy);
                
                if (dist <= sensor._radius + 5) {
                    this.canvas.style.cursor = 'pointer';
                    hovered = true;
                    break;
                }
            }
        }
        
        if (!hovered) {
            this.canvas.style.cursor = 'default';
        }
    }

    animate() {
        this.animationTime += 0.02;
        this.draw();
        requestAnimationFrame(() => this.animate());
    }
}
