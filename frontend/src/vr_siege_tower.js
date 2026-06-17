import { API_BASE } from './config.js';

const DEFAULT_FOV = 75;
const DEFAULT_DURATION = 800;

export class VrSiegeTower {
    constructor(getCurrentTower, getViewer) {
        this.getCurrentTower = getCurrentTower;
        this.getViewer = getViewer;
        this.viewpoints = [];
        this.experience = null;
        this.climbingMode = false;
        this.savedCameraState = null;
    }

    async load(towerId = null) {
        const tid = towerId || (this.getCurrentTower?.()?.tower_id) || 1;
        let data = null;
        try {
            const resp = await fetch(`${API_BASE}/api/towers/${tid}/climbing`);
            const json = await resp.json();
            if (json.code === 200) data = json.data;
        } catch (e) {}

        if (!data) data = this._generateMockExperience(tid);
        this.experience = data;
        this.viewpoints = data.viewpoints || [];
        return data;
    }

    _generateMockExperience(towerId) {
        const tower = this.getCurrentTower?.() || {
            tower_name: '临冲吕公车-一号',
            total_height: 18.5,
            total_layers: 5,
            base_depth: 4.8,
        };
        const L = tower.total_layers;
        const H = tower.total_height;
        const layerH = H / L;
        const viewpoints = [];

        for (let i = 1; i <= L; i++) {
            const layer_y = i * layerH;
            const h_ratio = i / L;
            const risk = VrSiegeTower.computeAcrophobiaRisk(layer_y);

            viewpoints.push({
                layer_id: i,
                layer_name: `第${i}层`,
                camera_position: [0, layer_y, tower.base_depth / 2 + 0.5],
                look_at: [0, layer_y, tower.base_depth + 20],
                description: h_ratio < 0.3
                    ? '底层视角：观察地面部署与城墙根基'
                    : h_ratio < 0.6
                    ? '中层视角：可观察城墙中部防御与敌军动向'
                    : h_ratio < 0.85
                    ? '高层视角：俯瞰战场全局，观察远距离敌情'
                    : '顶层视角：全面掌控战场态势，通信指挥位置',
                visibility_range_m: 100 + layer_y * 50,
                strategic_value: h_ratio < 0.3
                    ? '近距突击准备'
                    : h_ratio < 0.6
                    ? '中距火力压制'
                    : h_ratio < 0.85
                    ? '远距侦察指挥'
                    : '全局指挥调度',
                height_above_ground_m: layer_y,
                acrophobia_risk_level: risk,
                recommended_fov_deg: VrSiegeTower.computeRecommendedFov(risk),
                transition_duration_ms: VrSiegeTower.computeTransitionDuration(risk),
            });
        }

        return {
            tower_id: towerId,
            tower_name: tower.tower_name,
            viewpoints,
            total_height: H,
            battlefield_description: `${tower.tower_name}高${H}m，共${L}层`,
        };
    }

    static computeAcrophobiaRisk(heightM) {
        if (heightM < 5) return 1;
        if (heightM < 10) return 2;
        if (heightM < 20) return 3;
        if (heightM < 35) return 4;
        return 5;
    }

    static computeRecommendedFov(risk) {
        if (risk <= 2) return 75;
        if (risk === 3) return 65;
        if (risk === 4) return 55;
        return 45;
    }

    static computeTransitionDuration(risk) {
        if (risk <= 2) return 500;
        if (risk === 3) return 1000;
        if (risk === 4) return 1500;
        return 2000;
    }

    enterClimbingMode(viewpoint) {
        this.climbingMode = true;
        const viewer = this.getViewer?.();
        if (viewer) {
            if (!this.savedCameraState) {
                this.savedCameraState = {
                    position: viewer.camera.position.clone(),
                    target: viewer.controls?.target?.clone(),
                    fov: viewer.camera.fov,
                };
            }
            this._animateCameraTransition(viewer, viewpoint);
        }

        document.getElementById('climbingOverlay') &&
            (document.getElementById('climbingOverlay').style.display = 'flex');

        const warningEl = document.getElementById('climbingHeightWarning');
        const risk = viewpoint.acrophobia_risk_level || 1;
        if (warningEl && risk >= 3) {
            warningEl.textContent =
                `⚠ 当前高度 ${(viewpoint.height_above_ground_m || 0).toFixed(1)}m，恐高风险等级 ${risk}/5`;
            warningEl.style.display = 'block';
        } else if (warningEl) {
            warningEl.style.display = 'none';
        }

        const infoEl = document.getElementById('climbingInfo');
        if (infoEl) {
            infoEl.textContent = `${viewpoint.layer_name || '攀登'} · ${viewpoint.strategic_value || ''}`;
        }
        const descEl = document.getElementById('climbingDesc');
        if (descEl) descEl.textContent = viewpoint.description || '';
    }

    _animateCameraTransition(viewer, viewpoint) {
        const THREE = (typeof window !== 'undefined' && window.THREE) || globalThis.THREE;
        const pos = viewpoint.camera_position;
        const look = viewpoint.look_at;
        const targetFov = viewpoint.recommended_fov_deg || DEFAULT_FOV;
        const duration = viewpoint.transition_duration_ms || DEFAULT_DURATION;

        const startPos = viewer.camera.position.clone();
        const startFov = viewer.camera.fov;
        const startTime = performance.now();

        const animate = (now) => {
            const t = Math.min((now - startTime) / duration, 1);
            const ease = t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2;

            if (THREE) {
                viewer.camera.position.lerpVectors(
                    startPos,
                    new THREE.Vector3(pos[0], pos[1], pos[2]),
                    ease,
                );
            } else {
                viewer.camera.position.set(
                    startPos.x + (pos[0] - startPos.x) * ease,
                    startPos.y + (pos[1] - startPos.y) * ease,
                    startPos.z + (pos[2] - startPos.z) * ease,
                );
            }
            viewer.camera.fov = startFov + (targetFov - startFov) * ease;
            viewer.camera.lookAt(look[0], look[1], look[2]);
            viewer.camera.updateProjectionMatrix?.();

            if (t < 1) requestAnimationFrame(animate);
            else if (viewer.controls) viewer.controls.enabled = false;
        };

        requestAnimationFrame(animate);
    }

    exitClimbingMode() {
        this.climbingMode = false;
        const viewer = this.getViewer?.();
        if (viewer && this.savedCameraState) {
            const { position, fov, target } = this.savedCameraState;
            viewer.camera.position.copy(position);
            viewer.camera.fov = fov || DEFAULT_FOV;
            viewer.camera.updateProjectionMatrix?.();
            if (target && viewer.controls) {
                viewer.controls.target.copy(target);
                viewer.controls.enabled = true;
                viewer.controls.update?.();
            }
            this.savedCameraState = null;
        }

        const overlay = document.getElementById('climbingOverlay');
        if (overlay) overlay.style.display = 'none';
        const warning = document.getElementById('climbingHeightWarning');
        if (warning) warning.style.display = 'none';
    }

    renderViewpointButtons(container, onClick) {
        if (!container || !this.viewpoints.length) return;
        container.innerHTML = '';
        for (const vp of this.viewpoints) {
            const btn = document.createElement('button');
            btn.className = 'viewpoint-btn';
            const risk = vp.acrophobia_risk_level || 1;
            if (risk >= 4) btn.classList.add('risk-high');
            else if (risk >= 3) btn.classList.add('risk-mid');

            btn.innerHTML = `<span>${vp.layer_name}</span><small>R${risk}</small>`;
            btn.title = vp.description || '';
            btn.addEventListener('click', () => {
                if (onClick) onClick(vp);
                else this.enterClimbingMode(vp);
            });
            container.appendChild(btn);
        }
    }
}

export const VrSiegeTowerUI = {
    async init(options) {
        const { getCurrentTower, getViewer, viewpointsContainerId, exitBtnId } = options;
        const vr = new VrSiegeTower(getCurrentTower, getViewer);
        await vr.load();

        const container = document.getElementById(viewpointsContainerId || 'climbingViewpoints');
        vr.renderViewpointButtons(container);

        document.getElementById(exitBtnId || 'exitClimbingBtn')?.addEventListener('click', () => {
            vr.exitClimbingMode();
        });

        return vr;
    },
};
