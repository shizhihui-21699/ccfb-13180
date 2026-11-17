//! 确定性伪随机数发生器（SplitMix64 + Box–Muller）。
//!
//! 核心库不依赖 `rand`，保证主机与 WASM 上结果完全可复现；
//! 同一 seed、同一调用序列产生同一组样本，便于回归测试与演示复现。

/// SplitMix64 —— 轻量、周期 2^64、统计性质足够仿真用途。
#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
    /// 高斯缓存对中的第二个值。
    spare_gaussian: Option<f64>,
}

impl Rng {
    /// 用 64 位种子构造。
    pub fn new(seed: u64) -> Self {
        Self { state: seed, spare_gaussian: None }
    }

    /// 下一个 u64。
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// 区间 [0, 1) 上的均匀分布。
    pub fn uniform01(&mut self) -> f64 {
        // 右移 11 位得到 53 位有效数字的尾数。
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// 标准正态 N(0, 1)，Box–Muller 变换（成对生成，缓存一个）。
    pub fn gaussian(&mut self) -> f64 {
        if let Some(z) = self.spare_gaussian.take() {
            return z;
        }
        // uniform01 ∈ [0,1)，用 1-u 避免 log(0)。
        let u1 = 1.0 - self.uniform01();
        let u2 = self.uniform01();
        let r = (-2.0 * u1.ln()).sqrt();
        let theta = 2.0 * std::f64::consts::PI * u2;
        let z0 = r * theta.cos();
        let z1 = r * theta.sin();
        self.spare_gaussian = Some(z1);
        z0
    }

    /// 一般正态 N(mean, std^2)；std <= 0 时恒返回 mean。
    pub fn normal(&mut self, mean: f64, std: f64) -> f64 {
        if std <= 0.0 { mean } else { mean + std * self.gaussian() }
    }

    /// 以概率 p 返回 true 的伯努利试验。
    pub fn bernoulli(&mut self, p: f64) -> bool {
        if p <= 0.0 { false } else if p >= 1.0 { true } else { self.uniform01() < p }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_same_seed() {
        let a: Vec<u64> = (0..100).map(|_| Rng::new(42).next_u64()).collect();
        let b: Vec<u64> = (0..100).map(|_| Rng::new(42).next_u64()).collect();
        assert_eq!(a, b);
        let c: Vec<u64> = (0..100).map(|_| Rng::new(43).next_u64()).collect();
        assert_ne!(a, c);
    }

    #[test]
    fn gaussian_moments() {
        let mut rng = Rng::new(7);
        let n = 200_000usize;
        let mean = 80.0;
        let std = 4.0;
        let samples: Vec<f64> = (0..n).map(|_| rng.normal(mean, std)).collect();
        let m = samples.iter().sum::<f64>() / n as f64;
        let v = samples.iter().map(|x| (x - m).powi(2)).sum::<f64>() / n as f64;
        assert!((m - mean).abs() < 0.1, "mean {m}");
        assert!((v.sqrt() - std).abs() < 0.1, "std {}", v.sqrt());
    }

    #[test]
    fn bernoulli_rate() {
        let mut rng = Rng::new(99);
        let n = 100_000usize;
        let hits = (0..n).filter(|_| rng.bernoulli(0.3)).count();
        let rate = hits as f64 / n as f64;
        assert!((rate - 0.3).abs() < 0.02);
    }
}
