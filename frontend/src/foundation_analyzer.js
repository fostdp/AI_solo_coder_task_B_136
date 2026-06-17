import { API_BASE } from './config.js';

const DEFAULT_PARAMS = {
    moat_distance: 3.0,
    moat_depth: 4.0,
    water_table_depth: 1.5,
    wind_speed: 15.0,
    tilt_deg: 0.5,
    soil_type: 'loam',
};

const RISK_LABELS = ['', '极低', '低', '中等', '高', '极高'];

export class FoundationAnalyzer {
    constructor(getCurrentTower) {
        this.getCurrentTower = getCurrentTower;
        this.currentResult = null;
    }

    async analyze(params = {}) {
        const p = { ...DEFAULT_PARAMS, ...params };
        let data = null;

        const towerId = p.tower_id || (this.getCurrentTower?.()?.tower_id) || 1;
        try {
            const qs = new URLSearchParams({
                moat_distance: p.moat_distance,
                moat_depth: p.moat_depth,
                water_table_depth: p.water_table_depth,
                wind_speed: p.wind_speed,
                tilt_deg: p.tilt_deg,
                soil_type: p.soil_type,
            }).toString();
            const resp = await fetch(`${API_BASE}/api/towers/${towerId}/moat?${qs}`);
            const json = await resp.json();
            if (json.code === 200) data = json.data;
        } catch (e) {}

        if (!data) data = this._generateMock(p);
        this.currentResult = data;
        return data;
    }

    _generateMock(p) {
        const dist = p.moat_distance;
        const depth = p.moat_depth;
        const water = p.water_table_depth;
        const riskLevel = dist < 2 ? 5 : dist < 4 ? 4 : dist < 6 ? 3 : dist < 10 ? 2 : 1;

        const saturation_factor = Math.max(0, 1 - water / Math.max(depth, 0.1));
        const pore_pressure_ratio = 0.5 * saturation_factor * saturation_factor;
        const water_soil_coupling_factor = 1 - pore_pressure_ratio * (1 - 0.80);

        return {
            tower_id: p.tower_id || 1,
            soil_type: p.soil_type,
            moat_distance: dist,
            moat_depth: depth,
            water_table_depth: water,
            overall_safety_factor: (3.0 - riskLevel * 0.4),
            bearing_capacity_reduction: (0.5 + 0.5 * (water / depth)) * water_soil_coupling_factor,
            settlement_increase_pct: (50 + saturation_factor * 20) * (1 + pore_pressure_ratio * 0.5),
            lateral_displacement_pct: (30 + saturation_factor * 15) * (1 + pore_pressure_ratio * 2),
            slope_stability_factor: (1.8 - riskLevel * 0.25),
            foundation_stability: riskLevel <= 2 ? 'stable' : riskLevel <= 3 ? 'warning' : 'danger',
            settlement_risk: riskLevel <= 2 ? 'low' : riskLevel <= 3 ? 'medium' : 'high',
            lateral_pressure_ratio: (0.3 + (1 - dist / 20) * 0.5).toFixed(2),
            safety_factor: (3.0 - riskLevel * 0.4).toFixed(2),
            risk_level: riskLevel,
            max_settlement: (depth * 2 + water * 3).toFixed(1),
            pore_pressure_ratio: pore_pressure_ratio,
            seepage_force_kn: saturation_factor * 80 * 5,
            water_soil_coupling_factor,
            recommendation: riskLevel >= 4
                ? '建议远离护城河至少5m，设置排水系统降低孔隙水压力'
                : riskLevel >= 3
                ? '需加固地基、设置护壁桩，建议地基排水'
                : '地基条件可接受，常规监测即可',
            recommendations: [
                riskLevel >= 3 ? '远离护城河边缘' : '常规地基检查',
                pore_pressure_ratio > 0.3 ? '设置排水系统降低孔隙水压力' : '无特殊排水需求',
                riskLevel >= 4 ? '增加护壁桩与锚杆' : '基础形式满足要求',
            ],
        };
    }

    summaryText() {
        const d = this.currentResult;
        if (!d) return '';
        const risk = RISK_LABELS[d.risk_level] || '未知';
        return `风险等级 ${risk}，安全系数 ${d.safety_factor}，耦合效应 ${(d.water_soil_coupling_factor * 100).toFixed(0)}%`;
    }

    render(container, data = this.currentResult) {
        if (!container || !data) return;
        const riskLevel = data.risk_level || 1;
        const riskLabel = RISK_LABELS[riskLevel] || '未知';
        const stabilityMap = {
            stable: '✓ 地基稳定',
            warning: '⚠ 地基注意',
            danger: '✗ 地基危险',
        };

        container.innerHTML = `
            <div class="moat-summary">
                <div class="moat-risk-badge risk-${riskLevel}">风险等级: ${riskLabel}</div>
                <div class="moat-stability-tag ${data.foundation_stability}">${stabilityMap[data.foundation_stability] || data.foundation_stability}</div>
            </div>
            <div class="moat-details">
                <div class="moat-detail-row"><span>安全系数</span><span>${data.safety_factor}</span></div>
                <div class="moat-detail-row"><span>孔隙水压比</span><span>${(data.pore_pressure_ratio || 0).toFixed(3)}</span></div>
                <div class="moat-detail-row"><span>水土耦合因子</span><span>${(data.water_soil_coupling_factor || 0).toFixed(3)}</span></div>
                <div class="moat-detail-row"><span>渗流力</span><span>${(data.seepage_force_kn || 0).toFixed(1)} kN</span></div>
                <div class="moat-detail-row"><span>侧压力比</span><span>${data.lateral_pressure_ratio}</span></div>
                <div class="moat-detail-row"><span>最大沉降</span><span>${data.max_settlement} mm</span></div>
                <div class="moat-detail-row"><span>沉降风险</span><span>${data.settlement_risk}</span></div>
                <div class="moat-detail-row"><span>建议</span><span>${data.recommendation}</span></div>
            </div>
        `;

        const recBox = document.getElementById('moatRecommendations');
        if (recBox && data.recommendations?.length) {
            recBox.innerHTML = data.recommendations
                .map((r, i) => `<li class="moat-rec-item">${i + 1}. ${r}</li>`)
                .join('');
        }
    }
}

export const FoundationAnalyzerUI = {
    async init(containerId, analyzer, params = {}) {
        const container = document.getElementById(containerId);
        if (!container) return;
        const result = await analyzer.analyze(params);
        analyzer.render(container, result);
        return result;
    },

    bindInputs(analyzer, inputs) {
        const { distInputId, depthInputId, waterInputId, btnId, containerId } = inputs;
        document.getElementById(btnId)?.addEventListener('click', async () => {
            const dist = parseFloat(document.getElementById(distInputId)?.value || 3);
            const depth = parseFloat(document.getElementById(depthInputId)?.value || 4);
            const water = parseFloat(document.getElementById(waterInputId)?.value || 1.5);
            await FoundationAnalyzerUI.init(containerId, analyzer, {
                moat_distance: dist,
                moat_depth: depth,
                water_table_depth: water,
            });
        });
    },
};
