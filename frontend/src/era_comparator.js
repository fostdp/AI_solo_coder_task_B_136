import { API_BASE } from './config.js';

export class EraComparator {
    constructor(getCurrentTower) {
        this.getCurrentTower = getCurrentTower;
        this.currentData = null;
    }

    getMetricDefs() {
        return [
            { key: 'material_strength', label: '材料强度(MPa)' },
            { key: 'elastic_modulus', label: '弹性模量(MPa)' },
            { key: 'safety_factor', label: '安全系数' },
            { key: 'wind_resistance', label: '抗风(m/s)' },
            { key: 'natural_frequency', label: '自振频率(Hz)' },
            { key: 'weight_per_height', label: '单位高度重(t/m)' },
            { key: 'load_efficiency', label: '荷载效率' },
        ];
    }

    async load(windSpeed = 15, tiltDeg = 0.5) {
        let data = null;
        try {
            const resp = await fetch(
                `${API_BASE}/api/comparison/cross-era?wind_speed=${windSpeed}&tilt_deg=${tiltDeg}`
            );
            const json = await resp.json();
            if (json.code === 200) data = json.data;
        } catch (e) {}

        if (!data) data = this._generateMockData();
        this.currentData = data;
        return data;
    }

    _generateMockData() {
        const ancient = DEFAULT_TOWERS[1];
        const modern = DEFAULT_TOWERS[5];
        return {
            ancient: {
                tower_id: ancient.tower_id,
                tower_name: ancient.tower_name,
                era: '明朝',
                material: ancient.material,
                elastic_modulus: ancient.elastic_modulus,
                material_strength: ancient.material_strength,
                safety_factor: (ancient.material_strength / (10 + ancient.total_height * 0.8)).toFixed(2),
                wind_resistance: ancient.design_wind_speed,
                natural_frequency: (5 / (ancient.total_height * 0.5)).toFixed(2),
                weight_per_height: (ancient.total_weight / ancient.total_height).toFixed(2),
                load_efficiency: (ancient.design_load / (ancient.total_weight * 9.81)).toFixed(2),
            },
            modern: {
                tower_id: modern.tower_id,
                tower_name: modern.tower_name,
                era: '现代',
                material: modern.material,
                elastic_modulus: modern.elastic_modulus,
                material_strength: modern.material_strength,
                safety_factor: (modern.material_strength / (10 + modern.total_height * 0.8)).toFixed(2),
                wind_resistance: modern.design_wind_speed,
                natural_frequency: (5 / (modern.total_height * 0.5)).toFixed(2),
                weight_per_height: (modern.total_weight / modern.total_height).toFixed(2),
                load_efficiency: (modern.design_load / (modern.total_weight * 9.81)).toFixed(2),
            },
            ratios: {
                elastic_modulus_ratio: (modern.elastic_modulus / ancient.elastic_modulus).toFixed(2),
                strength_ratio: (modern.material_strength / ancient.material_strength).toFixed(2),
            },
            analysis: '现代Q345B钢材力学性能全面超越古代木结构，承载力和稳定性显著提升。',
        };
    }

    render(container, data = this.currentData) {
        if (!container) return;
        const metrics = this.getMetricDefs();

        const maxVals = {};
        for (const m of metrics) {
            maxVals[m.key] = Math.max(
                parseFloat(data.ancient[m.key]) || 1,
                parseFloat(data.modern[m.key]) || 1,
            );
        }

        const renderCol = (side, cls) => `
            <div class="era-column ${cls}">
                <div class="era-column-title">${data[side].tower_name}</div>
                <div class="era-column-sub">${data[side].era} · ${data[side].material}</div>
                ${metrics.map(m => {
                    const val = parseFloat(data[side][m.key]) || 0;
                    const pct = (val / maxVals[m.key]) * 100;
                    const ratioStr = side === 'ancient'
                        ? `<span class="era-ratio">${(val / (parseFloat(data.modern[m.key]) || 1)).toFixed(2)}x</span>`
                        : '';
                    return `<div class="era-metric-row">
                        <span class="era-metric-label">${m.label}</span>
                        <div class="era-metric-bar">
                            <div class="era-bar-fill ${cls}" style="width:${pct}%"></div>
                        </div>
                        <span class="era-metric-value">${data[side][m.key]}</span>
                        ${ratioStr}
                    </div>`;
                }).join('')}
            </div>
        `;

        container.innerHTML = renderCol('ancient', 'ancient') + renderCol('modern', 'modern');

        if (data.analysis) {
            const analysisBox = document.getElementById('crossEraAnalysis');
            if (analysisBox) analysisBox.textContent = data.analysis;
        }
    }
}

export const EraComparatorUI = {
    async init(containerId, comparator) {
        const container = document.getElementById(containerId);
        if (!container) return;
        const data = await comparator.load();
        comparator.render(container, data);
        return data;
    },
};
