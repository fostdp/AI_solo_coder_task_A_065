class SpectrumRenderer {
    constructor(canvasId) {
        this.canvas = document.getElementById(canvasId);
        this.ctx = this.canvas.getContext('2d');
        this.spectrumData = [];
        this.maxHistory = 60;
        this.setupCanvas();
    }

    setupCanvas() {
        const rect = this.canvas.parentElement.getBoundingClientRect();
        this.canvas.width = rect.width;
        this.canvas.height = 300;
    }

    addSpectrum(frequencies, amplitudes) {
        this.spectrumData.push({ frequencies, amplitudes, time: Date.now() });
        if (this.spectrumData.length > this.maxHistory) {
            this.spectrumData.shift();
        }
        this.drawWaterfall();
    }

    drawWaterfall() {
        const ctx = this.ctx;
        const w = this.canvas.width;
        const h = this.canvas.height;
        
        ctx.clearRect(0, 0, w, h);
        
        if (this.spectrumData.length === 0) return;
        
        const padding = { top: 30, right: 50, bottom: 40, left: 60 };
        const chartWidth = w - padding.left - padding.right;
        const chartHeight = h - padding.top - padding.bottom;
        
        ctx.fillStyle = 'rgba(0, 0, 0, 0.5)';
        ctx.fillRect(padding.left, padding.top, chartWidth, chartHeight);
        
        const sliceHeight = chartHeight / this.maxHistory;
        
        for (let i = 0; i < this.spectrumData.length; i++) {
            const data = this.spectrumData[i];
            const y = padding.top + (this.spectrumData.length - 1 - i) * sliceHeight;
            
            for (let j = 0; j < data.amplitudes.length - 1; j++) {
                const x1 = padding.left + (j / (data.amplitudes.length - 1)) * chartWidth;
                const x2 = padding.left + ((j + 1) / (data.amplitudes.length - 1)) * chartWidth;
                
                const amp = data.amplitudes[j];
                const color = this.getAmplitudeColor(amp);
                
                ctx.fillStyle = color;
                ctx.fillRect(x1, y, x2 - x1 + 1, sliceHeight + 1);
            }
        }
        
        this.drawAxes(padding, chartWidth, chartHeight);
        
        if (this.spectrumData.length > 0) {
            const latest = this.spectrumData[this.spectrumData.length - 1];
            this.drawCurrentSpectrum(latest, padding, chartWidth, chartHeight);
        }
    }

    getAmplitudeColor(amp) {
        const normalized = Math.min(amp / 5.0, 1.0);
        
        if (normalized < 0.25) {
            return `rgba(0, 0, 139, ${0.3 + normalized})`;
        } else if (normalized < 0.5) {
            const t = (normalized - 0.25) / 0.25;
            const r = Math.floor(t * 100);
            const g = Math.floor(t * 100);
            return `rgba(${r}, ${g}, 255, ${0.5 + normalized * 0.3})`;
        } else if (normalized < 0.75) {
            const t = (normalized - 0.5) / 0.25;
            const r = Math.floor(100 + t * 155);
            const g = Math.floor(100 + t * 55);
            return `rgba(${r}, ${g}, 100, ${0.6 + normalized * 0.2})`;
        } else {
            const t = (normalized - 0.75) / 0.25;
            const r = 255;
            const g = Math.floor(255 - t * 200);
            return `rgba(${r}, ${g}, 50, ${0.7 + normalized * 0.3})`;
        }
    }

    drawAxes(padding, chartWidth, chartHeight) {
        const ctx = this.ctx;
        
        ctx.strokeStyle = 'rgba(255, 255, 255, 0.3)';
        ctx.lineWidth = 1;
        
        ctx.beginPath();
        ctx.moveTo(padding.left, padding.top);
        ctx.lineTo(padding.left, padding.top + chartHeight);
        ctx.lineTo(padding.left + chartWidth, padding.top + chartHeight);
        ctx.stroke();
        
        ctx.fillStyle = 'rgba(255, 255, 255, 0.7)';
        ctx.font = '11px Arial';
        ctx.textAlign = 'center';
        
        const maxFreq = 1000;
        for (let i = 0; i <= 5; i++) {
            const freq = (i / 5) * maxFreq;
            const x = padding.left + (i / 5) * chartWidth;
            ctx.fillText(`${freq}Hz`, x, padding.top + chartHeight + 20);
            
            ctx.beginPath();
            ctx.moveTo(x, padding.top + chartHeight);
            ctx.lineTo(x, padding.top + chartHeight + 5);
            ctx.stroke();
        }
        
        ctx.textAlign = 'right';
        ctx.fillText('最新', padding.left - 10, padding.top + 15);
        ctx.fillText('时间', padding.left - 10, padding.top + chartHeight / 2);
        ctx.fillText('最早', padding.left - 10, padding.top + chartHeight - 5);
        
        ctx.textAlign = 'center';
        ctx.fillText('频率 (Hz)', padding.left + chartWidth / 2, padding.top + chartHeight + 35);
        
        const gradient = ctx.createLinearGradient(0, 0, 0, chartHeight);
        gradient.addColorStop(0, 'rgba(255, 100, 50, 0.8)');
        gradient.addColorStop(0.5, 'rgba(100, 100, 255, 0.8)');
        gradient.addColorStop(1, 'rgba(0, 0, 139, 0.8)');
        
        ctx.fillStyle = gradient;
        ctx.fillRect(padding.left + chartWidth + 15, padding.top, 15, chartHeight);
        
        ctx.fillStyle = 'rgba(255, 255, 255, 0.7)';
        ctx.textAlign = 'left';
        ctx.fillText('高', padding.left + chartWidth + 35, padding.top + 10);
        ctx.fillText('低', padding.left + chartWidth + 35, padding.top + chartHeight);
    }

    drawCurrentSpectrum(data, padding, chartWidth, chartHeight) {
        const ctx = this.ctx;
        
        ctx.strokeStyle = 'rgba(0, 212, 255, 0.9)';
        ctx.lineWidth = 2;
        ctx.beginPath();
        
        for (let i = 0; i < data.amplitudes.length; i++) {
            const x = padding.left + (i / (data.amplitudes.length - 1)) * chartWidth;
            const normalizedAmp = Math.min(data.amplitudes[i] / 5.0, 1.0);
            const y = padding.top + chartHeight - normalizedAmp * chartHeight;
            
            if (i === 0) {
                ctx.moveTo(x, y);
            } else {
                ctx.lineTo(x, y);
            }
        }
        ctx.stroke();
        
        ctx.fillStyle = 'rgba(0, 212, 255, 0.1)';
        ctx.lineTo(padding.left + chartWidth, padding.top + chartHeight);
        ctx.lineTo(padding.left, padding.top + chartHeight);
        ctx.closePath();
        ctx.fill();
    }

    generateMockData() {
        const frequencies = [];
        const amplitudes = [];
        const maxFreq = 1000;
        const numPoints = 100;
        
        for (let i = 0; i < numPoints; i++) {
            frequencies.push((i / (numPoints - 1)) * maxFreq);
            
            let amp = 0.3 + Math.random() * 0.3;
            
            const bearingFreq = 150;
            const idx = Math.round((bearingFreq / maxFreq) * (numPoints - 1));
            if (Math.abs(i - idx) < 3) {
                amp += 2.0 * Math.exp(-Math.pow((i - idx) / 2, 2));
            }
            
            const harmonicIdx = idx * 2;
            if (Math.abs(i - harmonicIdx) < 2) {
                amp += 1.0 * Math.exp(-Math.pow((i - harmonicIdx) / 1.5, 2));
            }
            
            amplitudes.push(amp);
        }
        
        return { frequencies, amplitudes };
    }

    startAnimation() {
        setInterval(() => {
            const data = this.generateMockData();
            this.addSpectrum(data.frequencies, data.amplitudes);
        }, 500);
    }
}
