import { Chart, registerables } from 'chart.js';
import { SiegeTower3D } from './siege_tower_3d.js';
Chart.register(...registerables);

const API_BASE = '';

const DEFAULT_TOWERS = {
    1: {
        tower_id: 1, tower_name: '临冲吕公车-一号', build_date: '1450-03-15',
        material: '杉木+铁木', total_height: 18.5, total_layers: 5,
        base_width: 6.2, base_depth: 4.8, total_weight: 28.5,
        design_load: 850.0, design_wind_speed: 35.0,
        material_strength: 38.0, elastic_modulus: 9500.0, poisson_ratio: 0.35,
    },
    2: {
        tower_id: 2, tower_name: '临冲吕公车-二号', build_date: '1452-07-22',
        material: '柏木+楠木', total_height: 21.0, total_layers: 6,
        base_width: 6.8, base_depth: 5.2, total_weight: 36.8,
        design_load: 1020.0, design_wind_speed: 40.0,
        material_strength: 44.0, elastic_modulus: 12000.0, poisson_ratio: 0.35,
    },
    3: {
        tower_id: 3, tower_name: '云梯车', build_date: '1368-05-10',
        material: '松木+竹', total_height: 12.0, total_layers: 3,
        base_width: 3.5, base_depth: 2.8, total_weight: 8.5,
        design_load: 280.0, design_wind_speed: 25.0,
        material_strength: 36.0, elastic_modulus: 10500.0, poisson_ratio: 0.35,
    },
    4: {
        tower_id: 4, tower_name: '冲车', build_date: '0230-01-01',
        material: '栎木+铁箍', total_height: 5.5, total_layers: 2,
        base_width: 4.2, base_depth: 3.0, total_weight: 15.0,
        design_load: 450.0, design_wind_speed: 20.0,
        material_strength: 48.0, elastic_modulus: 13800.0, poisson_ratio: 0.35,
    },
    5: {
        tower_id: 5, tower_name: '现代塔吊', build_date: '2024-01-15',
        material: 'Q345B钢材(GB/T1591)', total_height: 60.0, total_layers: 12,
        base_width: 8.0, base_depth: 8.0, total_weight: 95.0,
        design_load: 8000.0, design_wind_speed: 55.0,
        material_strength: 295.0, elastic_modulus: 206000.0, poisson_ratio: 0.30,
    },
};

const SOIL_NAMES = {
    sand: { name: '砂土', icon: '🏖️' },
    clay: { name: '黏土', icon: '🟫' },
    silt: { name: '粉土', icon: '🏜️' },
    rock: { name: '岩层', icon: '🪨' },
    loam: { name: '壤土', icon: '🌱' },
};

const ALERT_NAMES = {
    tilt_exceed: '倾斜超限',
    stress_critical: '应力临界',
    wind_overload: '风荷载超载',
    ground_failure: '地面承载失效',
    vibration_exceed: '振动超限共振',
    structure_instability: '结构失稳',
};

export class StabilityPanel {
    constructor() {
        this.currentTower = DEFAULT_TOWERS[1];
        this.tower3D = null;
        this.layerChart = null;
        this.historyChart = null;
        this.stressHistory = [];
        this.analysisSSE = null;
        this.sensorSSE = null;
        this.alertSSE = null;
        this.currentChartMode = 'stress';
        this.climbingMode = false;
        this.savedCameraState = null;
    }

    init() {
        this.init3DViewer();
        this.initCharts();
        this.loadTowerInfo();
        this.bindEvents();
        this.initLayerControlBar();
        this.loadInitialAnalysis();
        this.loadGroundAnalysis();
        this.connectSSE();
        this.setConnectionStatus('connected', '已连接');
        this.loadDynastyComparison();
        this.loadCrossEraComparison();
        this.loadMoatAnalysis();
        this.initClimbingExperience();
    }

    init3DViewer() {
        const canvas = document.getElementById('towerCanvas');
        this.tower3D = new SiegeTower3D(canvas, this.currentTower);
        const dummyStresses = this.tower3D.init();
        this.updateLayerStressLabels(dummyStresses);
    }

    initCharts() {
        const mainCtx = document.getElementById('layerChart').getContext('2d');
        const histCtx = document.getElementById('historyChart').getContext('2d');

        this.layerChart = new Chart(mainCtx, {
            type: 'bar',
            data: {
                labels: Array.from({ length: this.currentTower.total_layers }, (_, i) => `第${i + 1}层`),
                datasets: [{
                    label: '应力 (MPa)',
                    data: Array(this.currentTower.total_layers).fill(0),
                    backgroundColor: ctx => {
                        const max = this.currentTower.material_strength;
                        const v = ctx.parsed?.y || 0;
                        const r = Math.min(v / max, 1);
                        const hue = 120 - r * 120;
                        return `hsl(${hue}, 70%, 50%)`;
                    },
                    borderRadius: 6,
                    borderSkipped: false,
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                plugins: {
                    legend: { display: true, labels: { color: '#94a3b8', font: { size: 11 } } },
                    tooltip: {
                        backgroundColor: 'rgba(26, 36, 56, 0.95)',
                        titleColor: '#e8edf5',
                        bodyColor: '#94a3b8',
                        borderColor: '#2a3a5c',
                        borderWidth: 1,
                    }
                },
                scales: {
                    x: {
                        grid: { color: 'rgba(42, 58, 92, 0.5)' },
                        ticks: { color: '#94a3b8', font: { size: 10 } }
                    },
                    y: {
                        grid: { color: 'rgba(42, 58, 92, 0.5)' },
                        ticks: { color: '#94a3b8', font: { size: 10 } },
                        beginAtZero: true,
                    }
                },
                animation: { duration: 800, easing: 'easeOutCubic' }
            }
        });

        this.historyChart = new Chart(histCtx, {
            type: 'line',
            data: {
                labels: [],
                datasets: [
                    {
                        label: '最大应力',
                        data: [],
                        borderColor: '#ef4444',
                        backgroundColor: 'rgba(239, 68, 68, 0.15)',
                        fill: true,
                        tension: 0.4,
                        pointRadius: 0,
                        borderWidth: 2,
                    },
                    {
                        label: '临界值',
                        data: [],
                        borderColor: '#f59e0b',
                        borderDash: [4, 4],
                        borderWidth: 1.5,
                        pointRadius: 0,
                        fill: false,
                    }
                ]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                plugins: {
                    legend: { display: true, labels: { color: '#94a3b8', font: { size: 10 }, boxWidth: 12 } },
                    tooltip: {
                        backgroundColor: 'rgba(26, 36, 56, 0.95)',
                        titleColor: '#e8edf5',
                        bodyColor: '#94a3b8',
                    }
                },
                scales: {
                    x: {
                        grid: { display: false },
                        ticks: { color: '#64748b', font: { size: 9 }, maxTicksLimit: 6 }
                    },
                    y: {
                        grid: { color: 'rgba(42, 58, 92, 0.3)' },
                        ticks: { color: '#94a3b8', font: { size: 9 } },
                        beginAtZero: true,
                    }
                },
                animation: { duration: 500 }
            }
        });
    }

    loadTowerInfo() {
        const info = document.getElementById('towerInfo');
        const fields = [
            ['塔号', `#${this.currentTower.tower_id}`],
            ['建造年代', this.currentTower.build_date],
            ['材质结构', this.currentTower.material],
            ['总高度', `${this.currentTower.total_height} m`],
            ['层数', `${this.currentTower.total_layers} 层`],
            ['底宽 × 底深', `${this.currentTower.base_width} × ${this.currentTower.base_depth} m`],
            ['总重量', `${this.currentTower.total_weight} 吨`],
            ['设计荷载', `${this.currentTower.design_load} kN`],
            ['设计风速', `${this.currentTower.design_wind_speed} m/s`],
            ['材料强度', `${this.currentTower.material_strength} MPa`],
            ['弹性模量', `${(this.currentTower.elastic_modulus / 1000).toFixed(1)} GPa`],
            ['泊松比', this.currentTower.poisson_ratio.toFixed(2)],
        ];
        info.innerHTML = fields.map(([k, v]) => `
            <div class="info-row"><span class="k">${k}</span><span class="v">${v}</span></div>
        `).join('');
    }

    bindEvents() {
        document.getElementById('towerSelect').addEventListener('change', e => {
            const id = parseInt(e.target.value);
            this.currentTower = DEFAULT_TOWERS[id];
            this.restart();
        });

        document.querySelectorAll('.view-controls .view-btn[data-view]').forEach(btn => {
            btn.addEventListener('click', () => {
                document.querySelectorAll('.view-controls .view-btn[data-view]').forEach(b => b.classList.remove('active'));
                btn.classList.add('active');
                this.tower3D.setCameraView(btn.dataset.view);
            });
        });

        document.getElementById('btnStressView').addEventListener('click', () => {
            document.getElementById('btnStressView').classList.add('active');
            document.getElementById('btnStructView').classList.remove('active');
            this.tower3D.setStressView(true);
        });
        document.getElementById('btnStructView').addEventListener('click', () => {
            document.getElementById('btnStructView').classList.add('active');
            document.getElementById('btnStressView').classList.remove('active');
            this.tower3D.setStressView(false);
        });

        document.querySelectorAll('.cut-group .view-btn[data-cut]').forEach(btn => {
            btn.addEventListener('click', () => {
                document.querySelectorAll('.cut-group .view-btn[data-cut]').forEach(b => b.classList.remove('active'));
                btn.classList.add('active');
                this.tower3D.setCutMode(btn.dataset.cut);
            });
        });

        document.getElementById('explodeSlider').addEventListener('input', e => {
            this.tower3D.applyExplosion(e.target.value);
        });

        document.querySelectorAll('.chart-switch .sw').forEach(btn => {
            btn.addEventListener('click', () => {
                document.querySelectorAll('.chart-switch .sw').forEach(b => b.classList.remove('active'));
                btn.classList.add('active');
                this.currentChartMode = btn.dataset.chart;
                this.updateChartMode();
            });
        });

        document.getElementById('groundAnalyzeBtn').addEventListener('click', () => this.loadGroundAnalysis());
        document.getElementById('simulateBtn').addEventListener('click', () => this.runSimulation());
        document.getElementById('moatAnalyzeBtn')?.addEventListener('click', () => this.loadMoatAnalysis());
        document.getElementById('climbingBtn')?.addEventListener('click', () => {
            if (this.climbingMode) {
                this.exitClimbingMode();
            } else {
                this.enterClimbingMode({ position: { x: 0, y: 2, z: 5 }, look_at: { x: 0, y: 5, z: 0 } });
            }
        });
        document.getElementById('exitClimbingBtn')?.addEventListener('click', () => this.exitClimbingMode());
    }

    initLayerControlBar() {
        const bar = document.getElementById('layerControlBar');
        if (!bar || !this.tower3D) return;
        bar.innerHTML = '';

        for (let i = 1; i <= this.currentTower.total_layers; i++) {
            const chip = document.createElement('div');
            chip.className = 'layer-chip';
            chip.dataset.layer = i;
            chip.innerHTML = `
                <label class="chip-check">
                    <input type="checkbox" class="layer-visible" data-layer="${i}" checked>
                    <span>L${i}</span>
                </label>
                <div class="chip-meta">
                    <input type="range" class="layer-opacity" data-layer="${i}" min="0.1" max="1" step="0.05" value="1">
                    <span class="chip-stress" id="layerStressLbl${i}">--</span>
                </div>
            `;
            bar.appendChild(chip);
        }

        bar.querySelectorAll('.layer-visible').forEach(cb => {
            cb.addEventListener('change', e => {
                const l = parseInt(e.target.dataset.layer);
                this.tower3D.setLayerVisible(l, e.target.checked);
            });
        });

        bar.querySelectorAll('.layer-opacity').forEach(sl => {
            sl.addEventListener('input', e => {
                const l = parseInt(e.target.dataset.layer);
                this.tower3D.setLayerOpacity(l, e.target.value);
            });
        });
    }

    updateLayerStressLabels(stresses) {
        if (!stresses) return;
        stresses.forEach(s => {
            const lbl = document.getElementById(`layerStressLbl${s.layer}`);
            if (lbl) lbl.textContent = `${s.stress?.toFixed?.(1) || '--'} MPa`;
        });
    }

    restart() {
        if (this.tower3D) this.tower3D.dispose();
        this.stressHistory = [];
        this.closeSSE();
        if (this.layerChart) this.layerChart.destroy();
        if (this.historyChart) this.historyChart.destroy();

        this.loadTowerInfo();
        this.init3DViewer();
        this.initCharts();
        this.initLayerControlBar();
        this.loadInitialAnalysis();
        this.loadGroundAnalysis();
        this.connectSSE();
    }

    updateChartMode() {
        if (!this.layerChart) return;
        const mode = this.currentChartMode;

        this.layerChart.data.datasets = [{
            data: Array(this.currentTower.total_layers).fill(0),
            borderRadius: 6, borderSkipped: false,
        }];

        if (mode === 'stress') {
            this.layerChart.data.datasets[0].label = '应力 (MPa)';
            this.layerChart.data.datasets[0].backgroundColor = ctx => {
                const max = this.currentTower.material_strength;
                const v = ctx.parsed?.y || 0;
                const r = Math.min(v / max, 1);
                const hue = 120 - r * 120;
                return `hsl(${hue}, 70%, 50%)`;
            };
        } else if (mode === 'tilt') {
            this.layerChart.data.datasets[0].label = '倾斜 (°)';
            this.layerChart.data.datasets[0].backgroundColor = ctx => {
                const v = ctx.parsed?.y || 0;
                const r = Math.min(v / 5, 1);
                const hue = 120 - r * 120;
                return `hsl(${hue}, 70%, 50%)`;
            };
        } else {
            this.layerChart.data.datasets[0].label = '风荷载 (N/m²)';
            this.layerChart.data.datasets[0].backgroundColor = ctx => {
                const v = ctx.parsed?.y || 0;
                const r = Math.min(v / 500, 1);
                const hue = 200 - r * 140;
                return `hsl(${hue}, 70%, 50%)`;
            };
        }
        this.layerChart.update('none');
    }

    async loadInitialAnalysis() {
        try {
            const resp = await fetch(`${API_BASE}/api/towers/${this.currentTower.tower_id}/analysis`);
            const json = await resp.json();
            if (json.code === 200 && json.data) {
                this.updateAnalysisPanel(json.data);
            }
        } catch (e) {
            this.updateAnalysisPanel(this.generateMockAnalysis());
        }
    }

    async loadGroundAnalysis() {
        const ws = parseFloat(document.getElementById('windInput')?.value || 20);
        const tl = parseFloat(document.getElementById('tiltInput')?.value || 1);
        const moist = parseFloat(document.getElementById('moistureInput')?.value || 50);

        let data = null;
        try {
            const resp = await fetch(
                `${API_BASE}/api/towers/${this.currentTower.tower_id}/ground?wind_speed=${ws}&tilt_deg=${tl}&moisture_pct=${moist}`
            );
            const json = await resp.json();
            if (json.code === 200) data = json.data;
        } catch (e) {}

        if (!data) data = this.generateMockGround(ws, tl);
        this.renderGroundAnalysis(data);
    }

    renderGroundAnalysis(grounds) {
        const grid = document.getElementById('groundGrid');
        grid.innerHTML = grounds.map(g => {
            const info = SOIL_NAMES[g.soil_type] || { name: g.soil_type, icon: '🏞️' };
            const scoreCls = g.passability_score >= 75 ? 'good' : (g.passability_score >= 50 ? 'mid' : 'bad');
            const riskCls = g.risk_level === 1 ? 'low' : (g.risk_level === 2 ? 'med' : 'high');
            return `
                <div class="ground-card risk-${riskCls} ${g.can_pass ? 'can-pass' : 'cannot-pass'} score-${scoreCls}">
                    <div class="ground-soil-name"><span class="ground-soil-icon">${info.icon}</span> ${info.name}</div>
                    <div class="ground-score">${g.passability_score.toFixed(0)}</div>
                    <div class="ground-detail">
                        <div class="ground-detail-row"><span>承载力</span><span>${g.bearing_capacity.toFixed(0)} kPa</span></div>
                        <div class="ground-detail-row"><span>施加压力</span><span>${g.applied_pressure.toFixed(1)} kPa</span></div>
                        <div class="ground-detail-row"><span>安全系数</span><span>${g.safety_factor.toFixed(2)}</span></div>
                        <div class="ground-detail-row"><span>预计沉降</span><span>${g.settlement.toFixed(1)} mm</span></div>
                        <div class="ground-detail-row"><span>差异沉降</span><span>${g.differential_settlement.toFixed(1)} mm</span></div>
                    </div>
                    <div class="ground-pass-badge ${g.can_pass ? 'yes' : 'no'}">
                        ${g.can_pass ? '✓ 可通过' : '✗ 不可通过'}
                    </div>
                </div>
            `;
        }).join('');
    }

    generateMockAnalysis() {
        const maxStress = 18 + Math.random() * 15;
        return {
            safety_factor: (this.currentTower.material_strength / maxStress).toFixed(2),
            critical_stress: this.currentTower.material_strength,
            max_stress: maxStress.toFixed(2),
            max_stress_layer: Math.ceil(this.currentTower.total_layers * 0.8),
            max_tilt: (0.8 + Math.random() * 1.8).toFixed(2),
            max_tilt_layer: this.currentTower.total_layers,
            wind_resistance_limit: this.currentTower.design_wind_speed * 1.2,
            current_wind_factor: (Math.random() * 0.5 + 0.2).toFixed(2),
            ground_capacity_ratio: (Math.random() * 0.4 + 0.3).toFixed(2),
            is_stable: 1,
            stability_margin: (30 + Math.random() * 40).toFixed(1),
            second_order_effect: (1 + Math.random() * 0.3).toFixed(2),
            natural_frequency: (2 + Math.random()).toFixed(2),
            damping_ratio: (0.03 + Math.random() * 0.02).toFixed(3),
        };
    }

    generateMockGround(ws, tl) {
        return Object.keys(SOIL_NAMES).map(key => {
            const caps = { sand: 180, clay: 120, silt: 90, rock: 800, loam: 200 };
            const c = caps[key];
            const base_p = this.currentTower.total_weight * 9.81 / (this.currentTower.base_width * this.currentTower.base_depth);
            const wind_effect = ws * ws * 0.01 * 5;
            const tilt_effect = tl * 4;
            const applied = base_p + wind_effect + tilt_effect;
            const sf = c / applied;
            const settlement = key === 'rock' ? 1 : (key === 'clay' ? 80 : 30) * (1 + tl * 0.1);
            const score = Math.max(0, Math.min(100,
                (sf >= 3 ? 100 : sf * 30) + (settlement < 30 ? 40 : (settlement < 100 ? 20 : 0))
            ));
            return {
                soil_type: key,
                bearing_capacity: c,
                applied_pressure: applied,
                safety_factor: sf,
                settlement,
                differential_settlement: settlement * 0.3,
                passability_score: score,
                can_pass: score >= 50 ? 1 : 0,
                risk_level: score >= 75 ? 1 : (score >= 30 ? 2 : 3),
            };
        });
    }

    generateMockSensorData(windSpeed, baseTilt) {
        const layers = [];
        for (let l = 1; l <= this.currentTower.total_layers; l++) {
            const h = l / this.currentTower.total_layers;
            const q = 0.5 * 1.225 * 1.3 * windSpeed * windSpeed;
            const base = 2 + h * 22;
            const ws = q / 1000 * (1 + h * 0.5) * 15;
            const sx = base + ws + (Math.random() - 0.5) * 3;
            const sy = base * 0.75 + ws * 0.6 + (Math.random() - 0.5) * 2;
            const sz = (this.currentTower.total_weight * 9.81 /
                       (this.currentTower.base_width * this.currentTower.base_depth)) * (1 + h * 0.2);
            const j2 = 0.5 * ((sx - sy) ** 2 + (sy - sz) ** 2 + (sz - sx) ** 2);
            const vm = Math.sqrt(3 * j2);
            layers.push({
                layer_id: l,
                layer_name: `第${l}层`,
                stress_x: sx, stress_y: sy, stress_z: sz, stress_von_mises: vm,
                tilt_x: baseTilt * (0.4 + h) * 1.2 + (Math.random() - 0.5) * 0.1,
                tilt_y: baseTilt * (0.25 + h * 0.6) + (Math.random() - 0.5) * 0.08,
                tilt_total: 0,
                wind_load_x: q * (1 + h * 0.4),
                wind_load_y: q * 0.35 * (1 + h * 0.2),
            });
        }
        layers.forEach(l => {
            l.tilt_total = Math.sqrt(l.tilt_x * l.tilt_x + l.tilt_y * l.tilt_y);
        });

        const sensor = layers.map(l => ({
            layer_id: l.layer_id,
            layer_name: l.layer_name,
            stress_x: l.stress_x,
            stress_y: l.stress_y,
            stress_z: l.stress_z,
            stress_von_mises: l.stress_von_mises,
            tilt_x: l.tilt_x,
            tilt_y: l.tilt_y,
            tilt_total: l.tilt_total,
            wind_load_x: l.wind_load_x,
            wind_load_y: l.wind_load_y,
            wind_speed_mps: windSpeed,
            ground_pressure: 0,
            ground_settlement: 0,
            soil_type: 'loam',
            temperature_c: 22,
            humidity_pct: 65,
            vibration_freq: 2.5,
            vibration_amp: 0.3,
        }));

        return { layers, sensor };
    }

    async runSimulation() {
        const ws = 5 + Math.random() * 35;
        const tilt = Math.random() * 3;
        const { layers } = this.generateMockSensorData(ws, tilt);

        const batch = {
            tower_id: this.currentTower.tower_id,
            tower_name: this.currentTower.tower_name,
            timestamp: new Date().toISOString(),
            layers,
            environment: {
                wind_speed_mps: ws,
                wind_direction_deg: 0,
                ground_pressure_kpa: this.currentTower.total_weight * 9.81 / (this.currentTower.base_width * this.currentTower.base_depth) * 1.1,
                temperature_c: 15 + Math.random() * 15,
                humidity_pct: 40 + Math.random() * 40,
            }
        };

        let analysis = null, alerts = [], stresses = null;

        try {
            const resp = await fetch(`${API_BASE}/api/towers/${this.currentTower.tower_id}/sensor`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(batch)
            });
            const json = await resp.json();
            if (json.code === 200 && json.data) {
                analysis = json.data.analysis;
                alerts = json.data.alerts || [];
            }
        } catch (e) {}

        try {
            const resp2 = await fetch(
                `${API_BASE}/api/towers/${this.currentTower.tower_id}/analysis/full?wind_speed=${ws.toFixed(1)}&tilt_deg=${tilt.toFixed(2)}`
            );
            const json2 = await resp2.json();
            if (json2.code === 200) {
                stresses = json2.data;
                if (!analysis) analysis = json2.data.structure;
            }
        } catch (e) {}

        if (!analysis) analysis = this.generateMockAnalysis();

        const sensorData = layers.map(l => ({
            ...l,
            wind_speed_mps: ws,
            soil_type: batch.environment.soil_type || 'loam',
            ground_pressure: batch.environment.ground_pressure_kpa,
        }));

        this.updateFromData(sensorData, analysis, alerts, ws, batch.environment.soil_type || 'loam');

        if (stresses?.layer_stresses) {
            const st = stresses.layer_stresses.map(([layer, vm, _tx, _tt]) => ({ layer, stress: vm }));
            this.tower3D.updateLayerStresses(st, this.currentTower.material_strength);
            this.updateLayerStressLabels(st);
        } else {
            const st = layers.map(l => ({ layer: l.layer_id, stress: l.stress_von_mises }));
            this.tower3D.updateLayerStresses(st, this.currentTower.material_strength);
            this.updateLayerStressLabels(st);
        }
    }

    updateFromData(sensorData, analysis, alerts, windSpeed, soilType) {
        if (analysis) this.updateAnalysisPanel(analysis, soilType);

        if (alerts?.length) {
            alerts.forEach(a => this.addAlert(a));
        }

        if (windSpeed !== undefined) this.updateWindIndicator(windSpeed);

        if (sensorData?.length) {
            const stresses = sensorData.map(s => ({ layer: s.layer_id, stress: s.stress_von_mises }));
            this.tower3D.updateLayerStresses(stresses, this.currentTower.material_strength);
            this.updateLayerStressLabels(stresses);
            this.updateLayerChart(sensorData);

            const maxTilt = sensorData.reduce((m, s) => s.tilt_total > m.tilt_total ? s : m, sensorData[0]);
            this.updateTiltIndicator(maxTilt.tilt_x, maxTilt.tilt_y, maxTilt.tilt_total, analysis?.max_tilt_layer || maxTilt.layer_id);
            this.tower3D.updateTilt(maxTilt.tilt_x, maxTilt.tilt_y);

            const max = analysis?.max_stress ||
                sensorData.reduce((m, s) => Math.max(m, s.stress_von_mises), 0);
            this.addHistoryPoint(max, analysis?.critical_stress || this.currentTower.material_strength);
        }
    }

    updateAnalysisPanel(an, soilType = null) {
        const sf = parseFloat(an.safety_factor) || 0;
        const margin = parseFloat(an.stability_margin) || 0;
        const maxStress = parseFloat(an.max_stress) || 0;
        const critStress = parseFloat(an.critical_stress) || this.currentTower.material_strength;
        const wf = parseFloat(an.current_wind_factor) || 0;
        const wl = parseFloat(an.wind_resistance_limit) || 0;
        const gr = parseFloat(an.ground_capacity_ratio) || 0;
        const so = parseFloat(an.second_order_effect) || 1;
        const freq = parseFloat(an.natural_frequency) || 0;
        const isStable = an.is_stable === 1 || an.is_stable === true;

        this.setText('sfVal', sf.toFixed(2));
        this.setText('marginVal', margin > 0 ? `+${margin.toFixed(1)}%` : `${margin.toFixed(1)}%`);
        this.setText('maxStressVal', maxStress.toFixed(2));
        this.setText('criticalStressVal', critStress.toFixed(1));
        this.setText('windFactorVal', wf.toFixed(2));
        this.setText('windLimitVal', wl.toFixed(1));
        this.setText('groundRatioVal', (gr * 100).toFixed(1) + '%');
        this.setText('orderVal', so.toFixed(2));
        this.setText('freqVal', freq.toFixed(2));

        if (soilType) {
            const info = SOIL_NAMES[soilType];
            if (info) this.setText('soilTypeVal', `${info.icon} ${info.name}`);
        }

        this.setBarWidth('sfBar', Math.min(sf / 5, 1) * 100);
        this.setBarColor('sfBar', sf < 1.5 ? 'var(--danger)' : (sf < 2.5 ? 'var(--warning)' : 'var(--safe)'));
        this.setBarWidth('marginBar', Math.max(0, Math.min(margin + 100, 200)) / 2);
        this.setBarColor('marginBar', margin < 0 ? 'var(--danger)' : (margin < 20 ? 'var(--warning)' : 'var(--safe)'));
        this.setBarWidth('stressBar', Math.min(maxStress / critStress, 1) * 100);
        this.setBarColor('stressBar', maxStress / critStress > 0.9 ? 'var(--danger)' :
            (maxStress / critStress > 0.75 ? 'var(--warning)' : 'var(--safe)'));
        this.setBarWidth('windBar', Math.min(wf, 1) * 100);
        this.setBarColor('windBar', wf > 0.95 ? 'var(--danger)' : (wf > 0.8 ? 'var(--warning)' : 'var(--cyan)'));
        this.setBarWidth('groundBar', Math.min(gr, 1) * 100);
        this.setBarColor('groundBar', gr > 0.95 ? 'var(--danger)' : (gr > 0.8 ? 'var(--warning)' : 'var(--purple)'));

        const badge = document.getElementById('statusBadge');
        badge.classList.remove('stable', 'unstable');
        if (isStable) {
            badge.classList.add('stable');
            badge.textContent = `✓ 结构稳定 | 裕度 ${margin > 0 ? '+' : ''}${margin.toFixed(1)}%`;
        } else {
            badge.classList.add('unstable');
            badge.textContent = '⚠ 结构失稳警告！';
        }
    }

    updateTiltIndicator(tx, ty, total, maxLayer) {
        this.setText('tiltXVal', tx.toFixed(2) + '°');
        this.setText('tiltYVal', ty.toFixed(2) + '°');
        this.setText('tiltTotalVal', total.toFixed(2) + '°');
        this.setText('tiltMaxLayer', maxLayer ? `第${maxLayer}层` : '-');

        const ptr = document.getElementById('tiltPointer');
        const maxAngle = 7;
        const scale = 55 / maxAngle;
        const rad = Math.atan2(ty, tx) * 180 / Math.PI;
        const len = Math.min(total * scale, 60);
        ptr.style.transform = `translate(-50%, 0) rotate(${rad - 90}deg) translateY(-${len / 2}px)`;
        ptr.style.height = `${Math.max(len, 8)}px`;

        const tv = document.querySelector('.tilt-value');
        tv.style.color = total > 5 ? 'var(--danger)' : (total > 3 ? 'var(--warning)' : 'var(--accent-light)');
    }

    updateWindIndicator(ws) {
        this.setText('windSpeedVal', ws.toFixed(1));
        const arrow = document.getElementById('windArrow');
        arrow.style.transform = `rotate(${(ws * 12) % 360}deg) scale(${1 + Math.min(ws / 40, 0.8)})`;

        let level = '无';
        if (ws < 1) level = '无风';
        else if (ws < 5) level = '微风';
        else if (ws < 10) level = '轻风';
        else if (ws < 17) level = '和风';
        else if (ws < 25) level = '大风';
        else if (ws < 35) level = '强风';
        else if (ws < 45) level = '狂风';
        else level = '暴风';
        this.setText('windLevelLabel', level);
    }

    updateLayerChart(sensorData) {
        if (!this.layerChart) return;
        const mode = this.currentChartMode;

        if (mode === 'stress') {
            this.layerChart.data.datasets[0].data = sensorData.map(s => s.stress_von_mises);
            this.layerChart.data.datasets[0].backgroundColor = ctx => {
                const max = this.currentTower.material_strength;
                const v = ctx.parsed?.y || 0;
                const r = Math.min(v / max, 1);
                const hue = 120 - r * 120;
                return `hsl(${hue}, 70%, 50%)`;
            };
        } else if (mode === 'tilt') {
            this.layerChart.data.datasets[0].data = sensorData.map(s => s.tilt_total);
            this.layerChart.data.datasets[0].backgroundColor = ctx => {
                const v = ctx.parsed?.y || 0;
                const r = Math.min(v / 5, 1);
                const hue = 120 - r * 120;
                return `hsl(${hue}, 70%, 50%)`;
            };
        } else {
            this.layerChart.data.datasets[0].data = sensorData.map(s =>
                Math.sqrt(s.wind_load_x ** 2 + s.wind_load_y ** 2)
            );
            this.layerChart.data.datasets[0].backgroundColor = ctx => {
                const v = ctx.parsed?.y || 0;
                const r = Math.min(v / 500, 1);
                const hue = 200 - r * 140;
                return `hsl(${hue}, 70%, 50%)`;
            };
        }
        this.layerChart.data.labels = sensorData.map(s => s.layer_name);
        this.layerChart.update('active');
    }

    addHistoryPoint(stress, critical) {
        const now = new Date();
        const label = `${String(now.getHours()).padStart(2, '0')}:${String(now.getMinutes()).padStart(2, '0')}:${String(now.getSeconds()).padStart(2, '0')}`;
        this.stressHistory.push({ label, stress, critical });
        if (this.stressHistory.length > 30) this.stressHistory.shift();

        this.historyChart.data.labels = this.stressHistory.map(s => s.label);
        this.historyChart.data.datasets[0].data = this.stressHistory.map(s => s.stress);
        this.historyChart.data.datasets[1].data = this.stressHistory.map(() => critical);
        this.historyChart.update('none');
    }

    addAlert(alert) {
        const list = document.getElementById('alertList');
        const empty = list.querySelector('.empty-state');
        if (empty) empty.remove();

        const name = ALERT_NAMES[alert.alert_type] || alert.alert_type;
        const time = new Date(alert.timestamp).toLocaleTimeString('zh-CN');
        const levelCls = 'level-' + alert.alert_level;
        const levelText = ['', '预警', '告警', '危险'][alert.alert_level] || '';

        const item = document.createElement('div');
        item.className = `alert-item ${levelCls}`;
        item.innerHTML = `
            <div class="alert-header">
                <div class="alert-type">${name} · ${levelText}</div>
                <div class="alert-time">${time} · ${alert.layer_id ? '第' + alert.layer_id + '层' : '整体'}</div>
            </div>
            <div class="alert-desc">${alert.description}</div>
            <div class="alert-metric">
                <span>${alert.metric_name || ''}</span>
                <span class="metric">
                    ${(alert.metric_value ?? '-').toFixed(2)} / 阈值 ${(alert.threshold ?? '-').toFixed(2)}
                </span>
            </div>
        `;
        list.insertBefore(item, list.firstChild);

        const count = document.getElementById('alertCount');
        const current = parseInt(count.textContent || '0') + 1;
        count.textContent = current;
        count.classList.remove('zero');

        while (list.children.length > 50) {
            list.removeChild(list.lastChild);
        }
    }

    connectSSE() {
        try {
            this.analysisSSE = new EventSource(`${API_BASE}/api/stream/analysis`);
            this.analysisSSE.addEventListener('analysis', e => {
                try {
                    const data = JSON.parse(e.data);
                    this.updateAnalysisPanel(data);
                } catch {}
            });

            this.sensorSSE = new EventSource(`${API_BASE}/api/stream/sensor`);
            this.sensorSSE.addEventListener('sensor', e => {
                try {
                    const data = JSON.parse(e.data);
                    if (Array.isArray(data) && data.length) {
                        const tid = data[0].tower_id;
                        if (tid === this.currentTower.tower_id) {
                            const analysis = null;
                            const alerts = [];
                            const ws = data[0].wind_speed_mps;
                            const soil = data[0].soil_type;
                            this.updateFromData(data, analysis, alerts, ws, soil);
                        }
                    }
                } catch {}
            });

            this.alertSSE = new EventSource(`${API_BASE}/api/stream/alerts`);
            this.alertSSE.addEventListener('alert', e => {
                try {
                    const a = JSON.parse(e.data);
                    if (a.tower_id === this.currentTower.tower_id) this.addAlert(a);
                } catch {}
            });

            [this.analysisSSE, this.sensorSSE, this.alertSSE].forEach(s => {
                if (s) s.onerror = () => {};
            });
        } catch (e) {
            console.warn('SSE 不可用，将使用轮询');
        }
    }

    closeSSE() {
        [this.analysisSSE, this.sensorSSE, this.alertSSE].forEach(s => s?.close());
        this.analysisSSE = this.sensorSSE = this.alertSSE = null;
    }

    setText(id, val) {
        const el = document.getElementById(id);
        if (el) el.textContent = val;
    }
    setBarWidth(id, pct) {
        const el = document.getElementById(id);
        if (el) el.style.width = pct + '%';
    }
    setBarColor(id, color) {
        const el = document.getElementById(id);
        if (el) el.style.background = color;
    }
    setConnectionStatus(cls, text) {
        const s = document.getElementById('connectionStatus');
        if (!s) return;
        s.className = 'status-indicator ' + cls;
        const t = s.querySelector('.status-text');
        if (t) t.textContent = text;
    }

    async loadDynastyComparison() {
        let data = null;
        try {
            const resp = await fetch(`${API_BASE}/api/comparison/dynasty?wind_speed=15`);
            const json = await resp.json();
            if (json.code === 200) data = json.data;
        } catch (e) {}

        if (!data) data = this._generateMockDynastyComparison();
        this._renderDynastyComparison(data);
    }

    _generateMockDynastyComparison() {
        const towers = Object.values(DEFAULT_TOWERS).filter(t => t.tower_id <= 4);
        return towers.map(t => ({
            tower_id: t.tower_id,
            tower_name: t.tower_name,
            safety_factor: (t.material_strength / (10 + t.total_height * 0.8)).toFixed(2),
            max_stress: (10 + t.total_height * 0.8).toFixed(1),
            wind_resistance: t.design_wind_speed,
            weight_efficiency: (t.design_load / t.total_weight).toFixed(1),
        }));
    }

    _renderDynastyComparison(data) {
        const grid = document.getElementById('dynastyGrid');
        if (!grid) return;

        const metrics = ['safety_factor', 'max_stress', 'wind_resistance', 'weight_efficiency'];
        const labels = { safety_factor: '安全系数', max_stress: '最大应力(MPa)', wind_resistance: '抗风(m/s)', weight_efficiency: '荷载效率(kN/t)' };
        const bestBy = { safety_factor: 'max', max_stress: 'min', wind_resistance: 'max', weight_efficiency: 'max' };

        const bestVals = {};
        for (const m of metrics) {
            const vals = data.map(d => parseFloat(d[m]));
            bestVals[m] = bestBy[m] === 'max' ? Math.max(...vals) : Math.min(...vals);
        }

        grid.innerHTML = data.map(d => `
            <div class="dynasty-card">
                <div class="dynasty-card-title">${d.tower_name}</div>
                ${metrics.map(m => {
                    const v = parseFloat(d[m]);
                    const isBest = v === bestVals[m];
                    return `<div class="dynasty-metric ${isBest ? 'best' : ''}">
                        <span class="dynasty-metric-label">${labels[m]}</span>
                        <span class="dynasty-metric-value">${d[m]}</span>
                    </div>`;
                }).join('')}
            </div>
        `).join('');
    }

    async loadCrossEraComparison() {
        let data = null;
        try {
            const resp = await fetch(`${API_BASE}/api/comparison/cross-era?wind_speed=15`);
            const json = await resp.json();
            if (json.code === 200) data = json.data;
        } catch (e) {}

        if (!data) data = this._generateMockCrossEraComparison();
        this._renderCrossEraComparison(data);
    }

    _generateMockCrossEraComparison() {
        const ancient = DEFAULT_TOWERS[1];
        const modern = DEFAULT_TOWERS[5];
        return {
            ancient: {
                name: ancient.tower_name,
                material: ancient.material,
                height: ancient.total_height,
                weight: ancient.total_weight,
                strength: ancient.material_strength,
                elastic_modulus: ancient.elastic_modulus,
                design_load: ancient.design_load,
                wind_speed: ancient.design_wind_speed,
            },
            modern: {
                name: modern.tower_name,
                material: modern.material,
                height: modern.total_height,
                weight: modern.total_weight,
                strength: modern.material_strength,
                elastic_modulus: modern.elastic_modulus,
                design_load: modern.design_load,
                wind_speed: modern.design_wind_speed,
            },
        };
    }

    _renderCrossEraComparison(data) {
        const grid = document.getElementById('crossEraGrid');
        if (!grid) return;

        const metrics = [
            { key: 'height', label: '高度(m)' },
            { key: 'weight', label: '重量(t)' },
            { key: 'strength', label: '材料强度(MPa)' },
            { key: 'elastic_modulus', label: '弹性模量(MPa)' },
            { key: 'design_load', label: '设计荷载(kN)' },
            { key: 'wind_speed', label: '抗风(m/s)' },
        ];

        const maxVals = {};
        for (const m of metrics) {
            maxVals[m.key] = Math.max(data.ancient[m.key], data.modern[m.key]) || 1;
        }

        grid.innerHTML = `
            <div class="era-column ancient">
                <div class="era-column-title">${data.ancient.name}</div>
                <div class="era-column-sub">${data.ancient.material}</div>
                ${metrics.map(m => {
                    const pct = (data.ancient[m.key] / maxVals[m.key]) * 100;
                    const ratio = data.ancient[m.key] / (data.modern[m.key] || 1);
                    return `<div class="era-metric-row">
                        <span class="era-metric-label">${m.label}</span>
                        <div class="era-metric-bar">
                            <div class="era-bar-fill ancient" style="width:${pct}%"></div>
                        </div>
                        <span class="era-metric-value">${data.ancient[m.key]}</span>
                        <span class="era-ratio">${ratio.toFixed(1)}x</span>
                    </div>`;
                }).join('')}
            </div>
            <div class="era-column modern">
                <div class="era-column-title">${data.modern.name}</div>
                <div class="era-column-sub">${data.modern.material}</div>
                ${metrics.map(m => {
                    const pct = (data.modern[m.key] / maxVals[m.key]) * 100;
                    return `<div class="era-metric-row">
                        <span class="era-metric-label">${m.label}</span>
                        <div class="era-metric-bar">
                            <div class="era-bar-fill modern" style="width:${pct}%"></div>
                        </div>
                        <span class="era-metric-value">${data.modern[m.key]}</span>
                    </div>`;
                }).join('')}
            </div>
        `;
    }

    async loadMoatAnalysis() {
        const dist = parseFloat(document.getElementById('moatDistInput')?.value || 3);
        const depth = parseFloat(document.getElementById('moatDepthInput')?.value || 4);
        const water = parseFloat(document.getElementById('waterTableInput')?.value || 1.5);
        const soilType = 'loam';

        let data = null;
        try {
            const resp = await fetch(
                `${API_BASE}/api/towers/${this.currentTower.tower_id}/moat?moat_distance=${dist}&moat_depth=${depth}&water_table_depth=${water}&soil_type=${soilType}`
            );
            const json = await resp.json();
            if (json.code === 200) data = json.data;
        } catch (e) {}

        if (!data) data = this._generateMockMoatAnalysis(dist, depth, water);
        this._renderMoatAnalysis(data);
    }

    _generateMockMoatAnalysis(dist, depth, water) {
        const riskLevel = dist < 2 ? 5 : (dist < 4 ? 4 : (dist < 6 ? 3 : (dist < 10 ? 2 : 1)));
        return {
            moat_distance: dist,
            moat_depth: depth,
            water_table_depth: water,
            soil_type: 'loam',
            foundation_stability: riskLevel <= 2 ? 'stable' : (riskLevel <= 3 ? 'warning' : 'danger'),
            settlement_risk: riskLevel <= 2 ? 'low' : (riskLevel <= 3 ? 'medium' : 'high'),
            lateral_pressure_ratio: (0.3 + (1 - dist / 20) * 0.5).toFixed(2),
            safety_factor: (3.0 - riskLevel * 0.4).toFixed(2),
            risk_level: riskLevel,
            max_settlement: (depth * 2 + water * 3).toFixed(1),
            recommendation: riskLevel >= 4 ? '建议远离护城河至少5m' : (riskLevel >= 3 ? '需加固地基' : '地基条件可接受'),
        };
    }

    _renderMoatAnalysis(data) {
        const container = document.getElementById('moatResult');
        if (!container) return;

        const riskLabels = ['', '极低', '低', '中等', '高', '极高'];
        const riskLevel = data.risk_level || 1;

        container.innerHTML = `
            <div class="moat-summary">
                <div class="moat-risk-badge risk-${riskLevel}">
                    风险等级: ${riskLabels[riskLevel] || '未知'}
                </div>
                <div class="moat-stability-tag ${data.foundation_stability}">${data.foundation_stability === 'stable' ? '✓ 地基稳定' : (data.foundation_stability === 'warning' ? '⚠ 地基注意' : '✗ 地基危险')}</div>
            </div>
            <div class="moat-details">
                <div class="moat-detail-row"><span>安全系数</span><span>${data.safety_factor}</span></div>
                <div class="moat-detail-row"><span>侧压力比</span><span>${data.lateral_pressure_ratio}</span></div>
                <div class="moat-detail-row"><span>最大沉降</span><span>${data.max_settlement} mm</span></div>
                <div class="moat-detail-row"><span>沉降风险</span><span>${data.settlement_risk}</span></div>
                <div class="moat-detail-row"><span>建议</span><span>${data.recommendation}</span></div>
            </div>
        `;
    }

    async initClimbingExperience() {
        let data = null;
        try {
            const resp = await fetch(`${API_BASE}/api/towers/${this.currentTower.tower_id}/climbing`);
            const json = await resp.json();
            if (json.code === 200) data = json.data;
        } catch (e) {}

        if (!data) data = this._generateMockClimbingData();
        this._climbingViewpoints = data.viewpoints || data;
        this.renderViewpointButtons(this._climbingViewpoints);
    }

    _generateMockClimbingData() {
        const viewpoints = [];
        const L = this.currentTower.total_layers;
        const H = this.currentTower.total_height;
        const layerH = H / L;
        for (let i = 1; i <= L; i++) {
            const y = i * layerH;
            viewpoints.push({
                layer: i,
                name: `第${i}层`,
                position: { x: 0, y: y, z: 3 },
                look_at: { x: 0, y: y + layerH / 2, z: 0 },
                description: `站在${this.currentTower.tower_name}第${i}层，高度${y.toFixed(1)}m`,
            });
        }
        return { viewpoints };
    }

    renderViewpointButtons(viewpoints) {
        const container = document.getElementById('climbingViewpoints');
        if (!container) return;
        container.innerHTML = '';
        for (const vp of viewpoints) {
            const btn = document.createElement('button');
            btn.className = 'viewpoint-btn';
            btn.textContent = vp.name;
            btn.addEventListener('click', () => {
                this.enterClimbingMode(vp);
                const info = document.getElementById('climbingInfo');
                if (info) info.textContent = vp.name;
                const desc = document.getElementById('climbingDesc');
                if (desc) desc.textContent = vp.description || '';
            });
            container.appendChild(btn);
        }
    }

    enterClimbingMode(viewpoint) {
        this.climbingMode = true;
        const viewer = this.tower3D?.viewer;
        if (!viewer) return;

        if (!this.savedCameraState) {
            this.savedCameraState = {
                position: viewer.camera.position.clone(),
                target: viewer.controls.target.clone(),
                fov: viewer.camera.fov,
            };
        }

        const pos = viewpoint.position;
        const look = viewpoint.look_at;
        const targetFov = viewpoint.recommended_fov_deg || 65;
        const duration = viewpoint.transition_duration_ms || 1000;

        const startPos = viewer.camera.position.clone();
        const startFov = viewer.camera.fov;
        const startTime = performance.now();

        const animateTransition = (now) => {
            const elapsed = now - startTime;
            const t = Math.min(elapsed / duration, 1.0);
            const ease = t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2;

            viewer.camera.position.lerpVectors(startPos, new THREE.Vector3(pos.x, pos.y, pos.z), ease);
            viewer.camera.fov = startFov + (targetFov - startFov) * ease;
            viewer.camera.lookAt(look.x, look.y, look.z);
            viewer.camera.updateProjectionMatrix();

            if (t < 1.0) {
                requestAnimationFrame(animateTransition);
            } else {
                viewer.controls.enabled = false;
            }
        };

        if (typeof THREE !== 'undefined') {
            requestAnimationFrame(animateTransition);
        } else {
            viewer.camera.position.set(pos.x, pos.y, pos.z);
            viewer.camera.lookAt(look.x, look.y, look.z);
            viewer.controls.enabled = false;
        }

        const overlay = document.getElementById('climbingOverlay');
        if (overlay) overlay.style.display = 'flex';

        const heightWarning = document.getElementById('climbingHeightWarning');
        if (heightWarning && viewpoint.acrophobia_risk_level >= 3) {
            heightWarning.textContent = `⚠ 当前高度 ${viewpoint.height_above_ground_m?.toFixed(1) || ''}m，恐高风险等级 ${viewpoint.acrophobia_risk_level}/5`;
            heightWarning.style.display = 'block';
        } else if (heightWarning) {
            heightWarning.style.display = 'none';
        }
    }

    exitClimbingMode() {
        this.climbingMode = false;
        const viewer = this.tower3D?.viewer;
        if (viewer && this.savedCameraState) {
            viewer.camera.position.copy(this.savedCameraState.position);
            viewer.camera.fov = this.savedCameraState.fov || 75;
            viewer.camera.updateProjectionMatrix();
            viewer.controls.target.copy(this.savedCameraState.target);
            viewer.controls.enabled = true;
            viewer.controls.update();
            this.savedCameraState = null;
        }

        const overlay = document.getElementById('climbingOverlay');
        if (overlay) overlay.style.display = 'none';

        const heightWarning = document.getElementById('climbingHeightWarning');
        if (heightWarning) heightWarning.style.display = 'none';
    }
}
