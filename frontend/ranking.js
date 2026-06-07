const Ranking = {
    async loadRanking() {
        const ranking = await API.getHealthRanking();
        this.renderRanking(ranking);
    },

    renderRanking(ranking) {
        const tbody = document.getElementById('ranking-tbody');
        tbody.innerHTML = '';

        if (ranking.length === 0) {
            for (let i = 1; i <= 40; i++) {
                const tr = document.createElement('tr');
                tr.innerHTML = `
                    <td class="rank-${i <= 3 ? i : ''}">${i}</td>
                    <td>机床-${i.toString().padStart(2, '0')}</td>
                    <td>${(80 + Math.random() * 20).toFixed(1)}</td>
                    <td>${Math.floor(5000 + Math.random() * 10000)}</td>
                    <td>${this.getAlarmLevelText(i > 35 ? 1 : 0)}</td>
                `;
                tbody.appendChild(tr);
            }
            return;
        }

        ranking.forEach((item, idx) => {
            const tr = document.createElement('tr');
            const rankClass = idx < 3 ? `rank-${idx + 1}` : '';
            tr.innerHTML = `
                <td class="${rankClass}">${item.rank || idx + 1}</td>
                <td>机床-${item.machine_id.toString().padStart(2, '0')}</td>
                <td>${item.health_score.toFixed(1)}</td>
                <td>${item.rul_hours.toFixed(0)}</td>
                <td>${this.getAlarmLevelText(item.alarm_level)}</td>
            `;
            tbody.appendChild(tr);
        });
    },

    getAlarmLevelText(level) {
        switch (level) {
            case 2:
                return '<span style="color: #f44336;">二级更换预警</span>';
            case 1:
                return '<span style="color: #ff9800;">一级振动告警</span>';
            default:
                return '<span style="color: #69f0ae;">正常</span>';
        }
    }
};
