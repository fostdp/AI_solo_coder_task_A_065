const SpindleCanvas = {
    canvas: null,
    ctx: null,
    width: 800,
    height: 400,
    sensorPositions: [],
    currentMachineId: 1,
    vibrationData: [],

    init() {
        this.canvas = document.getElementById('spindle-canvas');
        this.ctx = this.canvas.getContext('2d');
        this.setupSensorPositions();
        this.bindEvents();
        this.draw();
    },

    setupSensorPositions() {
        const cx = 400;
        const cy = 200;
        const spindleRadius = 120;
        const sensorCount = 8;

        this.sensorPositions = [];
        for (let i = 0; i < sensorCount; i++) {
            const angle = (i / sensorCount) * Math.PI * 2 - Math.PI / 2;
            const radius = spindleRadius + 30;
            this.sensorPositions.push({
                x: cx + Math.cos(angle) * radius,
                y: cy + Math.sin(angle) * radius,
                index: i,
                radius: 12
            });
        }

        const tempCount = 4;
        for (let i = 0; i < tempCount; i++) {
            const angle = (i / tempCount) * Math.PI * 2 - Math.PI / 4;
            const radius = spindleRadius - 40;
            this.sensorPositions.push({
                x: cx + Math.cos(angle) * radius,
                y: cy + Math.sin(angle) * radius,
                index: 8 + i,
                radius: 10,
                type: 'temp'
            });
        }
    },

    bindEvents() {
        this.canvas.addEventListener('click', (e) => {
            const rect = this.canvas.getBoundingClientRect();
            const scaleX = this.canvas.width / rect.width;
            const scaleY = this.canvas.height / rect.height;
            const x = (e.clientX - rect.left) * scaleX;
            const y = (e.clientY - rect.top) * scaleY;

            for (const sensor of this.sensorPositions) {
                const dx = x - sensor.x;
                const dy = y - sensor.y;
                if (Math.sqrt(dx * dx + dy * dy) < sensor.radius + 5) {
                    this.onSensorClick(sensor);
                    break;
                }
            }
        });
    },

    onSensorClick(sensor) {
        const event = new CustomEvent('sensorSelected', {
            detail: {
                machineId: this.currentMachineId,
                sensorIndex: sensor.index,
                type: sensor.type || 'vibration'
            }
        });
        document.dispatchEvent(event);
    },

    updateData(vibrationData) {
        this.vibrationData = vibrationData;
        this.draw();
    },

    getSensorColor(value, type = 'vibration') {
        if (type === 'temp') {
            if (value < 50) return '#4caf50';
            if (value < 70) return '#ffeb3b';
            return '#f44336';
        }
        
        if (value < CONFIG.VIBRATION_THRESHOLDS.GOOD) return '#4caf50';
        if (value < CONFIG.VIBRATION_THRESHOLDS.WARNING) return '#ffeb3b';
        return '#f44336';
    },

    draw() {
        const ctx = this.ctx;
        const cx = 400;
        const cy = 200;
        const spindleRadius = 120;

        ctx.clearRect(0, 0, this.width, this.height);

        const bgGradient = ctx.createRadialGradient(cx, cy, 0, cx, cy, spindleRadius + 60);
        bgGradient.addColorStop(0, '#0d2137');
        bgGradient.addColorStop(1, '#0a1929');
        ctx.fillStyle = bgGradient;
        ctx.fillRect(0, 0, this.width, this.height);

        ctx.beginPath();
        ctx.arc(cx, cy, spindleRadius, 0, Math.PI * 2);
        const spindleGradient = ctx.createRadialGradient(cx - 20, cy - 20, 0, cx, cy, spindleRadius);
        spindleGradient.addColorStop(0, '#546e7a');
        spindleGradient.addColorStop(0.5, '#37474f');
        spindleGradient.addColorStop(1, '#263238');
        ctx.fillStyle = spindleGradient;
        ctx.fill();
        ctx.strokeStyle = '#78909c';
        ctx.lineWidth = 3;
        ctx.stroke();

        ctx.beginPath();
        ctx.arc(cx, cy, 40, 0, Math.PI * 2);
        ctx.fillStyle = '#1a1a2e';
        ctx.fill();
        ctx.strokeStyle = '#455a64';
        ctx.lineWidth = 2;
        ctx.stroke();

        for (let i = 0; i < 6; i++) {
            const angle = (i / 6) * Math.PI * 2;
            const x1 = cx + Math.cos(angle) * 45;
            const y1 = cy + Math.sin(angle) * 45;
            const x2 = cx + Math.cos(angle) * (spindleRadius - 10);
            const y2 = cy + Math.sin(angle) * (spindleRadius - 10);
            ctx.beginPath();
            ctx.moveTo(x1, y1);
            ctx.lineTo(x2, y2);
            ctx.strokeStyle = '#455a64';
            ctx.lineWidth = 2;
            ctx.stroke();
        }

        this.sensorPositions.forEach((sensor, idx) => {
            const value = this.vibrationData[idx] || 1.0;
            const color = this.getSensorColor(value, sensor.type);

            ctx.beginPath();
            ctx.arc(sensor.x, sensor.y, sensor.radius, 0, Math.PI * 2);
            
            const glow = ctx.createRadialGradient(
                sensor.x, sensor.y, 0,
                sensor.x, sensor.y, sensor.radius + 8
            );
            glow.addColorStop(0, color);
            glow.addColorStop(0.5, color + '80');
            glow.addColorStop(1, 'transparent');
            ctx.fillStyle = glow;
            ctx.fill();

            ctx.beginPath();
            ctx.arc(sensor.x, sensor.y, sensor.radius, 0, Math.PI * 2);
            ctx.fillStyle = color;
            ctx.fill();
            ctx.strokeStyle = '#fff';
            ctx.lineWidth = 2;
            ctx.stroke();

            ctx.fillStyle = '#fff';
            ctx.font = 'bold 10px Arial';
            ctx.textAlign = 'center';
            ctx.textBaseline = 'middle';
            ctx.fillText(sensor.index + 1, sensor.x, sensor.y);
        });

        ctx.fillStyle = '#90a4ae';
        ctx.font = '12px Arial';
        ctx.textAlign = 'center';
        ctx.fillText('主轴剖面图', cx, this.height - 20);
    }
};
