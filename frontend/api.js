const API = {
    async getAllMachines() {
        try {
            const res = await fetch(`${CONFIG.API_BASE_URL}/machines`);
            return await res.json();
        } catch (e) {
            console.error('Failed to fetch machines:', e);
            return [];
        }
    },

    async getMachineById(id) {
        try {
            const res = await fetch(`${CONFIG.API_BASE_URL}/machines/${id}`);
            return await res.json();
        } catch (e) {
            console.error(`Failed to fetch machine ${id}:`, e);
            return null;
        }
    },

    async getSensorHistory(machineId, sensorIdx, hours = 1) {
        try {
            const res = await fetch(`${CONFIG.API_BASE_URL}/machines/${machineId}/sensors/${sensorIdx}?hours=${hours}`);
            return await res.json();
        } catch (e) {
            console.error(`Failed to fetch sensor history:`, e);
            return null;
        }
    },

    async getHealthRanking() {
        try {
            const res = await fetch(`${CONFIG.API_BASE_URL}/ranking`);
            return await res.json();
        } catch (e) {
            console.error('Failed to fetch ranking:', e);
            return [];
        }
    },

    async getMonthlyStats(month) {
        try {
            const url = month 
                ? `${CONFIG.API_BASE_URL}/stats/monthly?month=${month}`
                : `${CONFIG.API_BASE_URL}/stats/monthly`;
            const res = await fetch(url);
            return await res.json();
        } catch (e) {
            console.error('Failed to fetch monthly stats:', e);
            return [];
        }
    },

    connectWebSocket(onMessage) {
        const ws = new WebSocket(CONFIG.WS_URL);
        
        ws.onopen = () => {
            console.log('WebSocket connected');
        };

        ws.onmessage = (event) => {
            try {
                const data = JSON.parse(event.data);
                onMessage(data);
            } catch (e) {
                console.error('Failed to parse WS message:', e);
            }
        };

        ws.onerror = (e) => {
            console.error('WebSocket error:', e);
        };

        ws.onclose = () => {
            console.log('WebSocket disconnected, reconnecting...');
            setTimeout(() => this.connectWebSocket(onMessage), 3000);
        };

        return ws;
    }
};
