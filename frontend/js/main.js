window.addEventListener('DOMContentLoaded', async () => {
    console.log('🔧 主轴健康监控系统初始化中...');

    window.spindleCanvas = new SpindleCanvas('spindle-canvas');
    window.waveformChart = new WaveformChart('waveform-canvas');
    window.spectrumChart = new SpectrumChart('spectrum-canvas');
    window.waterfallChart = new WaterfallChart('waterfall-canvas');
    window.rulChart = new RULTrendChart('rul-canvas');
    window.dashboard = new Dashboard();

    const positions = await API.getSensorPositions();
    window.spindleCanvas.setSensorPositions(positions);

    window.spindleCanvas.onSensorClick = (pos, rms) => {
        openSensorModal(pos, rms);
    };

    await window.dashboard.init();

    setInterval(() => {
        if (window.spindleCanvas) {
            window.spindleCanvas.draw();
        }
    }, 100);

    setInterval(() => {
        if (window.waterfallChart) {
            const data = [];
            const bins = 50;
            for (let i = 0; i < bins; i++) {
                const freq = (i / bins) * 1000;
                let val = 0;
                if (freq > 40 && freq < 60) val += 2.5;
                if (freq > 110 && freq < 130) val += 1.8;
                if (freq > 290 && freq < 310) val += 1.2;
                val += Math.random() * 0.5;
                val = Math.max(0, val);
                data.push(val);
            }
            window.waterfallChart.addFrame(data);
        }
    }, 2000);

    document.getElementById('sensor-modal').addEventListener('click', (e) => {
        if (e.target.id === 'sensor-modal') {
            closeSensorModal();
        }
    });

    console.log('✅ 系统初始化完成');
});
