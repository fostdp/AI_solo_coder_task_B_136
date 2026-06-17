import { Tower3DViewer } from './tower3d.js';

export class SiegeTower3D {
    constructor(canvas, tower) {
        this.canvas = canvas;
        this.tower = tower;
        this.viewer = null;
        this.currentStresses = [];
    }

    init() {
        this.viewer = new Tower3DViewer(this.canvas, this.tower);
        this.viewer.legendMaxStress = this.tower.material_strength;

        const dummyStresses = [];
        for (let i = 1; i <= this.tower.total_layers; i++) {
            dummyStresses.push({
                layer: i,
                stress: 10 + i * 4,
            });
        }
        this.updateLayerStresses(dummyStresses, this.tower.material_strength);
        return dummyStresses;
    }

    updateTower(tower) {
        this.tower = tower;
        if (this.viewer) {
            this.viewer.dispose();
        }
        return this.init();
    }

    setCameraView(view) {
        if (this.viewer) {
            this.viewer.setCameraView(view);
        }
    }

    setStressView(enabled) {
        if (this.viewer) {
            this.viewer.setStressView(enabled);
        }
    }

    setCutMode(mode) {
        if (this.viewer) {
            this.viewer.setCutMode(mode);
        }
    }

    applyExplosion(value) {
        if (this.viewer) {
            this.viewer.applyExplosion(parseFloat(value));
        }
    }

    setLayerVisible(layer, visible) {
        if (this.viewer) {
            this.viewer.setLayerVisible(layer, visible);
        }
    }

    setLayerOpacity(layer, opacity) {
        if (this.viewer) {
            this.viewer.setLayerOpacity(layer, parseFloat(opacity));
        }
    }

    updateLayerStresses(stresses, maxStress) {
        this.currentStresses = stresses;
        if (this.viewer) {
            this.viewer.updateLayerStresses(stresses, maxStress);
        }
    }

    updateTilt(tx, ty) {
        if (this.viewer) {
            this.viewer.updateTilt(tx, ty);
        }
    }

    getLayerStresses() {
        return this.currentStresses;
    }

    dispose() {
        if (this.viewer) {
            this.viewer.dispose();
            this.viewer = null;
        }
    }
}
