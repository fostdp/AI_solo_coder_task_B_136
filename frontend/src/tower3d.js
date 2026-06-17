import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';

export class Tower3DViewer {
    constructor(canvas, tower) {
        this.canvas = canvas;
        this.tower = tower;
        this.scene = null;
        this.camera = null;
        this.renderer = null;
        this.controls = null;
        this.towerGroup = null;
        this.layerMeshes = [];
        this.stressColors = true;
        this.tiltX = 0;
        this.tiltY = 0;
        this.animating = false;
        this.cutMode = 'none';
        this.explodeAmount = 0;
        this.legendMaxStress = 45;
        this.init();
    }

    init() {
        const rect = this.canvas.parentElement.getBoundingClientRect();
        this.scene = new THREE.Scene();
        this.scene.background = new THREE.Color(0x0a0f1a);
        this.scene.fog = new THREE.FogExp2(0x0a0f1a, 0.015);

        this.camera = new THREE.PerspectiveCamera(45, rect.width / rect.height, 0.1, 1000);
        this.setCameraView('perspective');

        this.renderer = new THREE.WebGLRenderer({
            canvas: this.canvas, antialias: true, alpha: true
        });
        this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
        this.renderer.setSize(rect.width, rect.height, false);
        this.renderer.shadowMap.enabled = true;
        this.renderer.shadowMap.type = THREE.PCFSoftShadowMap;

        this.controls = new OrbitControls(this.camera, this.renderer.domElement);
        this.controls.enableDamping = true;
        this.controls.dampingFactor = 0.05;
        this.controls.minDistance = 8;
        this.controls.maxDistance = 80;
        this.controls.maxPolarAngle = Math.PI * 0.495;

        this.setupLights();
        this.buildTower();
        this.buildGround();
        this.buildGrid();

        window.addEventListener('resize', () => this.onResize());
        this.animate();
    }

    setupLights() {
        this.scene.add(new THREE.AmbientLight(0x404060, 0.6));
        this.scene.add(new THREE.HemisphereLight(0x87ceeb, 0x3a2a1a, 0.4));

        const sun = new THREE.DirectionalLight(0xfff5e1, 1.1);
        sun.position.set(25, 35, 20);
        sun.castShadow = true;
        sun.shadow.mapSize.set(2048, 2048);
        Object.assign(sun.shadow.camera, { near: 0.5, far: 150, left: -40, right: 40, top: 40, bottom: -40 });
        this.scene.add(sun);

        const fill = new THREE.DirectionalLight(0x6080a0, 0.4);
        fill.position.set(-20, 10, -15);
        this.scene.add(fill);

        const rim = new THREE.PointLight(0x4080ff, 0.5, 60);
        rim.position.set(-10, 15, -15);
        this.scene.add(rim);
    }

    buildGround() {
        const geo = new THREE.PlaneGeometry(120, 120, 50, 50);
        const pos = geo.attributes.position;
        for (let i = 0; i < pos.count; i++) {
            const x = pos.getX(i), y = pos.getY(i);
            pos.setZ(i, Math.sin(Math.sqrt(x * x + y * y) * 0.08) * 0.15 + (Math.random() - 0.5) * 0.05);
        }
        geo.computeVertexNormals();
        const ground = new THREE.Mesh(geo, new THREE.MeshStandardMaterial({
            color: 0x2a3a2a, roughness: 0.95, metalness: 0,
        }));
        ground.rotation.x = -Math.PI / 2;
        ground.position.y = -0.05;
        ground.receiveShadow = true;
        this.scene.add(ground);

        const shadow = new THREE.Mesh(
            new THREE.CircleGeometry(12, 64),
            new THREE.MeshBasicMaterial({ color: 0, transparent: true, opacity: 0.35 })
        );
        shadow.rotation.x = -Math.PI / 2;
        shadow.position.y = 0.01;
        this.scene.add(shadow);
    }

    buildGrid() {
        this.scene.add(Object.assign(new THREE.GridHelper(60, 60, 0x2a3a5c, 0x1a2438), { position: new THREE.Vector3(0, 0, 0) }));
        const axes = new THREE.AxesHelper(5);
        axes.position.set(-14, 0.05, -14);
        this.scene.add(axes);
    }

    buildTower() {
        this.towerGroup = new THREE.Group();
        this.layerMeshes = [];
        const tid = this.tower.tower_id;

        if (tid === 3) {
            this._buildLadderCart();
        } else if (tid === 4) {
            this._buildRamCart();
        } else if (tid === 5) {
            this._buildTowerCrane();
        } else {
            this._buildSiegeTower();
        }

        const H = this.tower.total_height;
        const labelCanvas = document.createElement('canvas');
        labelCanvas.width = 320; labelCanvas.height = 72;
        const ctx = labelCanvas.getContext('2d');
        ctx.fillStyle = 'rgba(10,15,26,0.88)';
        if (ctx.roundRect) { ctx.roundRect(0, 0, 320, 72, 12); ctx.fill(); }
        ctx.strokeStyle = '#3b82f6'; ctx.lineWidth = 2; ctx.stroke();
        ctx.fillStyle = '#e8edf5';
        ctx.font = 'bold 22px "Microsoft YaHei", sans-serif';
        ctx.textAlign = 'center';
        ctx.fillText(this.tower.tower_name, 160, 30);
        ctx.fillStyle = '#60a5fa';
        ctx.font = '14px "Microsoft YaHei", sans-serif';
        ctx.fillText(`H=${this.tower.total_height}m  ${this.tower.total_layers}层  ${this.tower.material}`, 160, 54);

        const sprite = new THREE.Sprite(new THREE.SpriteMaterial({ map: new THREE.CanvasTexture(labelCanvas) }));
        sprite.position.set(0, H + 4, 0);
        sprite.scale.set(7.5, 1.7, 1);
        this.towerGroup.add(sprite);

        this.scene.add(this.towerGroup);
        this.applyCutMode();
        this.applyExplosion();
    }

    _buildSiegeTower() {
        const { total_height: H, total_layers: L, base_width: BW, base_depth: BD, material_strength: MS } = this.tower;
        const layerH = H / L;
        this.legendMaxStress = MS;

        for (let wi = 0; wi < 4; wi++) {
            const sign = wi % 2 === 0 ? 1 : -1;
            const sign2 = wi < 2 ? 1 : -1;
            const wheel = new THREE.Mesh(
                new THREE.CylinderGeometry(0.5, 0.5, 0.25, 24),
                new THREE.MeshStandardMaterial({ color: 0x1a1208, roughness: 0.95, metalness: 0.2 })
            );
            wheel.rotation.z = Math.PI / 2;
            wheel.position.set(sign * (BW / 2 - 0.3), 0.5, sign2 * (BD / 2 - 0.3));
            wheel.castShadow = true; wheel.receiveShadow = true;
            this.towerGroup.add(wheel);
        }

        for (let layer = 1; layer <= L; layer++) {
            const layerGroup = new THREE.Group();
            const hRatio = layer / L;
            const scale = 1 - hRatio * 0.3;
            const w = BW * scale, d = BD * scale;
            const yBase = (layer - 1) * layerH;
            const yMid = yBase + layerH / 2;

            const shell = this._createLayerShell(w, d, layerH, layer, yMid);
            layerGroup.add(shell);

            const frames = this._createFrames(w, d, layerH, yBase);
            layerGroup.add(frames);

            const bracing = this._createBracing(w, d, layerH, yMid);
            layerGroup.add(bracing);

            if (layer === L) {
                layerGroup.add(this._createRoof(w, yBase + layerH));
            } else if (layer >= 2) {
                layerGroup.add(this._createWindows(w, d, layerH, yMid));
            }

            layerGroup.userData.layer = layer;
            layerGroup.userData.baseY = yMid;
            this.towerGroup.add(layerGroup);

            this.layerMeshes.push({
                layer,
                group: layerGroup,
                materials: this._collectMaterials(layerGroup),
                stressValue: 0,
                stressRatio: 0,
                baseOpacity: 1.0,
                visible: true,
            });
        }
    }

    _buildLadderCart() {
        const { total_height: H, total_layers: L, base_width: BW, base_depth: BD, material_strength: MS } = this.tower;
        this.legendMaxStress = MS;
        const woodMat = new THREE.MeshStandardMaterial({ color: 0x6b4423, roughness: 0.82, metalness: 0.03 });
        const darkWoodMat = new THREE.MeshStandardMaterial({ color: 0x5a4018, roughness: 0.85, metalness: 0.03 });
        const wheelMat = new THREE.MeshStandardMaterial({ color: 0x1a1208, roughness: 0.95, metalness: 0.2 });

        const base = new THREE.Mesh(new THREE.BoxGeometry(BW, 0.5, BD), woodMat.clone());
        base.position.y = 0.5;
        base.castShadow = true; base.receiveShadow = true;
        this.towerGroup.add(base);

        for (let wi = 0; wi < 4; wi++) {
            const sx = wi % 2 === 0 ? 1 : -1;
            const sz = wi < 2 ? 1 : -1;
            const wheel = new THREE.Mesh(new THREE.CylinderGeometry(0.5, 0.5, 0.25, 24), wheelMat);
            wheel.rotation.z = Math.PI / 2;
            wheel.position.set(sx * (BW / 2 - 0.3), 0.5, sz * (BD / 2 - 0.3));
            wheel.castShadow = true; wheel.receiveShadow = true;
            this.towerGroup.add(wheel);
        }

        const tiltAngle = -Math.PI * 20 / 180;
        const colH = 8;
        for (const sx of [-1, 1]) {
            const col = new THREE.Mesh(new THREE.BoxGeometry(0.2, colH, 0.2), darkWoodMat.clone());
            col.position.set(sx * (BW / 2 - 0.5), 0.5 + colH / 2 * Math.cos(tiltAngle), BD / 2 - 0.5 + colH / 2 * Math.sin(-tiltAngle));
            col.rotation.x = tiltAngle;
            col.castShadow = true;
            this.towerGroup.add(col);
        }

        const ladderLen = 10;
        const railMat = darkWoodMat.clone();
        const leftRail = new THREE.Mesh(new THREE.BoxGeometry(0.12, ladderLen, 0.12), railMat);
        leftRail.position.set(-0.6, 0.5 + ladderLen / 2 * Math.cos(tiltAngle), BD / 2 - 0.5 + ladderLen / 2 * Math.sin(-tiltAngle));
        leftRail.rotation.x = tiltAngle;
        leftRail.castShadow = true;
        this.towerGroup.add(leftRail);

        const rightRail = new THREE.Mesh(new THREE.BoxGeometry(0.12, ladderLen, 0.12), railMat);
        rightRail.position.set(0.6, 0.5 + ladderLen / 2 * Math.cos(tiltAngle), BD / 2 - 0.5 + ladderLen / 2 * Math.sin(-tiltAngle));
        rightRail.rotation.x = tiltAngle;
        rightRail.castShadow = true;
        this.towerGroup.add(rightRail);

        const rungCount = 14;
        for (let ri = 1; ri <= rungCount; ri++) {
            const t = ri / (rungCount + 1);
            const rung = new THREE.Mesh(new THREE.BoxGeometry(0.08, 0.08, 3.2), woodMat.clone());
            rung.position.set(0, 0.5 + t * ladderLen * Math.cos(tiltAngle), BD / 2 - 0.5 + t * ladderLen * Math.sin(-tiltAngle));
            rung.rotation.x = tiltAngle;
            rung.castShadow = true;
            this.towerGroup.add(rung);
        }

        const topY = 0.5 + ladderLen * Math.cos(tiltAngle);
        const topZ = BD / 2 - 0.5 + ladderLen * Math.sin(-tiltAngle);
        const topPlat = new THREE.Mesh(new THREE.BoxGeometry(2.0, 0.15, 1.5), woodMat.clone());
        topPlat.position.set(0, topY, topZ);
        topPlat.castShadow = true;
        this.towerGroup.add(topPlat);

        const layerDefs = [
            { yMid: 0.5, label: 1 },
            { yMid: topY * 0.5, label: 2 },
            { yMid: topY, label: 3 },
        ];

        for (const ld of layerDefs) {
            const layerGroup = new THREE.Group();
            layerGroup.userData.layer = ld.label;
            layerGroup.userData.baseY = ld.yMid;
            this.towerGroup.add(layerGroup);

            this.layerMeshes.push({
                layer: ld.label,
                group: layerGroup,
                materials: this._collectMaterials(layerGroup),
                stressValue: 0,
                stressRatio: 0,
                baseOpacity: 1.0,
                visible: true,
            });
        }
    }

    _buildRamCart() {
        const { total_height: H, total_layers: L, base_width: BW, base_depth: BD, material_strength: MS } = this.tower;
        this.legendMaxStress = MS;
        const darkWoodMat = new THREE.MeshStandardMaterial({ color: 0x4a3a1a, roughness: 0.85, metalness: 0.05 });
        const ironMat = new THREE.MeshStandardMaterial({ color: 0x3a3a3a, roughness: 0.6, metalness: 0.7 });

        const base = new THREE.Mesh(new THREE.BoxGeometry(BW, 1.0, BD), darkWoodMat.clone());
        base.position.y = 0.8;
        base.castShadow = true; base.receiveShadow = true;
        this.towerGroup.add(base);

        for (let wi = 0; wi < 4; wi++) {
            const sx = wi % 2 === 0 ? 1 : -1;
            const sz = wi < 2 ? 1 : -1;
            const wheel = new THREE.Mesh(new THREE.CylinderGeometry(0.7, 0.7, 0.3, 24), new THREE.MeshStandardMaterial({ color: 0x1a1208, roughness: 0.95, metalness: 0.2 }));
            wheel.rotation.z = Math.PI / 2;
            wheel.position.set(sx * (BW / 2 - 0.5), 0.7, sz * (BD / 2 - 0.4));
            wheel.castShadow = true; wheel.receiveShadow = true;
            this.towerGroup.add(wheel);
        }

        const pillarH = 2.5;
        const pillarY = 1.3 + pillarH / 2;
        const pillarMat = darkWoodMat.clone();
        for (const sx of [-1, 1]) {
            for (const sz of [-1, 1]) {
                const pillar = new THREE.Mesh(new THREE.BoxGeometry(0.2, pillarH, 0.2), pillarMat);
                pillar.position.set(sx * (BW / 2 - 0.3), pillarY, sz * (BD / 2 - 0.3));
                pillar.castShadow = true;
                this.towerGroup.add(pillar);
            }
        }

        const roofY = pillarY + pillarH / 2;
        const roofH = 1.2;
        const roofGeo = new THREE.ConeGeometry(BW * 0.7, roofH, 4);
        const roof = new THREE.Mesh(roofGeo, darkWoodMat.clone());
        roof.position.y = roofY + roofH / 2;
        roof.rotation.y = Math.PI / 4;
        roof.castShadow = true;
        this.towerGroup.add(roof);

        const beamY = roofY - 0.3;
        const beam = new THREE.Mesh(new THREE.BoxGeometry(BW * 0.8, 0.2, 0.2), darkWoodMat.clone());
        beam.position.set(0, beamY, 0);
        beam.castShadow = true;
        this.towerGroup.add(beam);

        const ramY = 1.5;
        const ram = new THREE.Mesh(new THREE.CylinderGeometry(0.3, 0.25, 4.0, 16), darkWoodMat.clone());
        ram.rotation.z = Math.PI / 2;
        ram.position.set(0, ramY, 0);
        ram.castShadow = true;
        this.towerGroup.add(ram);

        const headCone = new THREE.Mesh(new THREE.ConeGeometry(0.35, 0.6, 12), ironMat);
        headCone.rotation.z = -Math.PI / 2;
        headCone.position.set(2.3, ramY, 0);
        headCone.castShadow = true;
        this.towerGroup.add(headCone);

        for (const chainX of [-0.8, 0.8]) {
            const chain = new THREE.Mesh(new THREE.CylinderGeometry(0.04, 0.04, beamY - ramY, 8), ironMat);
            chain.position.set(chainX, (beamY + ramY) / 2, 0);
            chain.castShadow = true;
            this.towerGroup.add(chain);
        }

        const layer1Group = new THREE.Group();
        layer1Group.userData.layer = 1;
        layer1Group.userData.baseY = 1.3;
        this.towerGroup.add(layer1Group);
        this.layerMeshes.push({
            layer: 1, group: layer1Group, materials: this._collectMaterials(layer1Group),
            stressValue: 0, stressRatio: 0, baseOpacity: 1.0, visible: true,
        });

        const layer2Group = new THREE.Group();
        layer2Group.userData.layer = 2;
        layer2Group.userData.baseY = ramY;
        this.towerGroup.add(layer2Group);
        this.layerMeshes.push({
            layer: 2, group: layer2Group, materials: this._collectMaterials(layer2Group),
            stressValue: 0, stressRatio: 0, baseOpacity: 1.0, visible: true,
        });
    }

    _buildTowerCrane() {
        const { total_height: H, total_layers: L, material_strength: MS } = this.tower;
        this.legendMaxStress = MS;
        const yellowMat = new THREE.MeshStandardMaterial({ color: 0xcc8800, roughness: 0.6, metalness: 0.3 });
        const steelMat = new THREE.MeshStandardMaterial({ color: 0x888888, roughness: 0.5, metalness: 0.6 });
        const cwMat = new THREE.MeshStandardMaterial({ color: 0x444444, roughness: 0.7, metalness: 0.2 });

        const layerH = H / L;

        for (let layer = 1; layer <= L; layer++) {
            const layerGroup = new THREE.Group();
            const yBase = (layer - 1) * layerH;
            const yMid = yBase + layerH / 2;

            const chordH = layerH;
            for (const sx of [-1, 1]) {
                for (const sz of [-1, 1]) {
                    const chord = new THREE.Mesh(new THREE.BoxGeometry(0.15, chordH, 0.15), yellowMat.clone());
                    chord.position.set(sx * 0.8, yMid, sz * 0.8);
                    chord.castShadow = true;
                    layerGroup.add(chord);
                }
            }

            for (const y of [yBase, yBase + layerH]) {
                for (const sz of [-1, 1]) {
                    const hBrace = new THREE.Mesh(new THREE.BoxGeometry(1.6, 0.08, 0.08), steelMat.clone());
                    hBrace.position.set(0, y, sz * 0.8);
                    hBrace.castShadow = true;
                    layerGroup.add(hBrace);
                }
                for (const sx of [-1, 1]) {
                    const hBrace = new THREE.Mesh(new THREE.BoxGeometry(0.08, 0.08, 1.6), steelMat.clone());
                    hBrace.position.set(sx * 0.8, y, 0);
                    hBrace.castShadow = true;
                    layerGroup.add(hBrace);
                }
            }

            const faces = [
                { sx: 0, sz: 1, rotAxis: 'z' },
                { sx: 0, sz: -1, rotAxis: 'z' },
                { sx: 1, sz: 0, rotAxis: 'x' },
                { sx: -1, sz: 0, rotAxis: 'x' },
            ];
            for (const f of faces) {
                const diagLen = Math.sqrt(1.6 * 1.6 + chordH * chordH);
                const diag1 = new THREE.Mesh(new THREE.BoxGeometry(0.06, diagLen, 0.06), steelMat.clone());
                if (f.rotAxis === 'z') {
                    diag1.position.set(f.sx * 0.8 + 0.4, yMid, f.sz * 0.8);
                    diag1.rotation.z = Math.atan2(chordH, 1.6) * (f.sx || f.sz);
                } else {
                    diag1.position.set(f.sx * 0.8, yMid, f.sz * 0.8 + 0.4);
                    diag1.rotation.x = -Math.atan2(chordH, 1.6) * (f.sx || f.sz);
                }
                diag1.castShadow = true;
                layerGroup.add(diag1);

                const diag2 = diag1.clone();
                if (f.rotAxis === 'z') {
                    diag2.position.x = f.sx * 0.8 - 0.4;
                    diag2.rotation.z = -diag1.rotation.z;
                } else {
                    diag2.position.z = f.sz * 0.8 - 0.4;
                    diag2.rotation.x = -diag1.rotation.x;
                }
                layerGroup.add(diag2);
            }

            layerGroup.userData.layer = layer;
            layerGroup.userData.baseY = yMid;
            this.towerGroup.add(layerGroup);

            this.layerMeshes.push({
                layer,
                group: layerGroup,
                materials: this._collectMaterials(layerGroup),
                stressValue: 0,
                stressRatio: 0,
                baseOpacity: 1.0,
                visible: true,
            });
        }

        const topY = H;
        const slewing = new THREE.Mesh(new THREE.BoxGeometry(2, 1, 2), yellowMat.clone());
        slewing.position.set(0, topY + 0.5, 0);
        slewing.castShadow = true;
        this.towerGroup.add(slewing);

        const cab = new THREE.Mesh(new THREE.BoxGeometry(1.5, 1.5, 1.5), steelMat.clone());
        cab.position.set(0.5, topY + 1.75, 0);
        cab.castShadow = true;
        this.towerGroup.add(cab);

        const jibLen = 30;
        const jib = new THREE.Mesh(new THREE.BoxGeometry(jibLen, 0.8, 0.8), yellowMat.clone());
        jib.position.set(jibLen / 2, topY + 1.4, 0);
        jib.castShadow = true;
        this.towerGroup.add(jib);

        const jibBraceCount = 8;
        for (let i = 1; i <= jibBraceCount; i++) {
            const bx = i * (jibLen / (jibBraceCount + 1));
            const brace = new THREE.Mesh(new THREE.BoxGeometry(0.06, 1.2, 0.06), steelMat.clone());
            brace.position.set(bx, topY + 0.8, 0);
            brace.castShadow = true;
            this.towerGroup.add(brace);
        }

        const cjLen = 10;
        const counterJib = new THREE.Mesh(new THREE.BoxGeometry(cjLen, 0.8, 0.8), yellowMat.clone());
        counterJib.position.set(-cjLen / 2, topY + 1.4, 0);
        counterJib.castShadow = true;
        this.towerGroup.add(counterJib);

        const counterweight = new THREE.Mesh(new THREE.BoxGeometry(3, 2, 3), cwMat);
        counterweight.position.set(-cjLen + 1.5, topY + 0.4, 0);
        counterweight.castShadow = true;
        this.towerGroup.add(counterweight);

        const cableGeo = new THREE.BufferGeometry().setFromPoints([
            new THREE.Vector3(jibLen - 1, topY + 1.8, 0),
            new THREE.Vector3(jibLen - 1, topY - 8, 0),
        ]);
        const cable = new THREE.Line(cableGeo, new THREE.LineBasicMaterial({ color: 0xcccccc }));
        this.towerGroup.add(cable);
    }

    _createLayerShell(w, d, h, layer, yMid) {
        const group = new THREE.Group();
        const thick = 0.12;
        const colors = [0x6b4423, 0x7a4e2a, 0x5d3b1c, 0x8b5a2b];
        const color = colors[layer % colors.length];
        const mat = new THREE.MeshStandardMaterial({
            color, roughness: 0.82, metalness: 0.03,
            side: THREE.DoubleSide, transparent: true, opacity: 0.98,
        });
        const edgeMat = new THREE.LineBasicMaterial({ color: 0x3a2a1a, transparent: true, opacity: 0.6 });

        const faces = [
            { name: 'front', w, h, pos: [0, 0, d / 2 + thick / 2], rot: [0, 0, 0] },
            { name: 'back', w, h, pos: [0, 0, -d / 2 - thick / 2], rot: [0, Math.PI, 0] },
            { name: 'left', w: d, h, pos: [-w / 2 - thick / 2, 0, 0], rot: [0, -Math.PI / 2, 0] },
            { name: 'right', w: d, h, pos: [w / 2 + thick / 2, 0, 0], rot: [0, Math.PI / 2, 0] },
            { name: 'top', w, h: d, pos: [0, h / 2 + thick / 2, 0], rot: [-Math.PI / 2, 0, 0] },
        ];

        for (const f of faces) {
            const geo = new THREE.BoxGeometry(f.w, f.h, thick);
            const mesh = new THREE.Mesh(geo, mat.clone());
            mesh.position.set(...f.pos);
            mesh.rotation.set(...f.rot);
            mesh.position.y += yMid;
            mesh.castShadow = true; mesh.receiveShadow = true;
            mesh.userData.isFace = true;
            mesh.userData.faceName = f.name;
            group.add(mesh);

            const edges = new THREE.LineSegments(new THREE.EdgesGeometry(geo), edgeMat.clone());
            edges.position.copy(mesh.position);
            edges.rotation.copy(mesh.rotation);
            group.add(edges);
        }

        return group;
    }

    _createFrames(w, d, h, yBase) {
        const g = new THREE.Group();
        const mat = new THREE.MeshStandardMaterial({ color: 0x4a3a1a, roughness: 0.75, metalness: 0.05 });
        const ft = 0.18;

        const corners = [[-w / 2, -d / 2], [w / 2, -d / 2], [-w / 2, d / 2], [w / 2, d / 2]];
        for (const [cx, cz] of corners) {
            for (let hi = 0; hi <= 5; hi++) {
                const hy = yBase + (hi / 5) * h + h * 0.1;
                const hThick = hi === 0 || hi === 5 ? 0.1 : 0.08;
                const beamX = new THREE.Mesh(new THREE.BoxGeometry(w + ft, hThick, ft * 0.8), mat);
                beamX.position.set(0, hy, 0);
                beamX.castShadow = true;
                g.add(beamX);
                const beamZ = new THREE.Mesh(new THREE.BoxGeometry(ft * 0.8, hThick, d + ft), mat);
                beamZ.position.set(0, hy, 0);
                beamZ.castShadow = true;
                g.add(beamZ);
            }
            const post = new THREE.Mesh(new THREE.BoxGeometry(ft, h + 0.05, ft), mat);
            post.position.set(cx, yBase + h / 2, cz);
            post.castShadow = true; post.receiveShadow = true;
            g.add(post);
        }
        return g;
    }

    _createBracing(w, d, h, yMid) {
        const g = new THREE.Group();
        const mat = new THREE.MeshStandardMaterial({ color: 0x5a4018, roughness: 0.8 });
        const sides = [
            { axis: 'z', sign: 1, items: [[w / 4, 0.3, 0], [-w / 4, -0.3, 0]] },
            { axis: 'z', sign: -1, items: [[w / 4, -0.3, 0], [-w / 4, 0.3, 0]] },
        ];

        for (const side of sides) {
            for (let half of [-1, 1]) {
                const diag = new THREE.Mesh(new THREE.BoxGeometry(0.08, h * 1.35, 0.08), mat);
                diag.position.set(half * w / 4, yMid, side.sign * (d / 2 + 0.06));
                diag.rotation.z = -0.3 * side.sign;
                diag.castShadow = true;
                g.add(diag);
                const diag2 = diag.clone();
                diag2.position.x = -half * w / 4;
                diag2.rotation.z = 0.3 * side.sign;
                g.add(diag2);

                const diagX = new THREE.Mesh(new THREE.BoxGeometry(0.08, h * 1.35, 0.08), mat);
                diagX.position.set(side.sign * (w / 2 + 0.06), yMid, half * d / 4);
                diagX.rotation.x = 0.3;
                diagX.castShadow = true;
                g.add(diagX);
                const diagX2 = diagX.clone();
                diagX2.position.z = -half * d / 4;
                diagX2.rotation.x = -0.3;
                g.add(diagX2);
            }
        }
        return g;
    }

    _createRoof(w, yTop) {
        const g = new THREE.Group();
        const roofH = 2.5;
        const roof = new THREE.Mesh(
            new THREE.ConeGeometry(w * 0.85, roofH, 4),
            new THREE.MeshStandardMaterial({ color: 0x6a4a1a, roughness: 0.9, side: THREE.DoubleSide })
        );
        roof.position.y = yTop + roofH / 2;
        roof.rotation.y = Math.PI / 4;
        roof.castShadow = true;
        g.add(roof);

        const pole = new THREE.Mesh(
            new THREE.CylinderGeometry(0.05, 0.05, 3, 8),
            new THREE.MeshStandardMaterial({ color: 0x2a2010 })
        );
        pole.position.y = yTop + roofH + 1.5;
        g.add(pole);

        const flagMat = new THREE.MeshStandardMaterial({ color: 0x8b0000, side: THREE.DoubleSide, roughness: 0.9 });
        const flag = new THREE.Mesh(new THREE.PlaneGeometry(1.5, 0.9), flagMat);
        flag.position.set(0.8, yTop + roofH + 2.0, 0);
        g.add(flag);

        const char = new THREE.Mesh(
            new THREE.PlaneGeometry(0.5, 0.5),
            new THREE.MeshBasicMaterial({ color: 0xffcc00 })
        );
        char.position.set(0.8, yTop + roofH + 2.0, 0.01);
        g.add(char);
        return g;
    }

    _createWindows(w, d, h, yMid) {
        const g = new THREE.Group();
        const slitMat = new THREE.MeshStandardMaterial({ color: 0x050505, roughness: 1 });
        const barMat = new THREE.MeshStandardMaterial({ color: 0x2a2010, roughness: 0.9 });
        for (const zSign of [1, -1]) {
            const wW = w * 0.35, wH = h * 0.55;
            const slit = new THREE.Mesh(new THREE.BoxGeometry(wW, wH, 0.1), slitMat);
            slit.position.set(0, yMid, zSign * (d / 2 + 0.02));
            g.add(slit);
            for (let b = 1; b < 4; b++) {
                const bar = new THREE.Mesh(new THREE.BoxGeometry(0.05, wH * 0.95, 0.06), barMat);
                bar.position.set(-wW / 2 + (wW / 4) * b, yMid, zSign * (d / 2 + 0.05));
                g.add(bar);
            }
        }
        return g;
    }

    _collectMaterials(group) {
        const out = [];
        group.traverse(obj => {
            if (obj.isMesh && obj.material) {
                const list = Array.isArray(obj.material) ? obj.material : [obj.material];
                list.forEach((m, i) => out.push({ mesh: obj, index: Array.isArray(obj.material) ? i : -1, base: m.clone() }));
            }
        });
        return out;
    }

    stressColor(ratio) {
        ratio = Math.max(0, Math.min(1, ratio));
        const stops = [
            { t: 0.0, c: new THREE.Color(0x10b981) },
            { t: 0.3, c: new THREE.Color(0x6ee7b7) },
            { t: 0.5, c: new THREE.Color(0xfacc15) },
            { t: 0.7, c: new THREE.Color(0xfb923c) },
            { t: 0.9, c: new THREE.Color(0xf87171) },
            { t: 1.0, c: new THREE.Color(0xdc2626) },
        ];
        for (let i = 0; i < stops.length - 1; i++) {
            if (ratio >= stops[i].t && ratio <= stops[i + 1].t) {
                const k = (ratio - stops[i].t) / (stops[i + 1].t - stops[i].t);
                return stops[i].c.clone().lerp(stops[i + 1].c, k);
            }
        }
        return stops[stops.length - 1].c;
    }

    updateLayerStresses(layerStresses, criticalStress) {
        for (const ls of layerStresses) {
            const entry = this.layerMeshes.find(m => m.layer === ls.layer);
            if (!entry) continue;
            entry.stressValue = ls.stress;
            const ratio = Math.min(ls.stress / criticalStress, 1.0);
            entry.stressRatio = ratio;

            if (this.stressColors) {
                const col = this.stressColor(ratio);
                for (const m of entry.materials) {
                    const isFace = m.mesh.userData.isFace;
                    const mat = m.index >= 0 ? m.mesh.material[m.index] : m.mesh.material;
                    if (mat.color && isFace) {
                        mat.color.copy(col);
                        mat.emissive = col.clone().multiplyScalar(0.08 * ratio + 0.02);
                        if (ratio > 0.75) mat.emissive.multiplyScalar(1.8);
                        mat.needsUpdate = true;
                    } else if (mat.color && m.base.color.getHex() !== 0x1a1208 && !m.mesh.userData.isFace) {
                        const dark = col.clone().multiplyScalar(0.45).offsetHSL(0, 0, -0.15);
                        mat.color.lerp(dark, 0.5);
                        mat.needsUpdate = true;
                    }
                }
            }
        }
        this.updateLegend();
    }

    setStressView(enabled) {
        this.stressColors = enabled;
        if (!enabled) {
            for (const entry of this.layerMeshes) {
                for (const m of entry.materials) {
                    const mat = m.index >= 0 ? m.mesh.material[m.index] : m.mesh.material;
                    if (mat.color && m.base.color) mat.color.copy(m.base.color);
                    if (mat.emissive) mat.emissive.setHex(0);
                }
            }
        } else if (this.layerMeshes.some(e => e.stressRatio > 0)) {
            const crit = this.legendMaxStress;
            const arr = this.layerMeshes.map(e => ({ layer: e.layer, stress: e.stressValue || crit * 0.5 }));
            this.updateLayerStresses(arr, crit);
        }
    }

    updateLegend() {
        const max = this.legendMaxStress || 45;
        const pairs = [['legendMin', 0], ['legendQ1', max * 0.25], ['legendMid', max * 0.5],
            ['legendQ3', max * 0.75], ['legendMax', max]];
        pairs.forEach(([id, v]) => { const el = document.getElementById(id); if (el) el.textContent = v.toFixed(1); });
    }

    updateTilt(tx, ty) {
        this.tiltX = tx; this.tiltY = ty;
        if (this.towerGroup) {
            this.towerGroup.rotation.z = -THREE.MathUtils.degToRad(ty) * 0.9;
            this.towerGroup.rotation.x = THREE.MathUtils.degToRad(tx) * 0.6;
        }
    }

    setLayerOpacity(layer, opacity) {
        const entry = this.layerMeshes.find(e => e.layer === layer);
        if (!entry) return;
        for (const m of entry.materials) {
            const mat = m.index >= 0 ? m.mesh.material[m.index] : m.mesh.material;
            mat.transparent = true;
            mat.opacity = opacity;
            mat.depthWrite = opacity > 0.8;
        }
        entry.baseOpacity = opacity;
    }

    setLayerVisible(layer, visible) {
        const entry = this.layerMeshes.find(e => e.layer === layer);
        if (!entry) return;
        entry.group.visible = visible;
        entry.visible = visible;
    }

    setCutMode(mode = 'none') {
        this.cutMode = mode;
        const BW = this.tower.base_width, BD = this.tower.base_depth;
        for (const entry of this.layerMeshes) {
            const hRatio = entry.layer / this.tower.total_layers;
            const w = BW * (1 - hRatio * 0.3), d = BD * (1 - hRatio * 0.3);
            entry.group.traverse(obj => {
                if (obj.isMesh && obj.userData.isFace) {
                    const nm = obj.userData.faceName;
                    let hide = false;
                    if (mode === 'cut_front' && nm === 'front') hide = true;
                    if (mode === 'cut_right' && nm === 'right') hide = true;
                    if (mode === 'cut_quarter' && (nm === 'front' || nm === 'right')) hide = true;
                    if (mode === 'cut_half') {
                        const p = obj.position;
                        if (p.x > 0 && (nm === 'front' || nm === 'right' || nm === 'top')) hide = true;
                    }
                    obj.visible = !hide;
                    obj.material.opacity = hide ? 0 : (entry.baseOpacity || 0.98);
                }
            });
        }
    }

    applyExplosion(amount = null) {
        if (amount !== null) this.explodeAmount = amount;
        for (const entry of this.layerMeshes) {
            const target = entry.group.userData.baseY;
            const offset = (entry.layer - 1) * this.explodeAmount * 0.8;
            entry.group.position.y = offset;
        }
    }

    setCameraView(view) {
        const H = this.tower.total_height, BW = this.tower.base_width, BD = this.tower.base_depth;
        const D = Math.max(BW, BD, H) * 2.2;
        switch (view) {
            case 'front': this.camera.position.set(0, H * 0.5, D); break;
            case 'side': this.camera.position.set(D, H * 0.5, 0); break;
            case 'top': this.camera.position.set(0, D * 1.2, 0.01); break;
            case 'iso': this.camera.position.set(D * 0.8, D * 0.7, D * 0.8); break;
            case 'perspective': default: this.camera.position.set(-D * 0.9, H * 0.7, D * 0.9);
        }
        if (this.controls) { this.controls.target.set(0, H * 0.4, 0); this.controls.update(); }
        else { this.camera.lookAt(0, H * 0.4, 0); }
    }

    onResize() {
        const r = this.canvas.parentElement.getBoundingClientRect();
        this.camera.aspect = r.width / r.height; this.camera.updateProjectionMatrix();
        this.renderer.setSize(r.width, r.height, false);
    }

    animate() {
        this.animating = true;
        const tick = () => {
            if (!this.animating) return;
            requestAnimationFrame(tick);
            this.controls && this.controls.update();
            const t = Date.now() * 0.001;
            if (this.towerGroup) {
                this.towerGroup.children.forEach(c => {
                    if (c.type === 'Sprite') c.material.opacity = 0.95 + Math.sin(t * 2) * 0.05;
                });
            }
            this.renderer.render(this.scene, this.camera);
        };
        tick();
    }

    dispose() {
        this.animating = false;
        this.renderer && this.renderer.dispose();
        this.controls && this.controls.dispose();
    }
}
