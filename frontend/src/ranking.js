var RankingChart = (function () {
    var rankCanvas, rankCtx;
    var pieCanvas, pieCtx;
    var timelineCanvas, timelineCtx;
    var faultData = null;
    var timelineData = null;
    var hoveredRankIdx = -1;

    function init() {
        rankCanvas = document.getElementById('ranking-canvas');
        rankCtx = rankCanvas.getContext('2d');
        pieCanvas = document.getElementById('fault-pie-canvas');
        pieCtx = pieCanvas.getContext('2d');
        timelineCanvas = document.getElementById('alert-timeline-canvas');
        timelineCtx = timelineCanvas.getContext('2d');

        resize();

        rankCanvas.addEventListener('mousemove', function (e) {
            var rect = rankCanvas.getBoundingClientRect();
            var my = e.clientY - rect.top;
            var dpr = window.devicePixelRatio;
            var m = { top: 8, bottom: 8, left: 8, right: 50 };
            var rh = (rankCanvas.height / dpr - m.top - m.bottom) / 40;
            hoveredRankIdx = Math.floor((my - m.top) / rh);
            if (hoveredRankIdx < 0 || hoveredRankIdx >= 40) hoveredRankIdx = -1;
        });
        rankCanvas.addEventListener('mouseleave', function () {
            hoveredRankIdx = -1;
        });
    }

    function resize() {
        setupCanvas(rankCanvas, rankCtx);
        setupCanvas(pieCanvas, pieCtx);
        setupCanvas(timelineCanvas, timelineCtx);
    }

    function setupCanvas(canvas, ctxRef) {
        var rect = canvas.parentElement.getBoundingClientRect();
        var w = rect.width;
        var h = rect.height;
        canvas.width = w * window.devicePixelRatio;
        canvas.height = h * window.devicePixelRatio;
        canvas.style.width = w + 'px';
        canvas.style.height = h + 'px';
        var c = canvas.getContext('2d');
        c.setTransform(window.devicePixelRatio, 0, 0, window.devicePixelRatio, 0, 0);
    }

    function draw(machines) {
        drawRanking(machines);
        if (faultData) drawFaultPie(faultData);
        if (timelineData) drawAlertTimeline(timelineData);
    }

    function drawRanking(machines) {
        if (!rankCtx) return;
        var c = rankCanvas;
        var cx = c.getContext('2d');
        var dpr = window.devicePixelRatio;
        var w = c.width / dpr;
        var h = c.height / dpr;
        cx.clearRect(0, 0, w, h);
        cx.fillStyle = '#0a0e17';
        cx.fillRect(0, 0, w, h);

        var arr = [];
        var keys = Object.keys(machines);
        for (var i = 0; i < keys.length; i++) {
            arr.push(machines[keys[i]]);
        }
        arr.sort(function (a, b) { return a.healthScore - b.healthScore; });

        var m = { top: 8, right: 50, bottom: 8, left: 60 };
        var pw = w - m.left - m.right;
        var ph = h - m.top - m.bottom;
        var barH = ph / arr.length;
        var barGap = Math.max(0.5, barH * 0.15);

        for (var j = 0; j < arr.length; j++) {
            var machine = arr[j];
            var by = m.top + j * barH + barGap / 2;
            var bh = barH - barGap;
            var bw = (machine.healthScore / 100) * pw;

            var barColor = machine.healthScore > 80 ? '#00ff88' :
                machine.healthScore > 60 ? '#ffaa00' : '#ff3366';

            var isHovered = j === hoveredRankIdx;
            var alpha = isHovered ? 0.9 : 0.7;

            cx.fillStyle = 'rgba(' + hexToRgb(barColor) + ',' + alpha + ')';
            cx.fillRect(m.left, by, bw, bh);

            if (isHovered) {
                cx.strokeStyle = '#ffffff';
                cx.lineWidth = 1;
                cx.strokeRect(m.left, by, bw, bh);
            }

            cx.fillStyle = '#94a3b8';
            cx.font = (bh > 8 ? 8 : 6) + 'px JetBrains Mono, monospace';
            cx.textAlign = 'right';
            cx.textBaseline = 'middle';
            cx.fillText(machine.name, m.left - 4, by + bh / 2);

            cx.fillStyle = barColor;
            cx.textAlign = 'left';
            cx.fillText(machine.healthScore + '', m.left + bw + 4, by + bh / 2);
        }
    }

    function drawFaultPie(data) {
        if (!pieCtx) return;
        faultData = data;
        var c = pieCanvas;
        var cx = c.getContext('2d');
        var dpr = window.devicePixelRatio;
        var w = c.width / dpr;
        var h = c.height / dpr;
        cx.clearRect(0, 0, w, h);
        cx.fillStyle = '#0a0e17';
        cx.fillRect(0, 0, w, h);

        var colors = ['#3b82f6', '#ff3366', '#ffaa00', '#00ff88', '#8b5cf6', '#f97316', '#06b6d4', '#ec4899'];
        var total = 0;
        for (var i = 0; i < data.length; i++) total += data[i].count;

        if (total === 0) {
            cx.fillStyle = '#64748b';
            cx.font = '12px Inter, sans-serif';
            cx.textAlign = 'center';
            cx.fillText('暂无故障数据', w / 2, h / 2);
            return;
        }

        var centerX = w * 0.35;
        var centerY = h * 0.5;
        var radius = Math.min(w * 0.3, h * 0.42);

        var startAngle = -Math.PI / 2;
        for (var j = 0; j < data.length; j++) {
            var sliceAngle = (data[j].count / total) * Math.PI * 2;
            cx.beginPath();
            cx.moveTo(centerX, centerY);
            cx.arc(centerX, centerY, radius, startAngle, startAngle + sliceAngle);
            cx.closePath();
            cx.fillStyle = colors[j % colors.length];
            cx.fill();
            cx.strokeStyle = '#0a0e17';
            cx.lineWidth = 2;
            cx.stroke();
            startAngle += sliceAngle;
        }

        cx.beginPath();
        cx.arc(centerX, centerY, radius * 0.45, 0, Math.PI * 2);
        cx.fillStyle = '#0a0e17';
        cx.fill();

        cx.fillStyle = '#e2e8f0';
        cx.font = 'bold 16px JetBrains Mono, monospace';
        cx.textAlign = 'center';
        cx.textBaseline = 'middle';
        cx.fillText(total + '', centerX, centerY - 4);
        cx.fillStyle = '#64748b';
        cx.font = '9px Inter, sans-serif';
        cx.fillText('总故障', centerX, centerY + 12);

        var legendX = w * 0.65;
        var legendY = 16;
        for (var k = 0; k < data.length; k++) {
            var ly = legendY + k * 22;
            if (ly > h - 10) break;
            cx.fillStyle = colors[k % colors.length];
            cx.fillRect(legendX, ly, 10, 10);
            cx.fillStyle = '#94a3b8';
            cx.font = '10px Inter, sans-serif';
            cx.textAlign = 'left';
            cx.textBaseline = 'top';
            var legendText = data[k].type + ' ' + data[k].count;
            cx.fillText(legendText, legendX + 14, ly);
        }
    }

    function drawAlertTimeline(data) {
        if (!timelineCtx) return;
        timelineData = data;
        var c = timelineCanvas;
        var cx = c.getContext('2d');
        var dpr = window.devicePixelRatio;
        var w = c.width / dpr;
        var h = c.height / dpr;
        cx.clearRect(0, 0, w, h);
        cx.fillStyle = '#0a0e17';
        cx.fillRect(0, 0, w, h);

        if (!data || data.length === 0) {
            cx.fillStyle = '#64748b';
            cx.font = '12px Inter, sans-serif';
            cx.textAlign = 'center';
            cx.fillText('暂无告警时间线数据', w / 2, h / 2);
            return;
        }

        var m = { top: 10, right: 10, bottom: 25, left: 35 };
        var pw = w - m.left - m.right;
        var ph = h - m.top - m.bottom;

        var maxCount = 0;
        for (var i = 0; i < data.length; i++) {
            if (data[i].count > maxCount) maxCount = data[i].count;
        }
        maxCount = Math.max(maxCount, 1);

        cx.strokeStyle = 'rgba(45, 58, 77, 0.4)';
        cx.lineWidth = 0.5;
        for (var g = 0; g <= 4; g++) {
            var gy = m.top + ph - (g / 4) * ph;
            cx.beginPath();
            cx.moveTo(m.left, gy);
            cx.lineTo(m.left + pw, gy);
            cx.stroke();
        }

        var barW = Math.max(2, pw / data.length - 1);
        for (var j = 0; j < data.length; j++) {
            var bx = m.left + (j / data.length) * pw + (pw / data.length - barW) / 2;
            var bh = (data[j].count / maxCount) * ph;
            var by = m.top + ph - bh;

            var barColor = data[j].count === 0 ? '#1e293b' :
                data[j].count < 3 ? '#ffaa00' : '#ff3366';
            cx.fillStyle = barColor;
            cx.fillRect(bx, by, barW, bh);
        }

        cx.fillStyle = '#64748b';
        cx.font = '8px JetBrains Mono, monospace';
        cx.textAlign = 'center';
        var labelStep = Math.max(1, Math.floor(data.length / 6));
        for (var l = 0; l < data.length; l += labelStep) {
            var lx = m.left + (l / data.length) * pw + pw / data.length / 2;
            cx.fillText(data[l].date.slice(5), lx, m.top + ph + 14);
        }

        cx.textAlign = 'right';
        for (var y = 0; y <= 4; y++) {
            var yy = m.top + ph - (y / 4) * ph;
            var val = Math.round((y / 4) * maxCount);
            cx.fillText(val + '', m.left - 4, yy + 3);
        }
    }

    function drawFaultStats(data) {
        faultData = data;
        drawFaultPie(data);
    }

    function drawAlertTimelineData(data) {
        timelineData = data;
        drawAlertTimeline(data);
    }

    function hexToRgb(hex) {
        hex = hex.replace('#', '');
        var r = parseInt(hex.substring(0, 2), 16);
        var g = parseInt(hex.substring(2, 4), 16);
        var b = parseInt(hex.substring(4, 6), 16);
        return r + ',' + g + ',' + b;
    }

    return {
        init: init,
        resize: resize,
        draw: draw,
        drawFaultStats: drawFaultStats,
        drawAlertTimeline: drawAlertTimelineData
    };
})();

window.addEventListener('DOMContentLoaded', function () {
    RankingChart.init();
});
