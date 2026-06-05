var SpectrumChart = (function () {
    var canvas, ctx;
    var canvasW, canvasH;
    var waterfallData = [];
    var maxWaterfallRows = 60;
    var freqRange = 2000;
    var bearingFreqs = {
        BPFI: 156.2,
        BPFO: 103.8,
        BSF: 67.5,
        FTF: 11.7
    };
    var hoverInfo = null;

    function init() {
        canvas = document.getElementById('spectrum-canvas');
        ctx = canvas.getContext('2d');
        resize();
        canvas.addEventListener('mousemove', handleMouseMove);
        canvas.addEventListener('mouseleave', function () {
            hoverInfo = null;
        });
    }

    function resize() {
        var rect = canvas.parentElement.getBoundingClientRect();
        canvasW = rect.width;
        canvasH = rect.height;
        canvas.width = canvasW * window.devicePixelRatio;
        canvas.height = canvasH * window.devicePixelRatio;
        canvas.style.width = canvasW + 'px';
        canvas.style.height = canvasH + 'px';
        ctx.setTransform(window.devicePixelRatio, 0, 0, window.devicePixelRatio, 0, 0);
    }

    function reset() {
        waterfallData = [];
    }

    function addData(spectrum) {
        if (!spectrum) return;
        var row = [];
        for (var i = 0; i < spectrum.length; i++) {
            row.push(spectrum[i]);
        }
        waterfallData.push(row);
        if (waterfallData.length > maxWaterfallRows) {
            waterfallData.shift();
        }
    }

    function draw() {
        if (!ctx) return;
        ctx.clearRect(0, 0, canvasW, canvasH);
        ctx.fillStyle = '#0a0e17';
        ctx.fillRect(0, 0, canvasW, canvasH);

        var margin = { top: 30, right: 20, bottom: 35, left: 55 };
        var plotW = canvasW - margin.left - margin.right;
        var plotH = canvasH - margin.top - margin.bottom;

        if (waterfallData.length < 2) {
            drawEmptyState(margin, plotW, plotH);
            return;
        }

        var depthAngle = 0.4;
        var depthFactor = 0.3;
        var rowSpacing = plotH / (maxWaterfallRows + 5);
        var numRows = waterfallData.length;

        for (var row = 0; row < numRows; row++) {
            var dataRow = waterfallData[row];
            var rowT = (numRows - 1 - row) / maxWaterfallRows;
            var baseY = margin.top + plotH - row * rowSpacing;
            var offsetX = row * rowSpacing * depthAngle;
            var offsetY = -row * rowSpacing * depthFactor;
            var alpha = 0.3 + 0.7 * (1 - rowT);

            ctx.beginPath();
            for (var fi = 0; fi < dataRow.length; fi++) {
                var fx = margin.left + offsetX + (fi / dataRow.length) * plotW;
                var fy = baseY + offsetY - (dataRow[fi] / 100) * rowSpacing * 8;
                if (fi === 0) ctx.moveTo(fx, fy);
                else ctx.lineTo(fx, fy);
            }
            ctx.lineTo(margin.left + offsetX + plotW, baseY + offsetY);
            ctx.lineTo(margin.left + offsetX, baseY + offsetY);
            ctx.closePath();

            var fillAlpha = alpha * 0.15;
            ctx.fillStyle = 'rgba(59, 130, 246, ' + fillAlpha + ')';
            ctx.fill();
            ctx.strokeStyle = getRowColor(rowT, alpha);
            ctx.lineWidth = row === numRows - 1 ? 2 : 0.8;
            ctx.stroke();
        }

        if (numRows > 0) {
            drawLatestSpectrum(margin, plotW, plotH, waterfallData[numRows - 1]);
        }

        drawAxes(margin, plotW, plotH);
        drawBearingMarkers(margin, plotW);
        drawTitle(margin);

        if (hoverInfo) {
            drawHoverInfo(margin, plotW, plotH);
        }
    }

    function drawLatestSpectrum(margin, plotW, plotH, data) {
        ctx.beginPath();
        for (var i = 0; i < data.length; i++) {
            var x = margin.left + (i / data.length) * plotW;
            var y = margin.top + plotH - (data[i] / 100) * plotH;
            if (i === 0) ctx.moveTo(x, y);
            else ctx.lineTo(x, y);
        }
        var grad = ctx.createLinearGradient(margin.left, 0, margin.left + plotW, 0);
        grad.addColorStop(0, 'rgba(0, 200, 255, 0.9)');
        grad.addColorStop(0.5, 'rgba(59, 130, 246, 0.9)');
        grad.addColorStop(1, 'rgba(100, 60, 255, 0.9)');
        ctx.strokeStyle = grad;
        ctx.lineWidth = 2;
        ctx.stroke();

        ctx.lineTo(margin.left + plotW, margin.top + plotH);
        ctx.lineTo(margin.left, margin.top + plotH);
        ctx.closePath();
        var areaGrad = ctx.createLinearGradient(0, margin.top, 0, margin.top + plotH);
        areaGrad.addColorStop(0, 'rgba(59, 130, 246, 0.25)');
        areaGrad.addColorStop(1, 'rgba(59, 130, 246, 0.02)');
        ctx.fillStyle = areaGrad;
        ctx.fill();
    }

    function drawEmptyState(margin, plotW, plotH) {
        ctx.strokeStyle = '#1e293b';
        ctx.lineWidth = 1;
        ctx.strokeRect(margin.left, margin.top, plotW, plotH);

        ctx.fillStyle = '#64748b';
        ctx.font = '13px Inter, sans-serif';
        ctx.textAlign = 'center';
        ctx.fillText('等待频谱数据...', margin.left + plotW / 2, margin.top + plotH / 2);

        drawAxes(margin, plotW, plotH);
        drawTitle(margin);
    }

    function drawAxes(margin, plotW, plotH) {
        ctx.strokeStyle = '#2d3a4d';
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.moveTo(margin.left, margin.top);
        ctx.lineTo(margin.left, margin.top + plotH);
        ctx.lineTo(margin.left + plotW, margin.top + plotH);
        ctx.stroke();

        ctx.fillStyle = '#64748b';
        ctx.font = '10px JetBrains Mono, monospace';
        ctx.textAlign = 'center';
        var freqSteps = [0, 200, 400, 600, 800, 1000, 1200, 1400, 1600, 1800, 2000];
        for (var i = 0; i < freqSteps.length; i++) {
            var x = margin.left + (freqSteps[i] / freqRange) * plotW;
            ctx.fillText(freqSteps[i] + '', x, margin.top + plotH + 15);
            ctx.strokeStyle = 'rgba(45, 58, 77, 0.5)';
            ctx.beginPath();
            ctx.moveTo(x, margin.top);
            ctx.lineTo(x, margin.top + plotH);
            ctx.stroke();
        }

        ctx.textAlign = 'right';
        ctx.fillText('频率 (Hz)', margin.left + plotW, margin.top + plotH + 30);

        ctx.save();
        ctx.translate(15, margin.top + plotH / 2);
        ctx.rotate(-Math.PI / 2);
        ctx.textAlign = 'center';
        ctx.fillText('幅值 (dB)', 0, 0);
        ctx.restore();
    }

    function drawBearingMarkers(margin, plotW) {
        var markers = [
            { label: 'BPFI', freq: bearingFreqs.BPFI, color: '#ff3366' },
            { label: 'BPFO', freq: bearingFreqs.BPFO, color: '#ffaa00' },
            { label: 'BSF', freq: bearingFreqs.BSF, color: '#3b82f6' },
            { label: 'FTF', freq: bearingFreqs.FTF, color: '#00ff88' }
        ];
        for (var i = 0; i < markers.length; i++) {
            var m = markers[i];
            var x = margin.left + (m.freq / freqRange) * plotW;
            ctx.strokeStyle = m.color;
            ctx.lineWidth = 1;
            ctx.setLineDash([4, 4]);
            ctx.beginPath();
            ctx.moveTo(x, margin.top);
            ctx.lineTo(x, margin.top + (canvasH - margin.top - margin.bottom));
            ctx.stroke();
            ctx.setLineDash([]);

            ctx.fillStyle = m.color;
            ctx.font = '9px JetBrains Mono, monospace';
            ctx.textAlign = 'center';
            ctx.fillText(m.label, x, margin.top - 5);

            for (var h = 2; h <= 4; h++) {
                var hf = m.freq * h;
                if (hf <= freqRange) {
                    var hx = margin.left + (hf / freqRange) * plotW;
                    ctx.strokeStyle = m.color;
                    ctx.globalAlpha = 0.3;
                    ctx.setLineDash([2, 6]);
                    ctx.beginPath();
                    ctx.moveTo(hx, margin.top);
                    ctx.lineTo(hx, margin.top + (canvasH - margin.top - margin.bottom));
                    ctx.stroke();
                    ctx.setLineDash([]);
                    ctx.globalAlpha = 1;
                }
            }
        }
    }

    function drawTitle(margin) {
        ctx.fillStyle = '#94a3b8';
        ctx.font = 'bold 12px Inter, sans-serif';
        ctx.textAlign = 'left';
        ctx.fillText('振动频谱瀑布图', margin.left, 16);
    }

    function getRowColor(rowT, alpha) {
        var r = Math.round(59 + (0 - 59) * rowT);
        var g = Math.round(130 + (180 - 130) * rowT);
        var b = Math.round(246 + (255 - 246) * rowT);
        return 'rgba(' + r + ',' + g + ',' + b + ',' + alpha + ')';
    }

    function handleMouseMove(e) {
        var rect = canvas.getBoundingClientRect();
        var mx = e.clientX - rect.left;
        var my = e.clientY - rect.top;
        var margin = { top: 30, right: 20, bottom: 35, left: 55 };
        var plotW = canvasW - margin.left - margin.right;

        if (mx >= margin.left && mx <= margin.left + plotW) {
            var freq = ((mx - margin.left) / plotW) * freqRange;
            var mag = 0;
            if (waterfallData.length > 0) {
                var latestRow = waterfallData[waterfallData.length - 1];
                var idx = Math.floor((freq / freqRange) * latestRow.length);
                idx = Math.max(0, Math.min(latestRow.length - 1, idx));
                mag = latestRow[idx];
            }
            hoverInfo = { freq: freq, mag: mag, x: mx, y: my };
        } else {
            hoverInfo = null;
        }
    }

    function drawHoverInfo(margin, plotW, plotH) {
        var x = hoverInfo.x;
        var y = hoverInfo.y;

        ctx.strokeStyle = 'rgba(255, 255, 255, 0.2)';
        ctx.lineWidth = 1;
        ctx.setLineDash([2, 4]);
        ctx.beginPath();
        ctx.moveTo(x, margin.top);
        ctx.lineTo(x, margin.top + plotH);
        ctx.stroke();
        ctx.setLineDash([]);

        var text = hoverInfo.freq.toFixed(1) + ' Hz | ' + hoverInfo.mag.toFixed(1) + ' dB';
        ctx.font = '11px JetBrains Mono, monospace';
        var tw = ctx.measureText(text).width;
        var tx = Math.min(x + 10, canvasW - tw - 10);
        var ty = Math.max(y - 10, margin.top + 15);

        ctx.fillStyle = 'rgba(17, 24, 39, 0.9)';
        ctx.fillRect(tx - 4, ty - 12, tw + 8, 18);
        ctx.strokeStyle = '#3d4f65';
        ctx.lineWidth = 1;
        ctx.strokeRect(tx - 4, ty - 12, tw + 8, 18);
        ctx.fillStyle = '#e2e8f0';
        ctx.textAlign = 'left';
        ctx.fillText(text, tx, ty);
    }

    return {
        init: init,
        resize: resize,
        reset: reset,
        addData: addData,
        draw: draw
    };
})();

window.addEventListener('DOMContentLoaded', function () {
    SpectrumChart.init();
});
