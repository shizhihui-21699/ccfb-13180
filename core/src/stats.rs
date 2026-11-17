//! 蒙特卡洛批量统计：误拒率 FRR、漏放率 FAR、门限扫描与等错误率（EER）。
//!
//! 术语：
//! * **FRR（False Reject Rate，误拒率）**：合法钥匙（诚实场景）被拒绝的比例；
//! * **FAR（False Accept Rate，漏放率/误接受率）**：中继攻击场景被放行的比例。
//!
//! 二者随门限此消彼长：门限收紧 → FRR 升、FAR 降；门限放松则相反。
//! 扫描给出整条权衡曲线，并自动标出 EER 与安全优先的推荐工作点。

use crate::model::{Params, Scenario};
use crate::protocol::run_authentication;
use crate::rng::Rng;

/// 单次认证的精简样本：保存逐轮 RTT（已排序）与位错轮号，供任意门限快速复算。
#[derive(Clone, Debug, PartialEq)]
pub struct Sample {
    /// 升序排列的各轮 RTT（ns）。
    pub rtts_sorted: Vec<f64>,
    /// 发生应答位错误的轮数。
    pub bit_errors: usize,
    /// 本次认证的确定性 RTT 下界（ns）。
    pub floor_ns: f64,
}

impl Sample {
    /// 在给定门限下复算失败轮数（超时轮用二分查找计数）。
    pub fn fail_count_at(&self, threshold_ns: f64) -> usize {
        let late = self
            .rtts_sorted
            .partition_point(|rtt| *rtt <= threshold_ns);
        let late = self.rtts_sorted.len() - late;
        late + self.bit_errors
    }

    /// 在给定门限与容错轮数下是否放行。
    pub fn accepted_at(&self, threshold_ns: f64, max_fail_rounds: usize) -> bool {
        self.fail_count_at(threshold_ns) <= max_fail_rounds
    }
}

/// 一类场景（诚实 / 攻击）的样本集合。
#[derive(Clone, Debug, PartialEq)]
pub struct SampleSet {
    pub samples: Vec<Sample>,
}

impl SampleSet {
    /// 对一个场景跑 `trials` 次完整认证并收集样本。
    /// 种子按 `base_seed` 顺序派生，保证两次调用结果可复现。
    pub fn collect(scenario: &Scenario, params: &Params, trials: usize, base_seed: u64) -> Self {
        let mut rng = Rng::new(base_seed);
        let samples = (0..trials)
            .map(|_| {
                let rep = run_authentication(scenario, params, &mut rng);
                let mut rtts: Vec<f64> = rep.rounds.iter().map(|r| r.rtt_ns).collect();
                rtts.sort_by(|a, b| a.partial_cmp(b).unwrap());
                Sample {
                    rtts_sorted: rtts,
                    bit_errors: rep.bit_error_count,
                    floor_ns: rep.floor_ns,
                }
            })
            .collect();
        Self { samples }
    }

    /// 指定门限下的拒绝率（0..1）。
    pub fn reject_rate_at(&self, threshold_ns: f64, max_fail_rounds: usize) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let rejected = self
            .samples
            .iter()
            .filter(|s| !s.accepted_at(threshold_ns, max_fail_rounds))
            .count();
        rejected as f64 / self.samples.len() as f64
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// 所有样本 RTT 均值的均值/分位数等汇总（用于界面展示分布）。
    pub fn rtt_summary(&self) -> RttSummary {
        let mut means: Vec<f64> = self
            .samples
            .iter()
            .map(|s| s.rtts_sorted.iter().sum::<f64>() / s.rtts_sorted.len() as f64)
            .collect();
        means.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let n = means.len();
        let avg = means.iter().sum::<f64>() / n.max(1) as f64;
        let std = (means
            .iter()
            .map(|x| (x - avg).powi(2))
            .sum::<f64>()
            / n.max(1) as f64)
            .sqrt();
        RttSummary {
            mean_ns: avg,
            std_ns: std,
            p01_ns: percentile_sorted(&means, 0.01),
            p50_ns: percentile_sorted(&means, 0.50),
            p99_ns: percentile_sorted(&means, 0.99),
            min_floor_ns: self.samples.iter().map(|s| s.floor_ns).fold(f64::INFINITY, f64::min),
        }
    }
}

/// RTT 分布汇总（ns）。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RttSummary {
    pub mean_ns: f64,
    pub std_ns: f64,
    pub p01_ns: f64,
    pub p50_ns: f64,
    pub p99_ns: f64,
    /// 该场景的确定性 RTT 下界。
    pub min_floor_ns: f64,
}

fn percentile_sorted(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let idx = ((sorted.len() as f64 - 1.0) * q).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

/// 门限扫描曲线上的一个点。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SweepPoint {
    pub threshold_ns: f64,
    /// 诚实场景误拒率。
    pub frr: f64,
    /// 攻击场景漏放率。
    pub far: f64,
}

/// 扫描结果。
#[derive(Clone, Debug, PartialEq)]
pub struct SweepResult {
    pub points: Vec<SweepPoint>,
    /// 等错误率工作点（FRR≈FAR 处，线性插值）。
    pub eer: OperatingPoint,
    /// 安全优先推荐工作点（满足 FRR 目标的最小门限）。
    pub recommended: OperatingPoint,
    /// 推荐点采用的 FRR 目标。
    pub frr_target: f64,
}

/// 一个具体工作点。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OperatingPoint {
    pub threshold_ns: f64,
    pub frr: f64,
    pub far: f64,
}

/// 对诚实/攻击两组样本做门限扫描。
///
/// 网格自动覆盖诚实分布 0.5 分位到攻击分布 99.5 分位，并纳入两个场景的
/// 物理下界与当前参数门限，共 `grid` 个点（端点外补两点保证曲线完整）。
pub fn threshold_sweep(
    honest: &SampleSet,
    attack: &SampleSet,
    params: &Params,
    grid: usize,
    frr_target: f64,
) -> SweepResult {
    let h_summary = honest.rtt_summary();
    let a_summary = attack.rtt_summary();

    let mut lo = (h_summary.p50_ns - 30.0).min(h_summary.min_floor_ns);
    let mut hi = (a_summary.p99_ns + 30.0).max(a_summary.min_floor_ns);
    if hi.partial_cmp(&lo) != Some(std::cmp::Ordering::Greater) {
        hi = lo + 100.0;
    }
    // 端点外扩，保证曲线能走到 0% 和 100% 区域。
    lo = (lo - 50.0).max(1.0);
    hi += 50.0;

    let n = grid.max(10);
    let mut thresholds: Vec<f64> = (0..n)
        .map(|i| lo + (hi - lo) * i as f64 / (n - 1) as f64)
        .collect();
    // 关键参考门限也纳入网格（去重排序）。
    thresholds.push(params.rtt_threshold_ns);
    thresholds.push(h_summary.min_floor_ns);
    thresholds.push(a_summary.min_floor_ns);
    thresholds.sort_by(|a, b| a.partial_cmp(b).unwrap());
    thresholds.dedup_by(|a, b| (*a - *b).abs() < 1e-9);

    let points: Vec<SweepPoint> = thresholds
        .iter()
        .map(|t| SweepPoint {
            threshold_ns: *t,
            frr: honest.reject_rate_at(*t, params.max_fail_rounds),
            far: 1.0 - attack.reject_rate_at(*t, params.max_fail_rounds),
        })
        .collect();

    let eer = find_eer(&points);
    let recommended = find_recommended(&points, frr_target);

    SweepResult { points, eer, recommended, frr_target }
}

/// FRR 与 FAR 曲线交点（等错误率），在相邻扫描点间线性插值。
fn find_eer(points: &[SweepPoint]) -> OperatingPoint {
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        // FRR 随门限单调降，FAR 单调升；符号改变处即交点。
        if (a.frr - a.far) * (b.frr - b.far) <= 0.0 && (b.frr - a.frr).abs() > 0.0 {
            let denom = (a.frr - a.far) - (b.frr - b.far);
            let w = if denom.abs() < 1e-12 { 0.5 } else { (a.frr - a.far) / denom };
            return OperatingPoint {
                threshold_ns: a.threshold_ns + w * (b.threshold_ns - a.threshold_ns),
                frr: a.frr + w * (b.frr - a.frr),
                far: a.far + w * (b.far - a.far),
            };
        }
    }
    // 无交点：返回差距最小的点。
    let best = points
        .iter()
        .copied()
        .min_by(|a, b| {
            (a.frr - a.far)
                .abs()
                .partial_cmp(&(b.frr - b.far).abs())
                .unwrap()
        })
        .unwrap();
    OperatingPoint { threshold_ns: best.threshold_ns, frr: best.frr, far: best.far }
}

/// 安全优先：取满足 FRR ≤ 目标的最小门限（门限越小，攻击面越窄）。
/// 若所有扫描点都达不到目标，退而取 FRR 最小的点。
fn find_recommended(points: &[SweepPoint], frr_target: f64) -> OperatingPoint {
    let feasible = points.iter().copied().find(|p| p.frr <= frr_target);
    match feasible {
        Some(p) => OperatingPoint {
            threshold_ns: p.threshold_ns,
            frr: p.frr,
            far: p.far,
        },
        None => {
            let p = points
                .iter()
                .copied()
                .min_by(|a, b| a.frr.partial_cmp(&b.frr).unwrap())
                .unwrap();
            OperatingPoint { threshold_ns: p.threshold_ns, frr: p.frr, far: p.far }
        }
    }
}

/// 标准正态分布累积函数（Abramowitz–Stegun 7.1.26 近似，误差 < 7.5e-8），
/// 供界面做解析对照（预测单轮超时概率）。
pub fn normal_cdf(x: f64) -> f64 {
    // erf 近似系数（Abramowitz–Stegun 7.1.26，|误差| < 7.5e-8）。
    // Φ(x) = ½(1 + erf(x/√2))。
    let y = x / std::f64::consts::SQRT_2;
    let t = 1.0 / (1.0 + 0.3275911 * y.abs());
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let poly = (((((a5 * t + a4) * t + a3) * t + a2) * t + a1) * t) * (-y * y).exp();
    let erf = if y < 0.0 { poly - 1.0 } else { 1.0 - poly };
    0.5 * (1.0 + erf)
}

/// 解析估计单轮超时概率 P(RTT > threshold)，假设 RTT 近似高斯。
pub fn analytical_late_prob(
    threshold_ns: f64,
    floor_ns: f64,
    total_std_ns: f64,
) -> f64 {
    if total_std_ns <= 0.0 {
        return if floor_ns > threshold_ns { 1.0 } else { 0.0 };
    }
    1.0 - normal_cdf((threshold_ns - floor_ns) / total_std_ns)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geo::Vec2;
    use crate::model::RelayMode;

    fn honest_scenario() -> Scenario {
        Scenario {
            vehicle: Vec2::new(10.0, 7.0),
            key: Vec2::new(11.5, 7.0),
            relay_a: Vec2::new(5.0, 7.0),
            relay_b: Vec2::new(15.0, 7.0),
            relay_count: 0,
            relay_mode: RelayMode::Bypass,
        }
    }

    fn attack_scenario() -> Scenario {
        Scenario {
            vehicle: Vec2::new(2.0, 7.0),
            key: Vec2::new(22.0, 7.0),
            relay_a: Vec2::new(3.5, 7.0),
            relay_b: Vec2::new(20.5, 7.0),
            relay_count: 2,
            relay_mode: RelayMode::AnalogAmplify,
        }
    }

    #[test]
    fn sweep_rates_are_monotonic() {
        let p = Params::default();
        let honest = SampleSet::collect(&honest_scenario(), &p, 200, 1000);
        let attack = SampleSet::collect(&attack_scenario(), &p, 200, 2000);
        let sweep = threshold_sweep(&honest, &attack, &p, 80, 0.01);

        for w in sweep.points.windows(2) {
            // 门限增大：FRR 不增，FAR 不降。
            assert!(w[1].frr <= w[0].frr + 1e-12);
            assert!(w[1].far >= w[0].far - 1e-12);
        }
        // 两端应逼近极端值。
        assert!(sweep.points.first().unwrap().far < 0.05);
        assert!(sweep.points.last().unwrap().frr < 0.05);
    }

    #[test]
    fn well_separated_scenarios_give_low_eer() {
        let p = Params::default();
        let honest = SampleSet::collect(&honest_scenario(), &p, 400, 1000);
        let attack = SampleSet::collect(&attack_scenario(), &p, 400, 2000);
        let sweep = threshold_sweep(&honest, &attack, &p, 100, 0.01);
        // 诚实 RTT ≈ 90ns、攻击下界 > 300ns，分布几乎不重叠。
        assert!(sweep.eer.frr < 0.02, "eer {:?}", sweep.eer);
        assert!(sweep.recommended.frr <= 0.01 + 1e-12);
        assert!(sweep.recommended.far < 0.01);
    }

    #[test]
    fn sample_replay_matches_direct_evaluation() {
        let p = Params::default();
        let set = SampleSet::collect(&honest_scenario(), &p, 50, 777);
        let direct = set.reject_rate_at(p.rtt_threshold_ns, p.max_fail_rounds);
        let replay = set
            .samples
            .iter()
            .filter(|s| !s.accepted_at(p.rtt_threshold_ns, p.max_fail_rounds))
            .count() as f64
            / set.len() as f64;
        assert!((direct - replay).abs() < 1e-12);
    }

    #[test]
    fn normal_cdf_sanity() {
        assert!((normal_cdf(0.0) - 0.5).abs() < 1e-6);
        assert!(normal_cdf(3.0) > 0.998 && normal_cdf(3.0) < 0.999);
        assert!((normal_cdf(-1.96) - 0.025).abs() < 1e-3);
    }
}
