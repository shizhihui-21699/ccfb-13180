//! 距离边界协议的单轮与单次认证仿真。
//!
//! 判定逻辑（Hancke–Kuhn 风格）：
//! 1. 车端与钥匙预先共享秘密，快速阶段进行 N 轮挑战—应答；
//! 2. 车端对每轮测量往返时延 RTT，超过门限记一次“超时”；
//! 3. 应答比特经信道可能误码，错误记一次“位错”；
//! 4. 超时与位错合并为失败轮数，超过 `max_fail_rounds` 即拒绝放行。
//!
//! 密码学应答本身假定安全（现代 UWB 标准如 CCC Digital Key / FiRa 均已覆盖），
//! 本仿真聚焦“距离”这一维：中继攻击无法密码学伪装，只能转发真实钥匙，
//! 而转发引入的绕路与处理时延必然体现在 RTT 上。

use crate::model::{Params, Scenario};
use crate::rng::Rng;

/// 单轮快速交换的测量记录。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoundTrace {
    /// 轮次序号（0 起）。
    pub index: usize,
    /// 实测往返时延（ns）。
    pub rtt_ns: f64,
    /// 本轮 RTT 的确定性下界（ns，无抖动），用于 UI 对照。
    pub floor_ns: f64,
    /// 是否超过门限。
    pub late: bool,
    /// 应答比特是否误码。
    pub bit_wrong: bool,
}

/// 单次认证的完整报告。
#[derive(Clone, Debug, PartialEq)]
pub struct AuthReport {
    pub accepted: bool,
    pub rounds: Vec<RoundTrace>,
    pub late_count: usize,
    pub bit_error_count: usize,
    pub fail_count: usize,
    /// 实测 RTT 均值（ns）。
    pub mean_rtt_ns: f64,
    /// 实测 RTT 最大值（ns）。
    pub max_rtt_ns: f64,
    /// 场景确定性 RTT 下界（ns）。
    pub floor_ns: f64,
    /// 门限等效距离（m）：门限扣除钥匙处理后按 2d/c 折算。
    pub threshold_equivalent_m: f64,
    /// 实测均值对应的等效距离（m）。
    pub measured_equivalent_m: f64,
}

/// 运行一次完整认证。
///
/// * `scenario`：当前布点与中继配置；
/// * `params`：协议与物理参数；
/// * `rng`：随机源（传入不同种子得到不同噪声实现）。
pub fn run_authentication(scenario: &Scenario, params: &Params, rng: &mut Rng) -> AuthReport {
    let legs = scenario.challenge_legs(params);
    // 单程传播时延是确定性的几何量。
    let one_way_propagation_ns: f64 = legs.iter().map(|l| l.one_way_ns()).sum();
    let floor_ns = scenario.rtt_floor_ns(params);

    let mut rounds = Vec::with_capacity(params.rounds);
    let mut late_count = 0usize;
    let mut bit_error_count = 0usize;
    let mut rtt_sum = 0.0;
    let mut max_rtt = f64::MIN;

    for index in 0..params.rounds {
        // 钥匙快速应答处理：独立高斯抖动。
        let key_proc = rng.normal(params.key_proc_mean_ns, params.key_proc_std_ns);
        // 中继转发处理：去程每台一次、回程每台一次（逐跳采样，保留各自抖动）。
        let mut relay_proc = 0.0;
        for _ in 0..2 {
            for leg in &legs {
                if leg.proc_mean_ns > 0.0 {
                    relay_proc += rng.normal(leg.proc_mean_ns, leg.proc_std_ns);
                }
            }
        }
        // 车端测量噪声。
        let measure_noise = rng.normal(0.0, params.rtt_noise_std_ns);

        let rtt_ns = 2.0 * one_way_propagation_ns + key_proc + relay_proc + measure_noise;
        let late = rtt_ns > params.rtt_threshold_ns;
        let bit_wrong = rng.bernoulli(params.bit_error_rate);

        if late {
            late_count += 1;
        }
        if bit_wrong {
            bit_error_count += 1;
        }
        rtt_sum += rtt_ns;
        if rtt_ns > max_rtt {
            max_rtt = rtt_ns;
        }

        rounds.push(RoundTrace { index, rtt_ns, floor_ns, late, bit_wrong });
    }

    let fail_count = late_count + bit_error_count;
    let accepted = fail_count <= params.max_fail_rounds;
    let mean_rtt_ns = rtt_sum / params.rounds as f64;

    AuthReport {
        accepted,
        rounds,
        late_count,
        bit_error_count,
        fail_count,
        mean_rtt_ns,
        max_rtt_ns: max_rtt,
        floor_ns,
        threshold_equivalent_m: Params::equivalent_distance_m(
            params.rtt_threshold_ns,
            params.key_proc_mean_ns,
        ),
        measured_equivalent_m: Params::equivalent_distance_m(
            mean_rtt_ns,
            params.key_proc_mean_ns,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geo::Vec2;
    use crate::model::RelayMode;

    fn close_legit() -> (Scenario, Params) {
        let p = Params::default();
        let s = Scenario {
            vehicle: Vec2::new(10.0, 7.0),
            key: Vec2::new(11.5, 7.0),
            relay_a: Vec2::new(5.0, 7.0),
            relay_b: Vec2::new(15.0, 7.0),
            relay_count: 0,
            relay_mode: RelayMode::Bypass,
        };
        (s, p)
    }

    #[test]
    fn legit_nearby_key_accepted() {
        let (s, p) = close_legit();
        let mut accepted = 0;
        for seed in 0..50 {
            let rep = run_authentication(&s, &p, &mut Rng::new(seed));
            if rep.accepted {
                accepted += 1;
            }
        }
        // 1.5m 直连、门限 150ns：几何下界 ≈ 90ns，50 个种子应几乎全部放行。
        assert!(accepted >= 49, "accepted {accepted}/50");
    }

    #[test]
    fn far_direct_key_rejected_by_rtt() {
        let (mut s, p) = close_legit();
        s.key = Vec2::new(30.0, 7.0); // 20m 直连，几何 RTT ≈ 213ns > 门限
        let rep = run_authentication(&s, &p, &mut Rng::new(1));
        assert_eq!(rep.late_count, p.rounds);
        assert!(!rep.accepted);
    }

    #[test]
    fn classic_two_relay_attack_rejected() {
        // 钥匙在屋内 20m 外，两台中继把信号搬到车旁。
        let p = Params::default();
        let s = Scenario {
            vehicle: Vec2::new(2.0, 7.0),
            key: Vec2::new(22.0, 7.0),
            relay_a: Vec2::new(3.5, 7.0),   // 车旁
            relay_b: Vec2::new(20.5, 7.0),  // 屋门口
            relay_count: 2,
            relay_mode: RelayMode::AnalogAmplify,
        };
        let rep = run_authentication(&s, &p, &mut Rng::new(3));
        assert!(rep.floor_ns > p.rtt_threshold_ns);
        assert!(!rep.accepted);
        assert!(rep.late_count > 0);
    }

    #[test]
    fn relay_cannot_reduce_rtt_below_direct_floor() {
        // 物理不变量：任何转发路径的 RTT 下界都不低于直连 2d/c。
        let p = Params::default();
        let s = Scenario {
            vehicle: Vec2::new(0.0, 0.0),
            key: Vec2::new(10.0, 0.0),
            relay_a: Vec2::new(3.0, 4.0),
            relay_b: Vec2::new(7.0, -4.0),
            relay_count: 2,
            relay_mode: RelayMode::AnalogAmplify,
        };
        let direct_floor = Params::geometric_floor_ns(10.0) + p.key_proc_mean_ns;
        assert!(s.rtt_floor_ns(&p) >= direct_floor);
    }
}
