//! 物理与设备模型：无线电传播时延、设备处理时延、中继工作模式、仿真场景。

use crate::geo::Vec2;

/// 真空光速（m/s）。空气传播速度按真空光速近似，误差约 0.03%。
pub const SPEED_OF_LIGHT: f64 = 299_792_458.0;

/// 中继设备的工作模式。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelayMode {
    /// 模拟/放大转发：解调门槛低，转发处理快（参考值约 15–50 ns）。
    AnalogAmplify,
    /// 数字解码再编码转发：处理更慢（参考值约 100–500 ns），但可整形信号。
    DecodeForward,
    /// 仅用于把场景退回无中继直连。
    Bypass,
}

impl RelayMode {
    pub fn label(self) -> &'static str {
        match self {
            RelayMode::AnalogAmplify => "模拟放大转发",
            RelayMode::DecodeForward => "解码转发",
            RelayMode::Bypass => "关闭（直连）",
        }
    }

    /// 该模式默认的单台中继转发处理时延均值（ns）。
    pub fn default_processing_ns(self) -> f64 {
        match self {
            RelayMode::AnalogAmplify => 25.0,
            RelayMode::DecodeForward => 200.0,
            RelayMode::Bypass => 0.0,
        }
    }
}

/// 全部仿真参数。时间单位除注明外均为纳秒（ns）。
#[derive(Clone, Debug, PartialEq)]
pub struct Params {
    /// 距离边界协议快速交换轮数 N（Hancke–Kuhn 经典取值 24–64）。
    pub rounds: usize,
    /// 车端 RTT 门限（ns）：单轮 RTT 超过即记一次“超时”。
    pub rtt_threshold_ns: f64,
    /// 允许的失败轮数（超时 + 应答位错误合并计数）。
    pub max_fail_rounds: usize,

    /// 钥匙每轮的快速应答处理时延均值（ns），UWB 收发机典型 40–150 ns。
    pub key_proc_mean_ns: f64,
    /// 钥匙处理时延随机抖动标准差（ns），涵盖时钟漂移、MAC/PHY 抖动。
    pub key_proc_std_ns: f64,
    /// 每轮 RTT 测量附加噪声标准差（ns），车端测量电路噪声。
    pub rtt_noise_std_ns: f64,
    /// 单比特应答的误码率（无线信道误码，与距离/中继无关的独立故障源）。
    pub bit_error_rate: f64,

    /// 单台中继设备转发处理时延均值（ns）。
    pub relay_proc_mean_ns: f64,
    /// 中继处理时延抖动标准差（ns）。
    pub relay_proc_std_ns: f64,
    /// 两台中继之间回传链路额外单向时延（ns），模拟长距离线缆/电台回传。
    pub relay_backhaul_ns: f64,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            rounds: 32,
            rtt_threshold_ns: 150.0,
            max_fail_rounds: 2,
            key_proc_mean_ns: 80.0,
            key_proc_std_ns: 4.0,
            rtt_noise_std_ns: 2.0,
            bit_error_rate: 1e-4,
            relay_proc_mean_ns: RelayMode::AnalogAmplify.default_processing_ns(),
            relay_proc_std_ns: 5.0,
            relay_backhaul_ns: 0.0,
        }
    }
}

impl Params {
    /// 直连链路距离 d 对应的单轮 RTT 几何下界（无噪声、无处理时）：2d/c，单位 ns。
    pub fn geometric_floor_ns(distance_m: f64) -> f64 {
        2.0 * distance_m / SPEED_OF_LIGHT * 1e9
    }

    /// RTT（ns）按 2d/c 反推出的“等效距离”（m），已扣除钥匙标称处理时延。
    pub fn equivalent_distance_m(rtt_ns: f64, key_proc_mean_ns: f64) -> f64 {
        let air = (rtt_ns - key_proc_mean_ns).max(0.0);
        air * 1e-9 * SPEED_OF_LIGHT / 2.0
    }
}

/// 一条消息路径上的一跳：到下一节点的直线距离 + 在该节点上的处理时延分布。
#[derive(Clone, Copy, Debug)]
pub struct PathLeg {
    /// 本跳空中距离（米）。
    pub distance_m: f64,
    /// 本跳起始设备的处理时延均值（ns）；直连场景下车端发起处理已在标定中扣除，取 0。
    pub proc_mean_ns: f64,
    /// 处理时延抖动标准差（ns）。
    pub proc_std_ns: f64,
}

impl PathLeg {
    /// 本跳单向时延（ns）：传播时延 + 该设备处理时延。
    pub fn one_way_ns(&self) -> f64 {
        Params::geometric_floor_ns(self.distance_m) / 2.0
    }
}

/// 一次认证面对的物理场景：设备布点 + 中继模式。
#[derive(Clone, Debug)]
pub struct Scenario {
    pub vehicle: Vec2,
    pub key: Vec2,
    pub relay_a: Vec2,
    pub relay_b: Vec2,
    /// 使用的中继数量（0 = 直连；1/2 对应单/双中继）。
    pub relay_count: usize,
    pub relay_mode: RelayMode,
}

impl Scenario {
    /// 车—钥匙直线距离（米）。
    pub fn direct_distance_m(&self) -> f64 {
        self.vehicle.distance(self.key)
    }

    /// 构造车端 → 钥匙方向的消息路径（挑战方向）。
    ///
    /// 处理时延只在“转发节点”上发生：
    /// * 直连：无中继，路径上没有任何设备处理项（车端发起时延假定已在门限标定中扣除）；
    /// * 单中继：车 → R → 钥匙，R 上一次中继处理；
    /// * 双中继：车 → RA →（回传）→ RB → 钥匙，RA、RB 各一次中继处理，
    ///   回传链路以额外距离等效项计入 RA→RB 这一跳。
    pub fn challenge_legs(&self, p: &Params) -> Vec<PathLeg> {
        match self.relay_count {
            0 => vec![PathLeg {
                distance_m: self.direct_distance_m(),
                proc_mean_ns: 0.0,
                proc_std_ns: 0.0,
            }],
            1 => vec![
                PathLeg {
                    distance_m: self.vehicle.distance(self.relay_a),
                    proc_mean_ns: 0.0,
                    proc_std_ns: 0.0,
                },
                PathLeg {
                    distance_m: self.relay_a.distance(self.key),
                    proc_mean_ns: p.relay_proc_mean_ns,
                    proc_std_ns: p.relay_proc_std_ns,
                },
            ],
            _ => vec![
                PathLeg {
                    distance_m: self.vehicle.distance(self.relay_a),
                    proc_mean_ns: 0.0,
                    proc_std_ns: 0.0,
                },
                PathLeg {
                    // 回传额外时延按等效距离折算进这一跳。
                    distance_m: self.relay_a.distance(self.relay_b)
                        + p.relay_backhaul_ns * 1e-9 * SPEED_OF_LIGHT,
                    proc_mean_ns: p.relay_proc_mean_ns,
                    proc_std_ns: p.relay_proc_std_ns,
                },
                PathLeg {
                    distance_m: self.relay_b.distance(self.key),
                    proc_mean_ns: p.relay_proc_mean_ns,
                    proc_std_ns: p.relay_proc_std_ns,
                },
            ],
        }
    }

    /// 路径总空中距离（米）。直连时等于车—钥匙距离。
    pub fn path_distance_m(&self, p: &Params) -> f64 {
        self.challenge_legs(p).iter().map(|l| l.distance_m).sum()
    }

    /// 单轮 RTT 的确定性下界（ns）：
    /// 往返传播 2L/c + 钥匙处理 + 路径上全部中继处理（每台中继往返各转发一次）。
    pub fn rtt_floor_ns(&self, p: &Params) -> f64 {
        let legs = self.challenge_legs(p);
        let propagation = 2.0 * legs.iter().map(|l| l.one_way_ns()).sum::<f64>();
        // 每台中继在去程、回程各转发一次 → 处理时延计两次；钥匙处理一次。
        let relay_proc: f64 = legs.iter().map(|l| l.proc_mean_ns).sum();
        propagation + p.key_proc_mean_ns + 2.0 * relay_proc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_meter_propagation_time() {
        // 1 米单向传播约 3.3356 ns。
        let t = Params::geometric_floor_ns(1.0) / 2.0;
        assert!((t - 3.3356).abs() < 0.001);
    }

    #[test]
    fn relay_floor_exceeds_direct_floor() {
        let p = Params::default();
        let direct = Scenario {
            vehicle: Vec2::new(0.0, 0.0),
            key: Vec2::new(8.0, 0.0),
            relay_a: Vec2::new(4.0, 3.0),
            relay_b: Vec2::new(0.0, 0.0),
            relay_count: 0,
            relay_mode: RelayMode::Bypass,
        };
        let mut relayed = direct.clone();
        relayed.relay_count = 1;
        assert!(relayed.rtt_floor_ns(&p) > direct.rtt_floor_ns(&p));
        // 直连下界 = 2d/c + 钥匙处理。
        let expected = Params::geometric_floor_ns(8.0) + p.key_proc_mean_ns;
        assert!((direct.rtt_floor_ns(&p) - expected).abs() < 1e-9);
    }

    #[test]
    fn two_relay_backhaul_adds_latency() {
        let p = Params::default();
        let mut s = Scenario {
            vehicle: Vec2::new(0.0, 0.0),
            key: Vec2::new(20.0, 0.0),
            relay_a: Vec2::new(2.0, 0.0),
            relay_b: Vec2::new(18.0, 0.0),
            relay_count: 2,
            relay_mode: RelayMode::AnalogAmplify,
        };
        let base = s.rtt_floor_ns(&p);
        s.relay_b = Vec2::new(18.0, 0.0);
        let p2 = Params { relay_backhaul_ns: 100.0, ..p.clone() };
        let with_backhaul = s.rtt_floor_ns(&p2);
        // 回传单向 100ns，往返计入 200ns。
        assert!((with_backhaul - base - 200.0).abs() < 1e-9);
    }
}
