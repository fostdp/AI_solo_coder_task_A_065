class ChartUtils {
    static drawGrid(ctx, width, height, padding) {
        ctx.save();
        ctx.strokeStyle = 'rgba(71, 85, 105, 0.3)';
        ctx.lineWidth = 1;
        
        const gridSize = 40;
        for (let x = padding.left; x < width - padding.right; x += gridSize) {
            ctx.beginPath();
            ctx.moveTo(x, padding.top);
            ctx.lineTo(x, height - padding.bottom);
            ctx.stroke();
        }
        for (let y = padding.top; y < height - padding.bottom; y += gridSize) {
            ctx.beginPath();
            ctx.moveTo(padding.left, y);
            ctx.lineTo(width - padding.right, y);
            ctx.stroke();
        }
        ctx.restore();
    }

    static drawAxes(ctx, width, height, padding, xLabel, yLabel) {
        ctx.save();
        ctx.strokeStyle = '#64748b';
        ctx.lineWidth = 2;
        
        ctx.beginPath();
        ctx.moveTo(padding.left, padding.top);
        ctx.lineTo(padding.left, height - padding.bottom);
        ctx.lineTo(width - padding.right, height - padding.bottom);
        ctx.stroke();
        
        ctx.fillStyle = '#94a3b8';
        ctx.font = '10px "Microsoft YaHei"';
        ctx.textAlign = 'center';
        ctx.fillText(xLabel, width / 2, height - 8);
        
        ctx.save();
        ctx.translate(12, height / 2);
        ctx.rotate(-Math.PI / 2);
        ctx.fillText(yLabel, 0, 0);
        ctx.restore();
        
        ctx.restore();
    }
}

class WaveformChart {
    constructor(canvasId) {
        this.canvas = document.getElementById(canvasId);
        this.ctx = this.canvas.getContext('2d');
        this.data = [];
        this.padding = { left: 50, right: 20, top: 20, bottom: 40 };
    }

    setData(data) {
        this.data = data || [];
        this.draw();
    }

    draw() {
        const ctx = this.ctx;
        const width = this.canvas.width;
        const height = this.canvas.height;
        
        ctx.clearRect(0, 0, width, height);
        
        ChartUtils.drawGrid(ctx, width, height, this.padding);
        ChartUtils.drawAxes(ctx, width, height, this.padding, '时间 (s)', '加速度 (g)');
        
        if (this.data.length < 2) return;
        
        const plotWidth = width - this.padding.left - this.padding.right;
        const plotHeight = height - this.padding.top - this.padding.bottom;
        
        const values = this.data.map(d => typeof d === 'number' ? d : d.value || 0);
        const maxVal = Math.max(...values, 0.1);
        const minVal = Math.min(...values, -0.1);
        const range = maxVal - minVal || 1;
        
        ctx.save();
        ctx.beginPath();
        
        this.data.forEach((d, i) => {
            const x = this.padding.left + (i / (this.data.length - 1)) * plotWidth;
            const val = typeof d === 'number' ? d : d.value || 0;
            const y = this.padding.top + plotHeight - ((val - minVal) / range) * plotHeight;
            
            if (i === 0) {
                ctx.moveTo(x, y);
            } else {
                ctx.lineTo(x, y);
            }
        });
        
        ctx.strokeStyle = '#3b82f6';
        ctx.lineWidth = 2;
        ctx.stroke();
        
        ctx.lineTo(width - this.padding.right, height - this.padding.bottom);
        ctx.lineTo(this.padding.left, height - this.padding.bottom);
        ctx.closePath();
        ctx.fillStyle = 'rgba(59, 130, 246, 0.15)';
        ctx.fill();
        
        ctx.fillStyle = '#64748b';
        ctx.font = '9px Arial';
        ctx.textAlign = 'right';
        ctx.fillText(maxVal.toFixed(2), this.padding.left - 5, this.padding.top + 10);
        ctx.fillText(minVal.toFixed(2), this.padding.left - 5, height - this.padding.bottom - 2);
        ctx.textAlign = 'left';
        ctx.fillText('0s', this.padding.left, height - this.padding.bottom + 15);
        ctx.fillText(`${this.data.length / 10}s`, width - this.padding.right - 20, height - this.padding.bottom + 15);
        
        ctx.restore();
    }

    generateMockWaveform(duration = 1, sampleRate = 100) {
        const data = [];
        const samples = duration * sampleRate;
        for (let i = 0; i < samples; i++) {
            const t = i / sampleRate;
            let val = 0;
            val += Math.sin(2 * Math.PI * 50 * t) * 0.3;
            val += Math.sin(2 * Math.PI * 120 * t) * 0.2;
            val += Math.sin(2 * Math.PI * 300 * t) * 0.1;
            val += (Math.random() - 0.5) * 0.1;
            data.push(val);
        }
        this.setData(data);
    }
}

class SpectrumChart {
    constructor(canvasId) {
        this.canvas = document.getElementById(canvasId);
        this.ctx = this.canvas.getContext('2d');
        this.data = [];
        this.padding = { left: 50, right: 20, top: 20, bottom: 40 };
        this.maxFreq = 1000;
    }

    setData(data) {
        this.data = data || [];
        this.draw();
    }

    draw() {
        const ctx = this.ctx;
        const width = this.canvas.width;
        const height = this.canvas.height;
        
        ctx.clearRect(0, 0, width, height);
        
        ChartUtils.drawGrid(ctx, width, height, this.padding);
        ChartUtils.drawAxes(ctx, width, height, this.padding, '频率 (Hz)', '幅值 (mm/s)');
        
        if (this.data.length < 2) return;
        
        const plotWidth = width - this.padding.left - this.padding.right;
        const plotHeight = height - this.padding.top - this.padding.bottom;
        
        const maxVal = Math.max(...this.data, 0.01);
        
        ctx.save();
        
        const barWidth = Math.max(2, plotWidth / this.data.length - 1);
        
        this.data.forEach((val, i) => {
            const x = this.padding.left + (i / this.data.length) * plotWidth;
            const barHeight = (val / maxVal) * plotHeight;
            const y = this.padding.top + plotHeight - barHeight;
            
            const gradient = ctx.createLinearGradient(x, y, x, this.padding.top + plotHeight);
            gradient.addColorStop(0, '#8b5cf6');
            gradient.addColorStop(0.5, '#6366f1');
            gradient.addColorStop(1, '#3b82f6');
            
            ctx.fillStyle = gradient;
            ctx.fillRect(x, y, barWidth, barHeight);
        });
        
        ctx.fillStyle = '#64748b';
        ctx.font = '9px Arial';
        ctx.textAlign = 'right';
        ctx.fillText(maxVal.toFixed(2), this.padding.left - 5, this.padding.top + 10);
        ctx.textAlign = 'left';
        
        for (let i = 0; i <= 5; i++) {
            const freq = (i / 5) * this.maxFreq;
            const x = this.padding.left + (i / 5) * plotWidth;
            ctx.fillText(`${freq}`, x - 10, height - this.padding.bottom + 15);
        }
        
        ctx.restore();
    }

    generateMockSpectrum() {
        const data = [];
        const bins = 100;
        for (let i = 0; i < bins; i++) {
            const freq = (i / bins) * this.maxFreq;
            let val = 0;
            
            if (freq > 40 && freq < 60) val += 2.5;
            if (freq > 110 && freq < 130) val += 1.8;
            if (freq > 290 && freq < 310) val += 1.2;
            if (freq > 500 && freq < 520) val += 0.8;
            
            val += (Math.random() - 0.5) * 0.3;
            val = Math.max(0, val);
            
            data.push(val);
        }
        this.setData(data);
    }
}

class WaterfallChart {
    constructor(canvasId) {
        this.canvas = document.getElementById(canvasId);
        this.ctx = this.canvas.getContext('2d');
        this.frames = [];
        this.maxFrames = 30;
        this.maxFreq = 1000;
    }

    addFrame(spectrumData) {
        this.frames.push(spectrumData);
        if (this.frames.length > this.maxFrames) {
            this.frames.shift();
        }
        this.draw();
    }

    draw() {
        const ctx = this.ctx;
        const width = this.canvas.width;
        const height = this.canvas.height;
        
        ctx.clearRect(0, 0, width, height);
        
        if (this.frames.length === 0) return;
        
        const bins = this.frames[0].length;
        const frameWidth = width / this.maxFrames;
        
        this.frames.forEach((frame, frameIdx) => {
            const xOffset = frameIdx * frameWidth;
            const maxVal = Math.max(...frame, 0.01);
            
            frame.forEach((val, binIdx) => {
                const normalized = val / maxVal;
                const x = xOffset + (binIdx / bins) * frameWidth;
                const y = height - (normalized * height * 0.8) - 20;
                const rectHeight = normalized * height * 0.8;
                
                const hue = 240 - normalized * 200;
                const lightness = 30 + normalized * 40;
                ctx.fillStyle = `hsl(${hue}, 80%, ${lightness}%)`;
                ctx.fillRect(x, y, frameWidth / bins + 0.5, rectHeight);
            });
        });
        
        ctx.save();
        ctx.fillStyle = 'rgba(15, 23, 42, 0.8)';
        ctx.fillRect(0, 0, width, 25);
        ctx.fillRect(0, height - 25, width, 25);
        
        ctx.fillStyle = '#94a3b8';
        ctx.font = '10px "Microsoft YaHei"';
        ctx.textAlign = 'left';
        ctx.fillText('← 时间 (30分钟)', 10, 17);
        ctx.fillText('频率 (Hz) →', 10, height - 8);
        
        ctx.textAlign = 'right';
        for (let i = 0; i <= 5; i++) {
            const freq = (i / 5) * this.maxFreq;
            const x = (i / 5) * width;
            ctx.fillText(`${freq}`, x - 5, height - 8);
        }
        
        const gradient = ctx.createLinearGradient(width - 60, 30, width - 60, height - 30);
        gradient.addColorStop(0, '#f87171');
        gradient.addColorStop(0.5, '#60a5fa');
        gradient.addColorStop(1, '#1e3a8a');
        ctx.fillStyle = gradient;
        ctx.fillRect(width - 50, 30, 15, height - 60);
        
        ctx.fillStyle = '#94a3b8';
        ctx.textAlign = 'left';
        ctx.fillText('高', width - 32, 40);
        ctx.fillText('低', width - 32, height - 35);
        
        ctx.restore();
    }

    generateMockFrames() {
        for (let i = 0; i < this.maxFrames; i++) {
            const data = [];
            const bins = 50;
            const phase = i / this.maxFrames * Math.PI * 2;
            
            for (let j = 0; j < bins; j++) {
                const freq = (j / bins) * this.maxFreq;
                let val = 0;
                
                const mod = Math.sin(phase + j * 0.1) * 0.5 + 0.5;
                if (freq > 40 && freq < 60) val += 2.5 * (0.8 + mod * 0.4);
                if (freq > 110 && freq < 130) val += 1.8 * (0.9 + mod * 0.2);
                if (freq > 290 && freq < 310) val += 1.2 * (0.7 + mod * 0.5);
                
                val += Math.random() * 0.3;
                val = Math.max(0, val);
                
                data.push(val);
            }
            
            this.frames.push(data);
        }
        this.draw();
    }
}

class RULTrendChart {
    constructor(canvasId) {
        this.canvas = document.getElementById(canvasId);
        this.ctx = this.canvas.getContext('2d');
        this.data = [];
        this.padding = { left: 60, right: 30, top: 25, bottom: 40 };
    }

    setData(data) {
        this.data = data || [];
        this.draw();
    }

    draw() {
        const ctx = this.ctx;
        const width = this.canvas.width;
        const height = this.canvas.height;
        
        ctx.clearRect(0, 0, width, height);
        
        ChartUtils.drawGrid(ctx, width, height, this.padding);
        
        ctx.save();
        ctx.strokeStyle = '#64748b';
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.moveTo(this.padding.left, this.padding.top);
        ctx.lineTo(this.padding.left, height - this.padding.bottom);
        ctx.lineTo(width - this.padding.right, height - this.padding.bottom);
        ctx.stroke();
        ctx.restore();
        
        const plotWidth = width - this.padding.left - this.padding.right;
        const plotHeight = height - this.padding.top - this.padding.bottom;
        
        ctx.save();
        ctx.strokeStyle = 'rgba(245, 158, 11, 0.5)';
        ctx.lineWidth = 1;
        ctx.setLineDash([5, 5]);
        const y500 = this.padding.top + plotHeight - (500 / 10000) * plotHeight;
        ctx.beginPath();
        ctx.moveTo(this.padding.left, y500);
        ctx.lineTo(width - this.padding.right, y500);
        ctx.stroke();
        
        ctx.strokeStyle = 'rgba(239, 68, 68, 0.5)';
        const y200 = this.padding.top + plotHeight - (200 / 10000) * plotHeight;
        ctx.beginPath();
        ctx.moveTo(this.padding.left, y200);
        ctx.lineTo(width - this.padding.right, y200);
        ctx.stroke();
        ctx.setLineDash([]);
        
        ctx.fillStyle = '#f59e0b';
        ctx.font = '9px Arial';
        ctx.textAlign = 'left';
        ctx.fillText('预警: 500h', width - this.padding.right + 5, y500 + 3);
        ctx.fillStyle = '#ef4444';
        ctx.fillText('告警: 200h', width - this.padding.right + 5, y200 + 3);
        ctx.restore();
        
        if (this.data.length < 2) return;
        
        ctx.save();
        ctx.beginPath();
        
        this.data.forEach((d, i) => {
            const x = this.padding.left + (i / (this.data.length - 1)) * plotWidth;
            const rul = typeof d === 'number' ? d : d.rul || d.value || 0;
            const y = this.padding.top + plotHeight - (rul / 10000) * plotHeight;
            
            if (i === 0) {
                ctx.moveTo(x, y);
            } else {
                ctx.lineTo(x, y);
            }
        });
        
        ctx.strokeStyle = '#10b981';
        ctx.lineWidth = 2.5;
        ctx.stroke();
        
        ctx.lineTo(width - this.padding.right, height - this.padding.bottom);
        ctx.lineTo(this.padding.left, height - this.padding.bottom);
        ctx.closePath();
        
        const areaGradient = ctx.createLinearGradient(0, this.padding.top, 0, height - this.padding.bottom);
        areaGradient.addColorStop(0, 'rgba(16, 185, 129, 0.3)');
        areaGradient.addColorStop(1, 'rgba(16, 185, 129, 0.05)');
        ctx.fillStyle = areaGradient;
        ctx.fill();
        
        this.data.forEach((d, i) => {
            const x = this.padding.left + (i / (this.data.length - 1)) * plotWidth;
            const rul = typeof d === 'number' ? d : d.rul || d.value || 0;
            const y = this.padding.top + plotHeight - (rul / 10000) * plotHeight;
            
            ctx.fillStyle = '#10b981';
            ctx.beginPath();
            ctx.arc(x, y, 3, 0, Math.PI * 2);
            ctx.fill();
        });
        
        ctx.fillStyle = '#64748b';
        ctx.font = '9px Arial';
        ctx.textAlign = 'right';
        for (let i = 0; i <= 5; i++) {
            const rul = (i / 5) * 10000;
            const y = this.padding.top + plotHeight - (i / 5) * plotHeight;
            ctx.fillText(`${rul}h`, this.padding.left - 5, y + 3);
        }
        
        ctx.textAlign = 'left';
        ctx.fillText('时间 →', width / 2, height - 8);
        
        ctx.restore();
    }

    generateMockRUL() {
        const data = [];
        const points = 20;
        let rul = 8000;
        for (let i = 0; i < points; i++) {
            rul -= 200 + Math.random() * 150;
            if (rul < 0) rul = 0;
            data.push({ rul: Math.max(0, rul) });
        }
        this.setData(data);
    }
}
