const SensorCharts = {
    waveformCanvas: null,
    waveformCtx: null,
    spectrumCanvas: null,
    spectrumCtx: null,
    trendCanvas: null,
    trendCtx: null,

    init() {
        this.waveformCanvas = document.getElementById('waveform-canvas');
        this.waveformCtx = this.waveformCanvas.getContext('2d');
        this.spectrumCanvas = document.getElementById('spectrum-canvas');
        this.spectrumCtx = this.spectrumCanvas.getContext('2d');
        this.trendCanvas = document.getElementById('trend-canvas');
        this.trendCtx = this.trendCanvas.getContext('2d');
    },

    async loadSensorData(machineId, sensorIndex) {
        document.getElementById('modal-sensor-id').textContent = sensorIndex + 1;
        
        const history = await API.getSensorHistory(machineId, sensorIndex, 1);
        if (history) {
            this.drawWaveform(history);
            this.drawSpectrum(history);
            this.drawTrend(history);
        }
    },

    drawWaveform(history) {
        const ctx = this.waveformCtx;
        const w = this.waveformCanvas.width;
        const h = this.waveformCanvas.height;

        ctx.clearRect(0, 0, w, h);
        ctx.fillStyle = '#0a1929';
        ctx.fillRect(0, 0, w, h);

        const values = history.values || [];
        if (values.length < 2) {
            ctx.fillStyle = '#607d8b';
            ctx.font = '12px Arial';
            ctx.textAlign = 'center';
            ctx.fillText('数据不足', w / 2, h / 2);
            return;
        }

        const maxVal = Math.max(...values) * 1.1;
        const minVal = Math.min(...values) * 0.9;
        const range = maxVal - minVal || 1;

        ctx.strokeStyle = '#29b6f6';
        ctx.lineWidth = 1.5;
        ctx.beginPath();

        values.forEach((val, i) => {
            const x = (i / (values.length - 1)) * w;
            const y = h - ((val - minVal) / range) * (h - 40) - 20;
            if (i === 0) {
                ctx.moveTo(x, y);
            } else {
                ctx.lineTo(x, y);
            }
        });
        ctx.stroke();

        ctx.fillStyle = '#90a4ae';
        ctx.font = '10px Arial';
        ctx.textAlign = 'right';
        ctx.fillText(maxVal.toFixed(2), 45, 15);
        ctx.fillText(minVal.toFixed(2), 45, h - 25);
        
        ctx.textAlign = 'center';
        ctx.fillText('时间', w - 30, h - 5);
    },

    drawSpectrum(history) {
        const ctx = this.spectrumCtx;
        const w = this.spectrumCanvas.width;
        const h = this.spectrumCanvas.height;

        ctx.clearRect(0, 0, w, h);
        ctx.fillStyle = '#0a1929';
        ctx.fillRect(0, 0, w, h);

        const frequencies = history.frequencies || [];
        const spectrum = history.spectrum && history.spectrum[0] || [];
        
        if (spectrum.length < 2) {
            const dummySpectrum = [];
            for (let i = 0; i < 64; i++) {
                dummySpectrum.push(Math.random() * 3 + 0.5);
            }
            this.drawBarSpectrum(ctx, w, h, dummySpectrum);
            return;
        }

        this.drawBarSpectrum(ctx, w, h, spectrum);
    },

    drawBarSpectrum(ctx, w, h, spectrum) {
        const barWidth = w / spectrum.length;
        const maxVal = Math.max(...spectrum) * 1.1;

        spectrum.forEach((val, i) => {
            const x = i * barWidth;
            const barHeight = (val / maxVal) * (h - 40);
            const y = h - barHeight - 20;

            const gradient = ctx.createLinearGradient(x, y, x, h - 20);
            gradient.addColorStop(0, '#29b6f6');
            gradient.addColorStop(1, '#1976d2');
            
            ctx.fillStyle = gradient;
            ctx.fillRect(x + 1, y, barWidth - 2, barHeight);
        });

        ctx.fillStyle = '#90a4ae';
        ctx.font = '10px Arial';
        ctx.textAlign = 'center';
        for (let i = 0; i <= 5; i++) {
            const x = (i / 5) * (w - 60) + 50;
            ctx.fillText(`${Math.round(i / 5 * 500)}Hz`, x, h - 5);
        }
    },

    drawTrend(history) {
        const ctx = this.trendCtx;
        const w = this.trendCanvas.width;
        const h = this.trendCanvas.height;

        ctx.clearRect(0, 0, w, h);
        ctx.fillStyle = '#0a1929';
        ctx.fillRect(0, 0, w, h);

        const values = history.values || [];
        if (values.length < 2) {
            const mockValues = [];
            for (let i = 0; i < 100; i++) {
                mockValues.push(1.5 + Math.sin(i / 10) * 0.5 + Math.random() * 0.3);
            }
            this.drawTrendLine(ctx, w, h, mockValues);
            return;
        }

        this.drawTrendLine(ctx, w, h, values);
    },

    drawTrendLine(ctx, w, h, values) {
        const maxVal = Math.max(...values) * 1.1;
        const minVal = Math.min(...values) * 0.9;
        const range = maxVal - minVal || 1;

        ctx.beginPath();
        values.forEach((val, i) => {
            const x = (i / (values.length - 1)) * w;
            const y = h - ((val - minVal) / range) * (h - 40) - 20;
            if (i === 0) {
                ctx.moveTo(x, y);
            } else {
                ctx.lineTo(x, y);
            }
        });
        ctx.strokeStyle = '#69f0ae';
        ctx.lineWidth = 2;
        ctx.stroke();

        ctx.fillStyle = '#90a4ae';
        ctx.font = '10px Arial';
        ctx.textAlign = 'right';
        ctx.fillText(maxVal.toFixed(2) + ' mm/s', 45, 15);
        ctx.fillText(minVal.toFixed(2) + ' mm/s', 45, h - 25);
        
        ctx.textAlign = 'center';
        ctx.fillText('最近1小时', w / 2, h - 5);
    }
};

function closeModal() {
    document.getElementById('sensor-modal').classList.remove('show');
}

window.onclick = function(event) {
    const modal = document.getElementById('sensor-modal');
    if (event.target === modal) {
        closeModal();
    }
};
