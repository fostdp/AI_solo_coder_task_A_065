const WaterfallCanvas = {
    canvas: null,
    ctx: null,
    width: 800,
    height: 300,
    data: [],
    maxLines: 60,

    init() {
        this.canvas = document.getElementById('waterfall-canvas');
        this.ctx = this.canvas.getContext('2d');
        this.draw();
    },

    addSpectrum(spectrum) {
        this.data.push(spectrum);
        if (this.data.length > this.maxLines) {
            this.data.shift();
        }
        this.draw();
    },

    draw() {
        const ctx = this.ctx;
        ctx.clearRect(0, 0, this.width, this.height);

        if (this.data.length === 0) {
            ctx.fillStyle = '#1a1a2e';
            ctx.fillRect(0, 0, this.width, this.height);
            ctx.fillStyle = '#607d8b';
            ctx.font = '14px Arial';
            ctx.textAlign = 'center';
            ctx.fillText('等待频谱数据...', this.width / 2, this.height / 2);
            return;
        }

        const lineHeight = this.height / this.maxLines;
        const maxFreq = 128;

        for (let i = 0; i < this.data.length; i++) {
            const spectrum = this.data[i];
            const y = this.height - (this.data.length - i) * lineHeight;
            
            for (let j = 0; j < maxFreq; j++) {
                const x = (j / maxFreq) * this.width;
                const value = spectrum[j] || 0;
                const color = this.getValueColor(value);
                
                ctx.fillStyle = color;
                ctx.fillRect(x, y, this.width / maxFreq + 1, lineHeight + 1);
            }
        }

        this.drawAxes();
    },

    getValueColor(value) {
        const normalized = Math.min(value / 5.0, 1.0);
        
        if (normalized < 0.25) {
            return `rgb(0, 0, ${Math.floor(normalized * 4 * 255)})`;
        } else if (normalized < 0.5) {
            const t = (normalized - 0.25) * 4;
            return `rgb(0, ${Math.floor(t * 255)}, 255)`;
        } else if (normalized < 0.75) {
            const t = (normalized - 0.5) * 4;
            return `rgb(${Math.floor(t * 255)}, 255, ${Math.floor((1 - t) * 255)})`;
        } else {
            const t = (normalized - 0.75) * 4;
            return `rgb(255, ${Math.floor((1 - t) * 255)}, 0)`;
        }
    },

    drawAxes() {
        const ctx = this.ctx;
        
        ctx.strokeStyle = '#455a64';
        ctx.lineWidth = 1;
        
        ctx.beginPath();
        ctx.moveTo(50, 0);
        ctx.lineTo(50, this.height);
        ctx.stroke();

        ctx.beginPath();
        ctx.moveTo(0, this.height - 25);
        ctx.lineTo(this.width, this.height - 25);
        ctx.stroke();

        ctx.fillStyle = '#90a4ae';
        ctx.font = '10px Arial';
        ctx.textAlign = 'right';
        for (let i = 0; i <= 5; i++) {
            const y = (i / 5) * (this.height - 25);
            ctx.fillText(`${Math.round((1 - i / 5) * 60)}s`, 45, y + 4);
        }

        ctx.textAlign = 'center';
        for (let i = 0; i <= 5; i++) {
            const x = 50 + (i / 5) * (this.width - 50);
            ctx.fillText(`${Math.round(i / 5 * 500)}Hz`, x, this.height - 10);
        }
    }
};
