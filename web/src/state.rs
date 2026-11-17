//! Yew 界面的应用状态与仿真编排（纯数据 + 纯函数，便于测试与推理）。

use db_core::{
    run_authentication, threshold_sweep, AuthReport, Params, RelayMode, Rng, SampleSet, Scenario,
    SweepResult, Vec2,
};

/// 场地逻辑尺寸（米）。
pub const FIELD_W: f64 = 20.0;
pub const FIELD_H: f64 = 14.0;

/// 批量仿真试验次数档位。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrialCount {
    Low,
    Medium,
    High,
}

impl TrialCount {
    pub fn trials(self) -> usize {
        match self {
            TrialCount::Low => 200,
            TrialCount::Medium => 1000,
            TrialCount::High => 4000,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            TrialCount::Low => "200 次（快）",
            TrialCount::Medium => "1000 次",
            TrialCount::High => "4000 次（精）",
        }
    }
}

/// 可拖拽设备。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragKind {
    Vehicle,
    Key,
    RelayA,
    RelayB,
}

impl DragKind {
    pub fn label(self) -> &'static str {
        match self {
            DragKind::Vehicle => "车辆",
            DragKind::Key => "钥匙",
            DragKind::RelayA => "中继 A（车旁）",
            DragKind::RelayB => "中继 B（钥匙旁）",
        }
    }
}

/// 批量统计结果。
#[derive(Clone, Debug, PartialEq)]
pub struct BatchResult {
    pub trials: usize,
    pub frr: f64,
    pub far: f64,
    pub honest_mean_rtt: f64,
    pub honest_std_rtt: f64,
    pub attack_mean_rtt: f64,
    pub attack_floor: f64,
    pub sweep: SweepResult,
}

/// 整个演示的可变状态。
#[derive(Clone, Debug, PartialEq)]
pub struct AppState {
    pub vehicle: Vec2,
    pub key: Vec2,
    pub relay_a: Vec2,
    pub relay_b: Vec2,
    /// 中继台数：0 / 1 / 2。
    pub relay_count: usize,
    pub relay_mode: RelayMode,

    pub params: Params,
    /// 诚实统计使用的合法距离（米）：FRR 应在“真实用户正常使用”的距离上评估。
    pub honest_distance_m: f64,
    pub trial_count: TrialCount,
    pub seed: u64,
}

impl Default for AppState {
    fn default() -> Self {
        let params = Params::default();
        Self {
            vehicle: Vec2::new(2.0, 7.0),
            key: Vec2::new(22.0_f64.min(FIELD_W - 1.0), 7.0),
            relay_a: Vec2::new(3.5, 7.0),
            relay_b: Vec2::new(17.5, 7.0),
            relay_count: 2,
            relay_mode: RelayMode::AnalogAmplify,
            params,
            honest_distance_m: 1.5,
            trial_count: TrialCount::Medium,
            seed: 42,
        }
    }
}

impl AppState {
    /// 当前布点对应的攻击/当前场景。
    pub fn scenario(&self) -> Scenario {
        Scenario {
            vehicle: self.vehicle,
            key: self.key,
            relay_a: self.relay_a,
            relay_b: self.relay_b,
            relay_count: self.relay_count,
            relay_mode: self.relay_mode,
        }
    }

    /// 诚实基准场景：钥匙在车辆正前方 `honest_distance_m` 米，直连无中继。
    pub fn honest_scenario(&self) -> Scenario {
        let mut s = self.scenario();
        s.key = Vec2::new(
            (self.vehicle.x + self.honest_distance_m).min(FIELD_W),
            self.vehicle.y,
        );
        s.relay_count = 0;
        s.relay_mode = RelayMode::Bypass;
        s
    }

    /// 移动某个设备并限制在场地内。
    pub fn move_device(&mut self, kind: DragKind, pos: Vec2) {
        let p = pos.clamp(FIELD_W, FIELD_H);
        match kind {
            DragKind::Vehicle => self.vehicle = p,
            DragKind::Key => self.key = p,
            DragKind::RelayA => self.relay_a = p,
            DragKind::RelayB => self.relay_b = p,
        }
    }

    /// 单次认证（攻击/当前布点）。
    pub fn run_single(&self) -> AuthReport {
        run_authentication(&self.scenario(), &self.params, &mut Rng::new(self.seed))
    }

    /// 批量统计 + 门限扫描。
    pub fn run_batch(&self) -> BatchResult {
        let trials = self.trial_count.trials();
        let honest = SampleSet::collect(
            &self.honest_scenario(),
            &self.params,
            trials,
            self.seed.wrapping_mul(1_000_003) | 1,
        );
        let attack = SampleSet::collect(
            &self.scenario(),
            &self.params,
            trials,
            self.seed.wrapping_mul(1_000_033) | 1,
        );
        let frr = honest.reject_rate_at(self.params.rtt_threshold_ns, self.params.max_fail_rounds);
        let far =
            1.0 - attack.reject_rate_at(self.params.rtt_threshold_ns, self.params.max_fail_rounds);
        let hs = honest.rtt_summary();
        let asu = attack.rtt_summary();
        let sweep = threshold_sweep(&honest, &attack, &self.params, 120, 0.01);
        BatchResult {
            trials,
            frr,
            far,
            honest_mean_rtt: hs.mean_ns,
            honest_std_rtt: hs.std_ns,
            attack_mean_rtt: asu.mean_ns,
            attack_floor: asu.min_floor_ns,
            sweep,
        }
    }

    /// 一键布防经典双中继盗窃场景。
    pub fn preset_classic_attack(&mut self) {
        self.vehicle = Vec2::new(2.0, 7.0);
        self.key = Vec2::new(19.0, 7.0);
        self.relay_a = Vec2::new(3.5, 7.0);
        self.relay_b = Vec2::new(17.5, 7.0);
        self.relay_count = 2;
        self.relay_mode = RelayMode::AnalogAmplify;
    }

    /// 合法近距场景（无中继，钥匙在车旁）。
    pub fn preset_legit(&mut self) {
        self.vehicle = Vec2::new(10.0, 7.0);
        self.key = Vec2::new(11.5, 7.0);
        self.relay_a = Vec2::new(6.0, 4.0);
        self.relay_b = Vec2::new(14.0, 10.0);
        self.relay_count = 0;
        self.relay_mode = RelayMode::Bypass;
    }

    /// 临界对抗：中继贴近车—钥匙直连路径、转发时延调到很小，
    /// 使攻击路径 RTT 下界（约 143 ns）逼近门限 150 ns，单轮超时概率仅几个百分点，
    /// 但 N=32 轮 + 容错 2 轮仍把整体漏放率压到约一成——展示多轮投票形成的
    /// “判定悬崖”：攻击 RTT 只要比诚实分布高几十 ns，EER 就为 0，
    /// 攻击只能在门限悬崖边缘拿到非零漏放率，而此时门限只需再收紧少许即可清零。
    pub fn preset_borderline(&mut self) {
        self.vehicle = Vec2::new(3.0, 7.0);
        self.key = Vec2::new(8.0, 7.0);
        self.relay_a = Vec2::new(4.3, 6.6);
        self.relay_b = Vec2::new(6.7, 7.4);
        self.relay_count = 2;
        self.relay_mode = RelayMode::AnalogAmplify;
        self.params.relay_proc_mean_ns = 7.0;
        self.params.relay_proc_std_ns = 2.5;
        self.params.relay_backhaul_ns = 0.0;
    }
}
