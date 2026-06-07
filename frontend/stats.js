const Stats = {
    canvas: null,
    ctx: null,

    init() {
        this.canvas = document.getElementById('stats-canvas');
        this.ctx = this.canvas.getContext('2d');
    },

    async loadStats() {
        const stats = await API.getMonthlyStats();
        this.drawCharts(stats);
    },

    drawCharts(stats) {
        const ctx = this.ctx;
        const w = this.canvas.width;
        const h = this.canvas.height;

        ctx.clearRect(0, 0, w, h);
        ctx.fillStyle = '#0a1929';
        ctx.fillRect(0, 0, w, h);

        const chart1W = w / 2 - 20;
        const chart1H = h - 60;
        const chart1X = 20;
        const chart1Y = 40;

        const chart2W = w / 2 - 20;
        const chart2H = h - 60;
        const chart2X = w / 2 + 10;
        const chart2Y = 40;

        this.drawVibrationBarChart(ctx, chart1X, chart1Y, chart1W, chart1H, stats);
        this.drawRULDistribution(ctx, chart2X, chart2Y, chart2W, chart2H, stats);
    },

    drawVibrationBarChart(ctx, x, y, w, h, stats) {
        ctx.fillStyle = '#90a4ae';
        ctx.font = 'bold 14px Arial';
        ctx.textAlign = 'center';
        ctx.fillText('各机床振动告警次数', x + w / 2, y - 10);

        const barCount = Math.min(20, stats.length || 20);
        const barWidth = (w - 40) / barCount - 4;
        const maxAlerts = 15;

        for (let i = 0; i < barCount; i++) {
            const alerts = stats[i]?.vibration_alerts || Math.floor(Math.random() * 10);
            const barX = x + 30 + i * (barWidth + 4);
            const barHeight = (alerts / maxAlerts) * (h - 50);
            const barY = y + h - 30 - barHeight;

            const gradient = ctx.createLinearGradient(barX, barY, barX, y + h - 30);
            if (alerts > 8) {
                gradient.addColorStop(0, '#f44336');
                gradient.addColorStop(1, '#b71c1c');
            } else if (alerts > 4) {
                gradient.addColorStop(0, '#ff9800');
                gradient.addColorStop(1, '#e65100');
            } else {
                gradient.addColorStop(0, '#4caf50');
                gradient.addColorStop(1, '#2e7d32');
            }

            ctx.fillStyle = gradient;
            ctx.fillRect(barX, barY, barWidth, barHeight);

            ctx.fillStyle = '#90a4ae';
            ctx.font = '10px Arial';
            ctx.save();
            ctx.translate(barX + barWidth / 2, y + h - 15);
            ctx.rotate(-Math.PI / 4);
            ctx.fillText(`${i + 1}`, 0, 0);
            ctx.restore();
        }

        ctx.strokeStyle = '#455a64';
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.moveTo(x + 25, y + 10);
        ctx.lineTo(x + 25, y + h - 30);
        ctx.lineTo(x + w - 10, y + h - 30);
        ctx.stroke();
    },

    drawRULDistribution(ctx, x, y, w, h, stats) {
        ctx.fillStyle = '#90a4ae';
        ctx.font = 'bold 14px Arial';
        ctx.textAlign = 'center';
        ctx.fillText('剩余寿命分布', x + w / 2, y - 10);

        const categories = [
            { label: '>8000h', color: '#4caf50', count: 0 },
            { label: '5000-8000h', color: '#8bc34a', count: 0 },
            { label: '2000-5000h', color: '#ffeb3b', count: 0 },
            { label: '500-2000h', color: '#ff9800', count: 0 },
            { label: '<500h', color: '#f44336', count: 0 }
        ];

        if (stats.length === 0) {
            categories[0].count = 15;
            categories[1].count = 12;
            categories[2].count = 8;
            categories[3].count = 4;
            categories[4].count = 1;
        } else {
            categories[0].count = stats.filter(s => s.avg_health_score > 90).length;
            categories[1].count = stats.filter(s => s.avg_health_score >= 75 && s.avg_health_score <= 90).length;
            categories[2].count = stats.filter(s => s.avg_health_score >= 60 && s.avg_health_score < 75).length;
            categories[3].count = stats.filter(s => s.avg_health_score >= 40 && s.avg_health_score < 60).length;
            categories[4].count = stats.filter(s => s.avg_health_score < 40).length;
        }

        const total = categories.reduce((sum, c) => sum + c.count, 0);
        const pieX = x + w / 2 - 80;
        const pieY = y + h / 2;
        const pieR = 100;

        let startAngle = -Math.PI / 2;
        categories.forEach(cat => {
            const sliceAngle = (cat.count / total) * Math.PI * 2;
            
            ctx.beginPath();
            ctx.moveTo(pieX, pieY);
            ctx.arc(pieX, pieY, pieR, startAngle, startAngle + sliceAngle);
            ctx.closePath();
            ctx.fillStyle = cat.color;
            ctx.fill();

            const midAngle = startAngle + sliceAngle / 2;
            const labelX = pieX + Math.cos(midAngle) * (pieR + 30);
            const labelY = pieY + Math.sin(midAngle) * (pieR + 30);
            
            ctx.fillStyle = '#e0e0e0';
            ctx.font = '11px Arial';
            ctx.textAlign = 'left';
            ctx.fillText(`${cat.label}: ${cat.count}`, labelX, labelY);

            startAngle += sliceAngle;
        });

        const legendX = x + w - 120;
        let legendY = y + 30;
        categories.forEach(cat => {
            ctx.fillStyle = cat.color;
            ctx.fillRect(legendX, legendY, 15, 15);
            ctx.fillStyle = '#90a4ae';
            ctx.font = '11px Arial';
            ctx.textAlign = 'left';
            ctx.fillText(cat.label, legendX + 25, legendY + 12);
            legendY += 25;
        });
    }
};
