var SpindleDiagram = (function () {
    var canvas, ctx;
    var canvasW, canvasH;
    var rotationAngle = 0;
    var animFrameId = null;
    var sensorHitAreas = [];
    var hoveredSensor = null;
    var glowPhase = 0;

    function init() {
        canvas = document.getElementById('spindle-canvas');
        ctx = canvas.getContext('2d');
        resize();
        canvas.addEventListener('click', handleClick);
        canvas.addEventListener('mousemove', handleMouseMove);
        canvas.addEventListener('mouseleave', function () {
            hoveredSensor = null;
            document.getElementById('tooltip').style.display = 'none';
        });
        startAnimation();
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

    function startAnimation() {
        function loop() {
            glowPhase += 0.05;
            if (glowPhase > Math.PI * 2) glowPhase = 0;
            var machine = App.getSelectedMachine();
            if (machine) {
                rotationAngle += (machine.rpm / 60) * 0.016 * Math.PI * 2 * 0.01;
            }
            draw(machine);
            animFrameId = requestAnimationFrame(loop);
        }
        loop();
    }

    function draw(machine) {
        if (!ctx) return;
        ctx.clearRect(0, 0, canvasW, canvasH);

        if (!machine) {
            ctx.fillStyle = '#64748b';
            ctx.font = '16px Inter, sans-serif';
            ctx.textAlign = 'center';
            ctx.fillText('请选择机床', canvasW / 2, canvasH / 2);
            return;
        }

        var cx = canvasW * 0.5;
        var cy = canvasH * 0.5;
        var shaftLen = canvasW * 0.7;
        var shaftH = canvasH * 0.16;
        var shaftX = cx - shaftLen / 2;

        drawBackground(cx, cy);
        drawHousing(shaftX, cy, shaftLen, shaftH * 2.5);
        drawShaft(shaftX, cy, shaftLen, shaftH, machine.rpm);
        drawFrontBearing(shaftX + shaftLen * 0.12, cy, shaftH);
        drawRearBearing(shaftX + shaftLen * 0.72, cy, shaftH);
        drawMotorRotor(shaftX + shaftLen * 0.85, cy, shaftH * 1.6);
        drawToolHolder(shaftX, cy, shaftH * 0.7);
        drawLabels(shaftX, cy, shaftLen, shaftH);
        drawSensors(machine, shaftX, cy, shaftLen, shaftH);
        drawMachineInfo(machine);
    }

    function drawBackground(cx, cy) {
        var grad = ctx.createRadialGradient(cx, cy, 0, cx, cy, canvasW * 0.5);
        grad.addColorStop(0, 'rgba(59, 130, 246, 0.03)');
        grad.addColorStop(1, 'rgba(10, 14, 23, 0)');
        ctx.fillStyle = grad;
        ctx.fillRect(0, 0, canvasW, canvasH);
    }

    function drawHousing(x, cy, len, h) {
        var hh = h / 2;
        ctx.fillStyle = '#1a2332';
        ctx.strokeStyle = '#2d3a4d';
        ctx.lineWidth = 1.5;
        roundRect(ctx, x - 20, cy - hh, len + 40, h, 8);
        ctx.fill();
        ctx.stroke();

        ctx.fillStyle = '#111827';
        ctx.strokeStyle = '#1e293b';
        ctx.lineWidth = 1;
        var innerPad = 8;
        roundRect(ctx, x - 10, cy - hh + innerPad, len + 20, h - innerPad * 2, 4);
        ctx.fill();
        ctx.stroke();
    }

    function drawShaft(x, cy, len, h, rpm) {
        var hh = h / 2;
        var grad = ctx.createLinearGradient(x, cy - hh, x, cy + hh);
        grad.addColorStop(0, '#4a5568');
        grad.addColorStop(0.3, '#718096');
        grad.addColorStop(0.5, '#a0aec0');
        grad.addColorStop(0.7, '#718096');
        grad.addColorStop(1, '#4a5568');

        ctx.fillStyle = grad;
        ctx.beginPath();
        ctx.roundRect(x, cy - hh, len, h, 2);
        ctx.fill();

        if (rpm > 0) {
            ctx.save();
            ctx.beginPath();
            ctx.roundRect(x, cy - hh, len, h, 2);
            ctx.clip();
            ctx.strokeStyle = 'rgba(160, 174, 192, 0.15)';
            ctx.lineWidth = 1;
            var stripeW = 12;
            var offset = (rotationAngle * 20) % (stripeW * 2);
            for (var sx = -stripeW * 2 + offset; sx < len + stripeW * 2; sx += stripeW * 2) {
                ctx.beginPath();
                ctx.moveTo(x + sx, cy - hh);
                ctx.lineTo(x + sx + stripeW, cy - hh);
                ctx.lineTo(x + sx + stripeW - h * 0.3, cy + hh);
                ctx.lineTo(x + sx - h * 0.3, cy + hh);
                ctx.closePath();
                ctx.stroke();
            }
            ctx.restore();
        }

        ctx.strokeStyle = '#2d3a4d';
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.roundRect(x, cy - hh, len, h, 2);
        ctx.stroke();

        var keywayX = x + len * 0.2;
        var keywayW = len * 0.15;
        ctx.fillStyle = '#2d3a4d';
        ctx.fillRect(keywayX, cy - hh - 2, keywayW, 4);
    }

    function drawFrontBearing(bx, cy, shaftH) {
        var bw = 28;
        var bh = shaftH * 2.2;
        drawBearingPair(bx, cy, bw, bh, '前轴承');
    }

    function drawRearBearing(bx, cy, shaftH) {
        var bw = 24;
        var bh = shaftH * 1.9;
        drawBearingPair(bx, cy, bw, bh, '后轴承');
    }

    function drawBearingPair(bx, cy, bw, bh, label) {
        var hh = bh / 2;
        var outerGrad = ctx.createLinearGradient(bx - bw / 2, cy - hh, bx + bw / 2, cy - hh);
        outerGrad.addColorStop(0, '#2d3a4d');
        outerGrad.addColorStop(0.5, '#4a5568');
        outerGrad.addColorStop(1, '#2d3a4d');

        ctx.fillStyle = outerGrad;
        roundRect(ctx, bx - bw / 2, cy - hh, bw, bh, 3);
        ctx.fill();
        ctx.strokeStyle = '#3d4f65';
        ctx.lineWidth = 1;
        roundRect(ctx, bx - bw / 2, cy - hh, bw, bh, 3);
        ctx.stroke();

        ctx.strokeStyle = '#1e293b';
        ctx.lineWidth = 1;
        var ballR = 4;
        var ballCount = Math.floor(bh / 14);
        for (var i = 0; i < ballCount; i++) {
            var by = cy - hh + 10 + i * (bh - 20) / Math.max(1, ballCount - 1);
            ctx.beginPath();
            ctx.arc(bx, by, ballR, 0, Math.PI * 2);
            ctx.fillStyle = '#718096';
            ctx.fill();
            ctx.strokeStyle = '#4a5568';
            ctx.stroke();
        }

        ctx.fillStyle = '#4a5568';
        ctx.fillRect(bx - 3, cy - hh, 6, bh);
    }

    function drawMotorRotor(mx, cy, size) {
        var hs = size / 2;
        var grad = ctx.createRadialGradient(mx, cy, 0, mx, cy, hs);
        grad.addColorStop(0, '#2d3a4d');
        grad.addColorStop(0.7, '#1a2332');
        grad.addColorStop(1, '#111827');

        ctx.fillStyle = grad;
        ctx.beginPath();
        ctx.arc(mx, cy, hs, 0, Math.PI * 2);
        ctx.fill();
        ctx.strokeStyle = '#3d4f65';
        ctx.lineWidth = 1.5;
        ctx.stroke();

        ctx.strokeStyle = '#4a5568';
        ctx.lineWidth = 1;
        for (var i = 0; i < 12; i++) {
            var a = (i / 12) * Math.PI * 2 + rotationAngle;
            ctx.beginPath();
            ctx.moveTo(mx + Math.cos(a) * 10, cy + Math.sin(a) * 10);
            ctx.lineTo(mx + Math.cos(a) * (hs - 4), cy + Math.sin(a) * (hs - 4));
            ctx.stroke();
        }

        ctx.fillStyle = '#1a2332';
        ctx.beginPath();
        ctx.arc(mx, cy, 10, 0, Math.PI * 2);
        ctx.fill();
        ctx.strokeStyle = '#4a5568';
        ctx.stroke();

        var windAngle = rotationAngle * 0.5;
        ctx.strokeStyle = 'rgba(59, 130, 246, 0.3)';
        ctx.lineWidth = 2;
        for (var w = 0; w < 3; w++) {
            var wa = windAngle + (w / 3) * Math.PI * 2;
            ctx.beginPath();
            ctx.arc(mx, cy, hs * 0.7, wa, wa + 0.8);
            ctx.stroke();
        }
    }

    function drawToolHolder(tx, cy, size) {
        var hw = 40;
        var hh = size;
        ctx.fillStyle = '#4a5568';
        ctx.beginPath();
        ctx.moveTo(tx, cy - hh / 2);
        ctx.lineTo(tx - hw, cy - hh * 0.35);
        ctx.lineTo(tx - hw, cy + hh * 0.35);
        ctx.lineTo(tx, cy + hh / 2);
        ctx.closePath();
        ctx.fill();
        ctx.strokeStyle = '#2d3a4d';
        ctx.lineWidth = 1;
        ctx.stroke();

        ctx.fillStyle = '#2d3a4d';
        ctx.beginPath();
        ctx.moveTo(tx, cy - 3);
        ctx.lineTo(tx - hw * 0.6, cy - hh * 0.2);
        ctx.lineTo(tx - hw * 0.6, cy + hh * 0.2);
        ctx.lineTo(tx, cy + 3);
        ctx.closePath();
        ctx.fill();
    }

    function drawLabels(shaftX, cy, shaftLen, shaftH) {
        ctx.fillStyle = '#64748b';
        ctx.font = '11px Inter, sans-serif';
        ctx.textAlign = 'center';
        ctx.fillText('刀柄锥度', shaftX - 25, cy + shaftH * 2.5);
        ctx.fillText('前轴承', shaftX + shaftLen * 0.12, cy + shaftH * 2.5);
        ctx.fillText('主轴中段', shaftX + shaftLen * 0.42, cy + shaftH * 2.5);
        ctx.fillText('后轴承', shaftX + shaftLen * 0.72, cy + shaftH * 2.5);
        ctx.fillText('电机转子', shaftX + shaftLen * 0.85, cy + shaftH * 2.5);
    }

    function drawSensors(machine, shaftX, cy, shaftLen, shaftH) {
        sensorHitAreas = [];
        var sensors = machine.sensors;

        for (var i = 0; i < sensors.length; i++) {
            var s = sensors[i];
            var sx = shaftX + s.px * shaftLen;
            var sy = cy + (s.py - 0.5) * shaftH * 3.5;
            var radius = s.type === 'vibration' ? 10 : 7;
            var color = getSensorColor(s);
            var isAlert = isSensorAlert(s);
            var isHovered = hoveredSensor === s.id;

            if (isAlert) {
                var glowAlpha = 0.2 + Math.sin(glowPhase) * 0.15;
                var glowR = radius + 8 + Math.sin(glowPhase) * 4;
                ctx.beginPath();
                ctx.arc(sx, sy, glowR, 0, Math.PI * 2);
                ctx.fillStyle = color.replace(')', ',' + glowAlpha + ')').replace('rgb', 'rgba');
                ctx.fill();
            }

            ctx.beginPath();
            ctx.arc(sx, sy, radius, 0, Math.PI * 2);
            ctx.fillStyle = isHovered ? '#ffffff' : color;
            ctx.fill();

            ctx.strokeStyle = isHovered ? '#ffffff' : 'rgba(255,255,255,0.3)';
            ctx.lineWidth = isHovered ? 2 : 1;
            ctx.stroke();

            if (s.type === 'vibration') {
                ctx.fillStyle = '#e2e8f0';
                ctx.font = '8px JetBrains Mono, monospace';
                ctx.textAlign = 'center';
                ctx.fillText(s.rms !== undefined ? s.rms.toFixed(1) : s.value.toFixed(1), sx, sy + radius + 14);
            }

            sensorHitAreas.push({
                id: s.id,
                x: sx,
                y: sy,
                r: radius + 4
            });
        }
    }

    function getSensorColor(s) {
        if (s.type === 'temperature') {
            var v = s.value;
            if (v < 45) return 'rgb(0, 255, 136)';
            if (v < 65) return 'rgb(255, 170, 0)';
            return 'rgb(255, 51, 102)';
        }
        if (s.type === 'displacement') {
            var d = s.value;
            if (d < 5) return 'rgb(0, 255, 136)';
            if (d < 8) return 'rgb(255, 170, 0)';
            return 'rgb(255, 51, 102)';
        }
        var vib = s.rms !== undefined ? s.rms : s.value;
        if (vib < 2.8) return 'rgb(0, 255, 136)';
        if (vib < 7.1) return 'rgb(255, 170, 0)';
        return 'rgb(255, 51, 102)';
    }

    function isSensorAlert(s) {
        if (s.type === 'vibration') {
            var v = s.rms !== undefined ? s.rms : s.value;
            return v >= 7.1;
        }
        if (s.type === 'temperature') return s.value >= 65;
        if (s.type === 'displacement') return s.value >= 8;
        return false;
    }

    function drawMachineInfo(machine) {
        ctx.fillStyle = '#e2e8f0';
        ctx.font = 'bold 16px Inter, sans-serif';
        ctx.textAlign = 'left';
        ctx.fillText(machine.name + ' - ' + machine.type, 20, 28);

        ctx.fillStyle = '#94a3b8';
        ctx.font = '12px JetBrains Mono, monospace';
        ctx.fillText('RPM: ' + machine.rpm + '  |  健康度: ' + machine.healthScore + '%', 20, 48);

        var healthColor = machine.healthScore > 80 ? '#00ff88' : machine.healthScore > 60 ? '#ffaa00' : '#ff3366';
        ctx.fillStyle = healthColor;
        var barW = 120;
        var barH = 4;
        ctx.fillRect(20, 54, barW * (machine.healthScore / 100), barH);
        ctx.strokeStyle = '#1e293b';
        ctx.strokeRect(20, 54, barW, barH);
    }

    function handleClick(e) {
        var rect = canvas.getBoundingClientRect();
        var mx = e.clientX - rect.left;
        var my = e.clientY - rect.top;
        for (var i = 0; i < sensorHitAreas.length; i++) {
            var a = sensorHitAreas[i];
            var dx = mx - a.x;
            var dy = my - a.y;
            if (dx * dx + dy * dy <= a.r * a.r) {
                App.selectSensor(a.id);
                return;
            }
        }
    }

    function handleMouseMove(e) {
        var rect = canvas.getBoundingClientRect();
        var mx = e.clientX - rect.left;
        var my = e.clientY - rect.top;
        var found = null;
        for (var i = 0; i < sensorHitAreas.length; i++) {
            var a = sensorHitAreas[i];
            var dx = mx - a.x;
            var dy = my - a.y;
            if (dx * dx + dy * dy <= a.r * a.r) {
                found = a;
                break;
            }
        }
        if (found) {
            canvas.style.cursor = 'pointer';
            hoveredSensor = found.id;
            var machine = App.getSelectedMachine();
            if (machine) {
                var sensor = null;
                for (var j = 0; j < machine.sensors.length; j++) {
                    if (machine.sensors[j].id === found.id) {
                        sensor = machine.sensors[j];
                        break;
                    }
                }
                if (sensor) {
                    var tooltip = document.getElementById('tooltip');
                    tooltip.style.display = 'block';
                    tooltip.style.left = (e.clientX + 12) + 'px';
                    tooltip.style.top = (e.clientY - 10) + 'px';
                    tooltip.textContent = sensor.name + ' | ' + sensor.value.toFixed(2) + ' ' + sensor.unit;
                }
            }
        } else {
            canvas.style.cursor = 'default';
            hoveredSensor = null;
            document.getElementById('tooltip').style.display = 'none';
        }
    }

    function roundRect(ctx, x, y, w, h, r) {
        ctx.beginPath();
        ctx.moveTo(x + r, y);
        ctx.lineTo(x + w - r, y);
        ctx.quadraticCurveTo(x + w, y, x + w, y + r);
        ctx.lineTo(x + w, y + h - r);
        ctx.quadraticCurveTo(x + w, y + h, x + w - r, y + h);
        ctx.lineTo(x + r, y + h);
        ctx.quadraticCurveTo(x, y + h, x, y + h - r);
        ctx.lineTo(x, y + r);
        ctx.quadraticCurveTo(x, y, x + r, y);
        ctx.closePath();
    }

    return {
        init: init,
        draw: draw,
        resize: resize
    };
})();

window.addEventListener('DOMContentLoaded', function () {
    SpindleDiagram.init();
});
