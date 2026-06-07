class WaterfallPlot {
    constructor(canvasId, options = {}) {
        this.canvas = document.getElementById(canvasId);
        this.ctx = this.canvas.getContext('2d');
        this.width = this.canvas.width;
        this.height = this.canvas.height;
        
        this.maxHistory = options.maxHistory || 60;
        this.maxFrequency = options.maxFrequency || 500;
        this.frequencyBins = options.frequencyBins || 256;
        this.sensorIndex = options.sensorIndex || 0;
        
        this.history = [];
        this.currentSpectrum = new Float32Array(this.frequencyBins);
        this.isPaused = false;
        this.lastRenderTime = 0;
        this.frameInterval = 1000 / 30;
        
        this.colorMap = this.createColorMap();
        this.bindEvents();
    }

    createColorMap() {
        const colors = [];
        const stops = [
            { pos: 0, r: 0, g: 0, b: 0 },
            { pos: 0.2, r: 0, g: 48, b: 87 },
            { pos: 0.4, r: 32, g: 128, b: 128 },
            { pos: 0.6, r: 144, g: 190, b: 109 },
            { pos: 0.8, r: 243, g: 196, b: 65 },
            { pos: 1.0, r: 242, g: 65, b: 65 }
        ];

        for (let i = 0; i < 256; i++) {
            const pos = i / 255;
            let lower = stops[0];
            let upper = stops[stops.length - 1];
            
            for (let j = 0; j < stops.length - 1; j++) {
                if (pos >= stops[j].pos && pos <= stops[j + 1].pos) {
                    lower = stops[j];
                    upper = stops[j + 1];
                    break;
                }
            }

            const range = upper.pos - lower.pos;
            const t = range === 0 ? 0 : (pos - lower.pos) / range;
            colors.push({
                r: Math.round(lower.r + (upper.r - lower.r) * t),
                g: Math.round(lower.g + (upper.g - lower.g) * t),
                b: Math.round(lower.b + (upper.b - lower.b) * t)
            });
        }
        
        return colors;
    }

    bindEvents() {
        this.canvas.addEventListener('click', () => this.togglePause());
    }

    togglePause() {
        this.isPaused = !this.isPaused;
        this.renderOverlay();
    }

    setSensorIndex(index) {
        this.sensorIndex = index;
        this.history = [];
        this.render();
    }

    addSpectrum(spectrumData) {
        if (this.isPaused) return;
        
        this.currentSpectrum = spectrumData;
        
        const displayBins = new Float32Array(this.frequencyBins);
        const scale = spectrumData.length / this.frequencyBins;
        
        for (let i = 0; i < this.frequencyBins; i++) {
            const srcIdx = Math.floor(i * scale);
            displayBins[i] = spectrumData[Math.min(srcIdx, spectrumData.length - 1)] || 0;
        }
        
        this.history.unshift(displayBins);
        if (this.history.length > this.maxHistory) {
            this.history.pop();
        }
    }

    update(spectrumData, sensorIndex) {
        if (sensorIndex !== undefined && sensorIndex !== this.sensorIndex) {
            this.setSensorIndex(sensorIndex);
        }
        if (spectrumData) {
            this.addSpectrum(spectrumData);
        }
        this.render();
    }

    render() {
        const now = performance.now();
        if (now - this.lastRenderTime < this.frameInterval) return;
        this.lastRenderTime = now;

        const padding = { left: 50, right: 20, top: 30, bottom: 40 };
        const plotWidth = this.width - padding.left - padding.right;
        const plotHeight = this.height - padding.top - padding.bottom;

        this.ctx.clearRect(0, 0, this.width, this.height);
        this.drawBackground();
        this.drawWaterfall(padding, plotWidth, plotHeight);
        this.drawGrid(padding, plotWidth, plotHeight);
        this.drawAxes(padding, plotWidth, plotHeight);
        this.drawTitle();
        this.drawColorBar(padding, plotWidth, plotHeight);
        
        if (this.isPaused) {
            this.renderOverlay();
        }
    }

    drawBackground() {
        this.ctx.fillStyle = '#0f1419';
        this.ctx.fillRect(0, 0, this.width, this.height);
    }

    drawWaterfall(padding, plotWidth, plotHeight) {
        const rowHeight = plotHeight / this.maxHistory;
        const binWidth = plotWidth / this.frequencyBins;
        const maxAmp = this.getMaxAmplitude();

        for (let row = 0; row < this.history.length; row++) {
            const spectrum = this.history[row];
            const y = padding.top + row * rowHeight;
            
            for (let bin = 0; bin < this.frequencyBins; bin++) {
                const amplitude = spectrum[bin];
                const normalized = maxAmp > 0 ? Math.min(amplitude / maxAmp, 1.0) : 0;
                const colorIdx = Math.floor(normalized * 255);
                const color = this.colorMap[Math.min(colorIdx, 255)];
                
                this.ctx.fillStyle = `rgb(${color.r},${color.g},${color.b})`;
                this.ctx.fillRect(
                    padding.left + bin * binWidth,
                    y,
                    binWidth + 1,
                    rowHeight + 1
                );
            }
        }
    }

    getMaxAmplitude() {
        let max = 0;
        for (const spectrum of this.history) {
            for (let i = 0; i < spectrum.length; i++) {
                if (spectrum[i] > max) max = spectrum[i];
            }
        }
        return max || 1;
    }

    drawGrid(padding, plotWidth, plotHeight) {
        this.ctx.strokeStyle = 'rgba(255,255,255,0.1)';
        this.ctx.lineWidth = 1;

        const freqTicks = 5;
        for (let i = 0; i <= freqTicks; i++) {
            const x = padding.left + (plotWidth * i) / freqTicks;
            this.ctx.beginPath();
            this.ctx.moveTo(x, padding.top);
            this.ctx.lineTo(x, padding.top + plotHeight);
            this.ctx.stroke();
        }

        const timeTicks = 5;
        for (let i = 0; i <= timeTicks; i++) {
            const y = padding.top + (plotHeight * i) / timeTicks;
            this.ctx.beginPath();
            this.ctx.moveTo(padding.left, y);
            this.ctx.lineTo(padding.left + plotWidth, y);
            this.ctx.stroke();
        }
    }

    drawAxes(padding, plotWidth, plotHeight) {
        this.ctx.fillStyle = '#a0aec0';
        this.ctx.font = '11px sans-serif';
        this.ctx.textAlign = 'center';

        const freqTicks = 5;
        for (let i = 0; i <= freqTicks; i++) {
            const x = padding.left + (plotWidth * i) / freqTicks;
            const freq = Math.round((this.maxFrequency * i) / freqTicks);
            this.ctx.fillText(`${freq} Hz`, x, padding.top + plotHeight + 20);
        }

        this.ctx.textAlign = 'right';
        const timeTicks = 5;
        for (let i = 0; i <= timeTicks; i++) {
            const y = padding.top + (plotHeight * i) / timeTicks;
            const time = Math.round((this.maxHistory * i) / timeTicks);
            this.ctx.fillText(`${time}s`, padding.left - 8, y + 4);
        }

        this.ctx.save();
        this.ctx.translate(15, padding.top + plotHeight / 2);
        this.ctx.rotate(-Math.PI / 2);
        this.ctx.textAlign = 'center';
        this.ctx.fillText('时间 (秒)', 0, 0);
        this.ctx.restore();

        this.ctx.textAlign = 'center';
        this.ctx.fillText('频率 (Hz)', padding.left + plotWidth / 2, padding.top + plotHeight + 35);
    }

    drawTitle() {
        this.ctx.fillStyle = '#e2e8f0';
        this.ctx.font = '14px sans-serif';
        this.ctx.textAlign = 'center';
        const sensorName = `传感器 ${this.sensorIndex + 1}`;
        this.ctx.fillText(`瀑布图 - ${sensorName} 频谱演化`, this.width / 2, 18);
        
        if (this.isPaused) {
            this.ctx.fillStyle = '#f56565';
            this.ctx.fillText('[已暂停]', this.width / 2 + 100, 18);
        }
    }

    drawColorBar(padding, plotWidth, plotHeight) {
        const barWidth = 15;
        const barX = padding.left + plotWidth + 8;
        const barY = padding.top;

        for (let i = 0; i < 256; i++) {
            const color = this.colorMap[255 - i];
            this.ctx.fillStyle = `rgb(${color.r},${color.g},${color.b})`;
            this.ctx.fillRect(barX, barY + i * (plotHeight / 256), barWidth, plotHeight / 256 + 1);
        }

        this.ctx.strokeStyle = '#4a5568';
        this.ctx.lineWidth = 1;
        this.ctx.strokeRect(barX, barY, barWidth, plotHeight);

        this.ctx.fillStyle = '#a0aec0';
        this.ctx.font = '10px sans-serif';
        this.ctx.textAlign = 'left';
        this.ctx.fillText('高', barX + barWidth + 5, barY + 10);
        this.ctx.fillText('低', barX + barWidth + 5, barY + plotHeight - 2);
    }

    renderOverlay() {
        if (this.isPaused) {
            this.ctx.fillStyle = 'rgba(0,0,0,0.5)';
            this.ctx.fillRect(0, 0, this.width, this.height);
            
            this.ctx.fillStyle = '#fff';
            this.ctx.font = 'bold 24px sans-serif';
            this.ctx.textAlign = 'center';
            this.ctx.fillText('已暂停', this.width / 2, this.height / 2);
            this.ctx.font = '14px sans-serif';
            this.ctx.fillText('点击继续', this.width / 2, this.height / 2 + 30);
        }
    }

    clear() {
        this.history = [];
        this.render();
    }

    resize() {
        this.width = this.canvas.width;
        this.height = this.canvas.height;
        this.render();
    }

    destroy() {
        this.history = [];
        this.colorMap = [];
    }
}

if (typeof module !== 'undefined' && module.exports) {
    module.exports = WaterfallPlot;
}
