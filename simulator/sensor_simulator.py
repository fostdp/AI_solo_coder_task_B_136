
#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
古代临冲吕公车 - DTU传感器模拟器
=========================================
功能：
1. 模拟攻城塔各层传感器数据（应力、倾斜、风速、温度、振动等）
2. 支持多种风荷载曲线：稳定/阵风/渐变/台风
3. 支持5种土壤类型配置：砂土/粉土/黏土/淤泥/碎石
4. 动态调整土壤含水率
5. 支持故障注入：传感器漂移、数据缺失、异常突变
6. 双输出模式：HTTP POST到API + MQTT发布到Broker

用法：
    python sensor_simulator.py --help
    python sensor_simulator.py --wind-profile typhoon --soil-type clay --moisture 35 --inject-faults
"""

import argparse
import json
import math
import os
import random
import sys
import time
import signal
import logging
from datetime import datetime, timezone
from typing import Optional, List, Dict, Any

import urllib.request
import urllib.error

MQTT_AVAILABLE = False
try:
    import paho.mqtt.client as mqtt
    MQTT_AVAILABLE = True
except ImportError:
    pass


# ========================================
# 配置常量
# ========================================

SOIL_TYPES = {
    "sand":    {"name": "砂土",   "base_settlement": 2.0,  "pressure_kpa": 35.0, "compressibility": 0.02},
    "silt":    {"name": "粉土",   "base_settlement": 5.0,  "pressure_kpa": 28.0, "compressibility": 0.20},
    "clay":    {"name": "黏土",   "base_settlement": 12.0, "pressure_kpa": 22.0, "compressibility": 0.35},
    "silt_soft": {"name": "淤泥", "base_settlement": 25.0, "pressure_kpa": 12.0, "compressibility": 0.55},
    "rock":    {"name": "碎石",   "base_settlement": 0.2,  "pressure_kpa": 85.0, "compressibility": 0.001},
}

WIND_PROFILES = {
    "steady":  "稳定风：风速波动小",
    "gusty":   "阵风：间歇性强阵风",
    "ramp":    "渐变风：风速线性增加",
    "typhoon": "台风：高强度湍流",
    "calm":    "静风：接近无风",
}

LAYER_NAMES = {
    1: "底座层 (Base)",
    2: "次结构层 (Lower)",
    3: "主架构层 (Mid)",
    4: "上层平台 (Upper)",
    5: "顶层塔楼 (Top)",
    6: "瞭望塔尖 (Spire)",
}


# ========================================
# 风荷载生成器
# ========================================
class WindGenerator:
    """多模式风荷载生成器"""

    def __init__(self, profile: str = "gusty", base_mps: float = 10.0,
                 max_mps: float = 40.0, gust_factor: float = 2.5,
                 change_interval: int = 120, direction: str = "random"):
        self.profile = profile
        self.base = base_mps
        self.max = max_mps
        self.gust_factor = gust_factor
        self.change_interval = change_interval
        self.direction_mode = direction
        self.start_time = time.time()
        self.last_wind_x = 0.0
        self.last_wind_y = 0.0
        self.phase = 0.0
        self._rng = random.Random(hash(("wind", time.time_ns())) & 0xFFFFFFFF)

    def get_wind(self, elapsed: float) -> Dict[str, float]:
        t = elapsed
        profile = self.profile

        if profile == "calm":
            speed = self.base * (0.2 + 0.1 * math.sin(t * 0.1))
        elif profile == "steady":
            speed = self.base * (1.0 + 0.08 * math.sin(t * 0.05))
        elif profile == "gusty":
            cycle = math.sin(t * 2 * math.pi / 30)
            gust_pulse = max(0, math.sin(t * 2 * math.pi / self.change_interval + self.phase))
            speed = self.base + cycle * 2 + (gust_pulse ** 3) * self.base * self.gust_factor
        elif profile == "ramp":
            ramp = min(1.0, t / (self.change_interval * 3))
            speed = self.base + (self.max - self.base) * ramp + math.sin(t * 0.3) * 1.5
        elif profile == "typhoon":
            base_typhoon = self.max * 0.7
            turbulence = math.sin(t * 0.8) * 4 + math.sin(t * 2.3) * 2 + self._rng.gauss(0, 3)
            gust = max(0, math.sin(t * 2 * math.pi / 15)) * 10
            speed = base_typhoon + turbulence + gust
        else:
            speed = self.base

        speed = max(0.0, min(speed, self.max * 1.1))

        # 风向 (角度，0=X轴，随时间缓慢变化)
        if self.direction_mode == "fixed":
            angle_rad = 0.0
        elif self.direction_mode == "rotate":
            angle_rad = t * 2 * math.pi / self.change_interval
        else:
            angle_rad = t * 0.02 + math.sin(t * 0.007) * 1.5

        wind_x = speed * math.cos(angle_rad)
        wind_y = speed * math.sin(angle_rad)

        # 简单平滑
        wind_x = self.last_wind_x * 0.6 + wind_x * 0.4
        wind_y = self.last_wind_y * 0.6 + wind_y * 0.4
        self.last_wind_x = wind_x
        self.last_wind_y = wind_y

        wind_load_x = 1.225 * 0.5 * wind_x ** 2 * 0.85
        wind_load_y = 1.225 * 0.5 * wind_y ** 2 * 0.85

        return {
            "speed": speed,
            "wind_x": wind_x,
            "wind_y": wind_y,
            "load_x": wind_load_x,
            "load_y": wind_load_y,
            "angle_deg": math.degrees(angle_rad) % 360,
        }


# ========================================
# 土壤条件生成器
# ========================================
class SoilConditionGenerator:
    """动态土壤条件生成器（含水率动态变化）"""

    def __init__(self, soil_type: str = "sand", moisture_pct: float = 15.0,
                 settlement_mm: float = 0.0, dynamic: bool = True):
        self.soil_type = soil_type if soil_type in SOIL_TYPES else "sand"
        self.base_moisture = moisture_pct
        self.base_settlement = settlement_mm
        self.dynamic = dynamic
        self._rng = random.Random(hash(("soil", time.time_ns())) & 0xFFFFFFFF)

    def get_soil(self, elapsed: float, total_weight_kg: float = 28500.0,
                 base_area_m2: float = 29.76) -> Dict[str, Any]:
        info = SOIL_TYPES[self.soil_type]

        # 动态含水率 (模拟降雨/蒸发)
        if self.dynamic:
            moisture = self.base_moisture + math.sin(elapsed * 2 * math.pi / 3600) * 8
            moisture = max(1.0, min(moisture, 80.0))
        else:
            moisture = self.base_moisture

        # 含水率修正系数 (Terzaghi公式)
        moisture_ratio = (moisture - 10.0) / 40.0
        c_factor = math.exp(-0.025 * max(0, moisture - 15))
        phi_factor = 1.0 - 0.008 * max(0, moisture_ratio)
        capacity_factor = c_factor * phi_factor

        base_bearing = info["pressure_kpa"]
        bearing_kpa = max(5.0, base_bearing * capacity_factor)

        # 施加压力 (重量 + 动力放大)
        weight = total_weight_kg * 9.81 / 1000.0
        dynamic_factor = 1.0 + math.sin(elapsed * 0.03) * 0.05
        applied_kpa = (weight / base_area_m2) * dynamic_factor + 8.0

        settlement = self.base_settlement + info["base_settlement"] * (applied_kpa / bearing_kpa) ** 2.3
        settlement += self._rng.gauss(0, 0.3)

        passability = max(0.0, min(100.0, 100 * (bearing_kpa / applied_kpa - 1.0)))

        return {
            "soil_type": self.soil_type,
            "soil_name": info["name"],
            "moisture_pct": round(moisture, 2),
            "bearing_capacity_kpa": round(bearing_kpa, 3),
            "applied_pressure_kpa": round(applied_kpa, 3),
            "capacity_util_ratio": round(applied_kpa / bearing_kpa, 4),
            "settlement_mm": round(settlement, 3),
            "passability_score": round(passability, 1),
            "rain_simulated": moisture > self.base_moisture + 3,
        }


# ========================================
# 主模拟器
# ========================================
class TowerSensorSimulator:
    """攻城塔多层传感器模拟器"""

    def __init__(self, args: argparse.Namespace):
        self.args = args
        self.tower_id = args.tower_id
        self.tower_name = args.tower_name
        self.interval = args.interval_sec
        self.total_layers = 5
        self.shutdown = False
        self._rng = random.Random(args.seed if args.seed else None)

        self.wind = WindGenerator(
            profile=args.wind_profile,
            base_mps=args.wind_base,
            max_mps=args.wind_max,
            gust_factor=args.wind_gust_factor,
            change_interval=args.wind_change_interval,
            direction=args.wind_direction,
        )
        self.soil = SoilConditionGenerator(
            soil_type=args.soil_type,
            moisture_pct=args.soil_moisture,
            settlement_mm=args.soil_settlement,
            dynamic=args.soil_dynamic,
        )

        self.fault_injector = FaultInjector(
            enabled=args.inject_faults,
            probability=args.fault_probability,
            rng=self._rng,
        )

        self.api_url = f"{args.api_base.rstrip('/')}/api/towers/{self.tower_id}/sensor"
        self.mqtt_topic = args.mqtt_topic
        self.output_mode = args.output_mode

        self.http_client = None
        self.mqtt_client = None

        self._setup_logging()
        self._setup_mqtt()

    # ---------- 初始化 ----------
    def _setup_logging(self):
        log_dir = "logs"
        os.makedirs(log_dir, exist_ok=True)
        log_file = os.path.join(log_dir, f"simulator_{datetime.now():%Y%m%d}.log")
        logging.basicConfig(
            level=getattr(logging, self.args.log_level.upper(), logging.INFO),
            format="%(asctime)s | %(levelname)-7s | %(name)s | %(message)s",
            handlers=[
                logging.FileHandler(log_file, encoding="utf-8"),
                logging.StreamHandler(sys.stdout),
            ]
        )
        self.log = logging.getLogger("SIM")

    def _setup_mqtt(self):
        if self.output_mode in ("mqtt", "both") and MQTT_AVAILABLE:
            try:
                client = mqtt.Client(client_id=f"sim_tower_{self.tower_id}_{int(time.time())}",
                                     clean_session=True)
                client.connect_async(self.args.mqtt_broker, self.args.mqtt_port, keepalive=60)
                client.loop_start()
                self.mqtt_client = client
                self.log.info(f"MQTT连接中 -> {self.args.mqtt_broker}:{self.args.mqtt_port}")
            except Exception as e:
                self.log.warning(f"MQTT初始化失败: {e}")

    # ---------- 数据生成 ----------
    def _layer_params(self, layer_id: int) -> Dict[str, float]:
        """层相关参数：越高层风效应越大、应力放大"""
        wf = 0.35 + 0.22 * layer_id
        sf = 0.45 + 0.18 * layer_id
        tf = 0.05 + 0.03 * layer_id
        vf = 0.3 + 0.18 * layer_id
        return {"wf": wf, "sf": sf, "tf": tf, "vf": vf, "weight_kg": 28500.0 / self.total_layers}

    def generate_layer(self, layer_id: int, wind: Dict, soil: Dict,
                       elapsed: float) -> Dict[str, Any]:
        """生成单层传感器数据"""
        p = self._layer_params(layer_id)
        rng = self._rng

        # 风在该层的效应 (层越高放大越多)
        layer_wind = wind["speed"] * p["wf"]
        stress_z = self.args.stress_base * p["sf"] + layer_wind ** 2 * 0.21
        stress_x = stress_z * 0.28 * abs(wind["wind_x"]) / max(0.1, wind["speed"])
        stress_y = stress_z * 0.28 * abs(wind["wind_y"]) / max(0.1, wind["speed"])
        von = (stress_x ** 2 + stress_y ** 2 + stress_z ** 2 - stress_x * stress_y
               - stress_y * stress_z - stress_z * stress_x
               + 3 * (stress_x * 0.15) ** 2) ** 0.5

        # 倾斜 (顶层放大)
        tilt_x = self.args.tilt_base * p["tf"] + math.copysign(1, wind["wind_x"]) * (
                layer_wind ** 1.6) * 0.006
        tilt_y = self.args.tilt_base * p["tf"] + math.copysign(1, wind["wind_y"]) * (
                layer_wind ** 1.6) * 0.006

        # 沉降 (各层略有差异，底层最大)
        layer_settlement = soil["settlement_mm"] * (1.0 - 0.1 * (layer_id - 1))

        # 温度 (顶层低)
        temp = self.args.temp_base - 0.8 * (layer_id - 1) + rng.gauss(0, 0.4)
        humidity = self.args.humidity_base + 1.5 * (layer_id - 1) + rng.gauss(0, 1.2)

        # 振动频率和振幅
        vib_freq = 2.1 - 0.15 * (layer_id - 1) + rng.gauss(0, 0.08)
        vib_amp = 0.2 + 0.12 * layer_id + layer_wind * 0.04 + rng.gauss(0, 0.05)

        displacement = 0.4 + 0.9 * layer_id + math.sqrt(tilt_x ** 2 + tilt_y ** 2) * 35 + rng.gauss(0, 0.3)

        layer = {
            "layer_id": layer_id,
            "layer_name": LAYER_NAMES.get(layer_id, f"Layer-{layer_id}"),
            "wind_speed_mps": round(layer_wind, 3),
            "wind_load_x_nm2": round(wind["load_x"] * p["wf"], 3),
            "wind_load_y_nm2": round(wind["load_y"] * p["wf"], 3),
            "stress_x": round(stress_x + rng.gauss(0, 0.8), 4),
            "stress_y": round(stress_y + rng.gauss(0, 0.8), 4),
            "stress_z": round(stress_z + rng.gauss(0, 1.5), 4),
            "stress_von_mises": round(von + rng.gauss(0, 2.0), 4),
            "tilt_x_deg": round(tilt_x, 4),
            "tilt_y_deg": round(tilt_y, 4),
            "tilt_total_deg": round(math.sqrt(tilt_x ** 2 + tilt_y ** 2), 4),
            "temperature_c": round(temp, 2),
            "humidity_pct": round(max(0, min(100, humidity)), 2),
            "vibration_freq_hz": round(max(0.1, vib_freq), 4),
            "vibration_amp_mm": round(max(0, vib_amp), 4),
            "displacement_mm": round(max(0, displacement), 3),
            "ground_settlement_mm": round(layer_settlement, 3),
            "ground_pressure_kpa": round(soil["applied_pressure_kpa"], 3),
            "soil_bearing_kpa": round(soil["bearing_capacity_kpa"], 3),
            "soil_moisture_pct": round(soil["moisture_pct"], 2),
            "weight_on_soil_kg": round(p["weight_kg"], 1),
            "soil_type": soil["soil_type"],
        }

        if self.fault_injector.enabled:
            layer = self.fault_injector.apply(layer, layer_id)
        return layer

    # ---------- 输出 ----------
    def _send_http(self, payload: Dict) -> bool:
        try:
            data = json.dumps(payload).encode("utf-8")
            req = urllib.request.Request(
                self.api_url,
                data=data,
                headers={"Content-Type": "application/json"},
                method="POST",
            )
            with urllib.request.urlopen(req, timeout=10) as resp:
                ok = 200 <= resp.status < 300
                if not ok:
                    self.log.warning(f"HTTP异常状态: {resp.status}")
                return ok
        except urllib.error.HTTPError as e:
            self.log.warning(f"HTTP {e.code}: {e.read()[:200]}")
            return False
        except Exception as e:
            self.log.error(f"HTTP发送失败: {e}")
            return False

    def _send_mqtt(self, payload: Dict) -> bool:
        if not self.mqtt_client:
            return False
        try:
            msg = json.dumps(payload, ensure_ascii=False)
            info = self.mqtt_client.publish(self.mqtt_topic, msg, qos=0)
            return info.rc == 0
        except Exception as e:
            self.log.error(f"MQTT发送失败: {e}")
            return False

    # ---------- 主循环 ----------
    def run(self):
        def _shutdown(signum, frame):
            self.log.info(f"收到信号 {signum}, 正在优雅退出...")
            self.shutdown = True

        signal.signal(signal.SIGINT, _shutdown)
        signal.signal(signal.SIGTERM, _shutdown)

        self.log.info("=" * 60)
        self.log.info("攻城塔传感器模拟器启动")
        self.log.info(f"  塔ID:        {self.tower_id} ({self.tower_name})")
        self.log.info(f"  层数:        {self.total_layers}")
        self.log.info(f"  发送间隔:    {self.interval}s")
        self.log.info(f"  风工况:      {self.args.wind_profile} [{WIND_PROFILES[self.args.wind_profile]}]")
        self.log.info(f"    基础风速:  {self.args.wind_base} m/s, 极限: {self.args.wind_max} m/s")
        self.log.info(f"  土壤类型:    {self.soil.soil_type} [{SOIL_TYPES[self.soil.soil_type]['name']}]")
        self.log.info(f"    含水率:    {self.args.soil_moisture}%")
        self.log.info(f"  故障注入:    {'开' if self.args.inject_faults else '关'}")
        self.log.info(f"  输出模式:    {self.output_mode}")
        self.log.info(f"    HTTP:      {self.api_url}")
        if self.mqtt_client:
            self.log.info(f"    MQTT:      {self.args.mqtt_broker}:{self.args.mqtt_port} -> {self.mqtt_topic}")
        self.log.info("=" * 60)

        tick = 0
        start = time.time()
        ok_count = 0
        fail_count = 0

        while not self.shutdown:
            try:
                elapsed = time.time() - start
                tick += 1

                wind = self.wind.get_wind(elapsed)
                soil = self.soil.get_soil(elapsed)

                layers = [self.generate_layer(lid, wind, soil, elapsed)
                          for lid in range(1, self.total_layers + 1)]

                payload = {
                    "tower_id": self.tower_id,
                    "tower_name": self.tower_name,
                    "timestamp": datetime.now(timezone.utc).isoformat(),
                    "elapsed_sec": round(elapsed, 2),
                    "tick": tick,
                    "summary": {
                        "wind_speed_mps": round(wind["speed"], 3),
                        "wind_angle_deg": round(wind["angle_deg"], 2),
                        "wind_load_total_nm2": round(math.sqrt(wind["load_x"]**2 + wind["load_y"]**2), 3),
                        "soil_name": soil["soil_name"],
                        "soil_moisture_pct": soil["moisture_pct"],
                        "bearing_capacity_kpa": soil["bearing_capacity_kpa"],
                        "capacity_util_ratio": soil["capacity_util_ratio"],
                        "settlement_total_mm": soil["settlement_mm"],
                        "passability_score": soil["passability_score"],
                        "rain_event": soil["rain_simulated"],
                    },
                    "layers": layers,
                }

                ok = True
                if self.output_mode in ("http", "both"):
                    http_ok = self._send_http(payload)
                    ok = ok and http_ok
                if self.output_mode in ("mqtt", "both"):
                    mqtt_ok = self._send_mqtt(payload)
                    ok = ok and mqtt_ok

                if ok:
                    ok_count += 1
                else:
                    fail_count += 1

                if tick % 12 == 1 or not ok:
                    max_stress = max(l["stress_von_mises"] for l in layers)
                    max_tilt = max(l["tilt_total_deg"] for l in layers)
                    self.log.info(
                        f"Tick {tick:>5} | W:{wind['speed']:>5.1f}m/s @{wind['angle_deg']:.0f}° "
                        f"| 土壤:{soil['soil_name']} φ={soil['moisture_pct']:.1f}% "
                        f"承载力比={soil['capacity_util_ratio']:.2f} | "
                        f"max(σ)={max_stress:.2f}MPa max(θ)={max_tilt:.3f}° "
                        f"[OK:{ok_count} FAIL:{fail_count}]"
                    )

                # 精确等待
                elapsed_this = time.time() - start
                next_tick = (tick + 1) * self.interval
                sleep_time = max(0.05, next_tick - elapsed_this)
                time.sleep(sleep_time)

            except Exception as e:
                self.log.error(f"主循环异常: {e}", exc_info=True)
                time.sleep(self.interval)

        self.log.info(f"模拟器退出: 共发送 {ok_count + fail_count} 包, OK={ok_count}, FAIL={fail_count}")
        if self.mqtt_client:
            try:
                self.mqtt_client.loop_stop()
                self.mqtt_client.disconnect()
            except Exception:
                pass


# ========================================
# 故障注入器
# ========================================
class FaultInjector:
    """传感器故障注入：漂移、丢失、噪声突变、卡死"""

    def __init__(self, enabled: bool = False, probability: float = 0.02, rng=None):
        self.enabled = enabled
        self.p = probability
        self.rng = rng or random.Random()
        self._drift = {}
        self._stuck = {}

    def apply(self, layer: Dict, layer_id: int) -> Dict:
        if not self.enabled:
            return layer

        r = self.rng.random()

        # 漂移故障
        if layer_id not in self._drift:
            self._drift[layer_id] = {k: 0.0 for k in ("stress_x", "stress_y", "stress_z", "tilt_x_deg", "tilt_y_deg")}
        if r < self.p * 0.1:
            for k in self._drift[layer_id]:
                self._drift[layer_id][k] += self.rng.gauss(0, 0.05 * abs(layer.get(k, 1.0)))
        for k, v in self._drift[layer_id].items():
            if k in layer:
                layer[k] = round(layer[k] + v, 5)

        # 卡死故障 (重置卡死概率)
        if layer_id not in self._stuck:
            self._stuck[layer_id] = None
        if self._stuck[layer_id] is None and r < self.p * 0.02:
            self._stuck[layer_id] = dict(layer)
            layer["_fault"] = "stuck"
        elif self._stuck[layer_id] is not None:
            if self.rng.random() < 0.1:
                self._stuck[layer_id] = None
                layer["_fault_recovery"] = True
            else:
                stuck = self._stuck[layer_id]
                for k in ("stress_x", "stress_y", "stress_z", "wind_speed_mps",
                          "tilt_x_deg", "tilt_y_deg", "temperature_c", "humidity_pct"):
                    if k in stuck:
                        layer[k] = stuck[k]
                return layer

        # 异常突变
        if self.p < r < self.p * 2.5:
            for k in ("stress_x", "stress_y", "stress_z"):
                layer[k] = round(layer[k] * self.rng.uniform(2.5, 6.0), 4)
            layer["_fault"] = "spike"

        # 数据缺失 (强制某些值为 None)
        if self.p * 5 < r < self.p * 5.3:
            miss_key = self.rng.choice(["vibration_freq_hz", "humidity_pct", "temperature_c"])
            layer[miss_key] = None
            layer["_fault"] = "missing"

        # 严重越界
        if r > 1 - self.p * 0.05:
            layer["tilt_x_deg"] = round(layer["tilt_x_deg"] * 15, 4)
            layer["_fault"] = "out_of_range"

        return layer


# ========================================
# 命令行入口
# ========================================
def main():
    parser = argparse.ArgumentParser(
        description="攻城塔DTU传感器模拟器 (风荷载+土壤条件可配置)",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
示例:
  # 台风 + 高含水率黏土
  python sensor_simulator.py --wind-profile typhoon --soil-type clay --moisture 40

  # 稳定风 + 碎石地基 高频率
  python sensor_simulator.py --wind-profile steady -b 15 -i 1 --soil-type rock

  # 仅HTTP输出 不输出MQTT
  python sensor_simulator.py --output http

  # 不注入故障 适合基准测试
  python sensor_simulator.py --no-faults
        """,
    )

    parser.add_argument("-m", "--mode", default="hybrid", choices=["hybrid"], help="运行模式 (预留)")
    parser.add_argument("-i", "--interval-sec", type=float,
                        default=float(os.environ.get("SIM_INTERVAL_SEC", 5)), help="发送间隔秒数")
    parser.add_argument("--tower-id", type=int,
                        default=int(os.environ.get("SIM_TOWER_ID", 1)), help="塔ID")
    parser.add_argument("--tower-name",
                        default=os.environ.get("SIM_TOWER_NAME", "临冲吕公车-一号"), help="塔名称")

    # 风荷载
    parser.add_argument("-w", "--wind-profile",
                        default=os.environ.get("WIND_PROFILE", "gusty"),
                        choices=list(WIND_PROFILES.keys()),
                        help="风工况 (详见上方常量)")
    parser.add_argument("-b", "--wind-base", type=float,
                        default=float(os.environ.get("WIND_BASE_MPS", 10.0)), help="基础风速 m/s")
    parser.add_argument("--wind-max", type=float,
                        default=float(os.environ.get("WIND_MAX_MPS", 40.0)), help="最大风速 m/s")
    parser.add_argument("--wind-gust-factor", type=float,
                        default=float(os.environ.get("WIND_GUST_FACTOR", 2.5)), help="阵风放大系数")
    parser.add_argument("--wind-change-interval", type=int,
                        default=int(os.environ.get("WIND_CHANGE_INTERVAL", 120)), help="风变化周期(秒)")
    parser.add_argument("--wind-direction",
                        default=os.environ.get("WIND_DIRECTION", "random"),
                        choices=["random", "fixed", "rotate"], help="风向模式")

    # 土壤
    parser.add_argument("-s", "--soil-type",
                        default=os.environ.get("SOIL_TYPE", "sand"),
                        choices=list(SOIL_TYPES.keys()),
                        help="土壤类型 (sand/silt/clay/silt_soft/rock)")
    parser.add_argument("--soil-moisture", "--moisture", dest="soil_moisture", type=float,
                        default=float(os.environ.get("SOIL_MOISTURE_PCT", 15.0)),
                        help="基础土壤含水率%%")
    parser.add_argument("--soil-settlement", type=float,
                        default=float(os.environ.get("SOIL_SETTLEMENT_MM", 0.0)),
                        help="初始沉降量 mm")
    parser.add_argument("--soil-static", dest="soil_dynamic", action="store_false",
                        default=os.environ.get("SOIL_DYNAMIC", "true").lower() in ("true", "1", "yes"),
                        help="禁用土壤含水率动态变化")
    parser.add_argument("--soil-dynamic", dest="soil_dynamic", action="store_true", help=argparse.SUPPRESS)

    # 基础
    parser.add_argument("--stress-base", type=float,
                        default=float(os.environ.get("STRESS_BASE_MPA", 25.0)), help="基础应力 MPa")
    parser.add_argument("--tilt-base", type=float,
                        default=float(os.environ.get("TILT_BASE_DEG", 0.5)), help="基础倾斜 °")
    parser.add_argument("--temp-base", type=float,
                        default=float(os.environ.get("TEMP_BASE_C", 20.0)), help="基础温度 °C")
    parser.add_argument("--humidity-base", type=float,
                        default=float(os.environ.get("HUMIDITY_BASE_PCT", 50.0)), help="基础湿度 %%")

    # 故障
    parser.add_argument("--no-faults", dest="inject_faults", action="store_false",
                        default=os.environ.get("INJECT_FAULTS", "true").lower() in ("true", "1", "yes"),
                        help="禁用故障注入")
    parser.add_argument("--faults", dest="inject_faults", action="store_true", help=argparse.SUPPRESS)
    parser.add_argument("--fault-probability", type=float,
                        default=float(os.environ.get("FAULT_PROBABILITY", 0.02)),
                        help="每包故障概率 (0.02=2%%)")

    # 输出
    parser.add_argument("--api-base",
                        default=os.environ.get("API_BASE_URL", "http://localhost:8080"), help="后端API基址")
    parser.add_argument("--mqtt-broker",
                        default=os.environ.get("MQTT_BROKER", "localhost"), help="MQTT Broker地址")
    parser.add_argument("--mqtt-port", type=int,
                        default=int(os.environ.get("MQTT_PORT", 1883)), help="MQTT端口")
    parser.add_argument("--mqtt-topic",
                        default=os.environ.get("MQTT_SENSOR_TOPIC", "siege_tower/sensor"),
                        help="MQTT传感器主题")
    parser.add_argument("-o", "--output", dest="output_mode",
                        default=os.environ.get("OUTPUT_MODE", "both"),
                        choices=["http", "mqtt", "both", "stdout"],
                        help="输出模式: http/mqtt/both/stdout")
    parser.add_argument("--log-level", default="INFO",
                        choices=["DEBUG", "INFO", "WARNING", "ERROR"], help="日志级别")
    parser.add_argument("--seed", type=int, default=None, help="随机种子 (重现测试场景)")

    args = parser.parse_args()

    if args.output_mode == "stdout":
        args.output_mode = "both"

    sim = TowerSensorSimulator(args)
    sim.run()


if __name__ == "__main__":
    main()
