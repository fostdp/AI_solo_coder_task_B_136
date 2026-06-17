import { API_BASE } from './config.js';

export class StructureComparator {
    constructor(getCurrentTower) {
        this.getCurrentTower = getCurrentTower;
        this.currentData = null;
    }

    getMetrics() {
        return [
            { key: 'safety_factor', label: '安全系数', best: 'max' },
            { key: 'wind_resistance_limit', label: '抗风(m/s)', best: 'max' },
            { key: 'natural_frequency', label: '自振频率(Hz)', best: 'max' },
            { key: 'overturning_ratio', label: '抗倾覆比', best: 'max' },
            { key: 'weight_efficiency', label: '荷载效率', best: 'max' },
            { key: 'height_to_base_ratio', label: '高宽比', best: 'max' },
        ];
    }

    async load(windSpeed = 15, tiltDeg = 0.5) {
        let data = null;
        try {
            const resp = await fetch(
                `${API_BASE}/api/comparison/dynasty?wind_speed=${windSpeed}&tilt_deg=${tiltDeg}`
            );
            const json = await resp.json();
            if (json.code === 200) data = json.data;
        } catch (e) {}

        if (!data) data = this._generateMockData();
        this.currentData = data;
        return data;
    }

    _generateMockData() {
        const towers = Object.values(DEFAULT_TOWERS).filter(t => t.tower_id <= 4);
        return towers.map(t => ({
            tower_id: t.tower_id,
            tower_name: t.tower_name,
            dynasty: this._dynastyFor(t.tower_id),
            category: 'ancient_wooden',
            safety_factor: (t.material_strength / (10 + t.total_height * 0.8)).toFixed(2),
            wind_resistance_limit: t.design_wind_speed,
            natural_frequency: (5 / (t.total_height * 0.5)).toFixed(2),
            overturning_ratio: (t.base_width / t.total_height * 8).toFixed(2),
            weight_efficiency: (t.design_load / (t.total_weight * 9.81)).toFixed(2),
            height_to_base_ratio: (t.total_height / t.base_width).toFixed(2),
        }));
    }

    _dynastyFor(towerId) {
        return { 1: '明朝(景泰)', 2: '明朝(景泰)', 3: '明朝(洪武)', 4: '三国(魏)' }[towerId] || '未知';
    }

    bestValues(dataRows) {
        const best = {};
        for (const m of this.getMetrics()) {
            const vals = dataRows.map(d => parseFloat(d[m.key]));
            best[m.key] = m.best === 'max' ? Math.max(...vals) : Math.min(...vals);
        }
        return best;
    }

    render(container, data = this.currentData) {
        if (!container) return;
        const rows = data.towers || data;
        const best = this.bestValues(rows);
        const metrics = this.getMetrics();

        container.innerHTML = rows.map(d => `
            <div class="dynasty-card" data-tower-id="${d.tower_id}">
                <div class="dynasty-card-title">${d.tower_name}</div>
                <div class="dynasty-card-sub">${d.dynasty}</div>
                ${metrics.map(m => {
                    const v = parseFloat(d[m.key]);
                    const isBest = v === best[m.key];
                    return `<div class="dynasty-metric ${isBest ? 'best' : ''}">
                        <span class="dynasty-metric-label">${m.label}</span>
                        <span class="dynasty-metric-value">${d[m.key]}</span>
                    </div>`;
                }).join('')}
            </div>
        `).join('');
    }
}

export const StructureComparatorUI = {
    async init(containerId, comparator) {
        const container = document.getElementById(containerId);
        if (!container) return;
        const data = await comparator.load();
        comparator.render(container, data);
        return data;
    },
};
