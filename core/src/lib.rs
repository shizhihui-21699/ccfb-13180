//! # db-core —— 距离边界认证仿真核心
//!
//! 面向无钥匙进入（PEPS / 数字钥匙）中继攻击防御的可复现仿真库：
//!
//! * [`geo`]：二维布点与路径几何；
//! * [`model`]：传播/处理/中继时延模型与仿真参数 [`model::Params`]；
//! * [`protocol`]：Hancke–Kuhn 式快速挑战—应答的单次认证判定；
//! * [`stats`]：蒙特卡洛 FRR/FAR 统计、门限扫描、EER 与推荐工作点；
//! * [`rng`]：确定性 PRNG，主机与 WASM 结果一致。
//!
//! ## 物理背景
//!
//! 无线电波传播速度受光速约束，距离 d 的单向传播时延为 d/c。
//! 车端测量挑战—应答往返时延 RTT，任何中继转发都只能在真实路径上
//! **增加**空中距离（三角不等式）与设备处理时延，无法缩短。
//! 因此把 RTT 门限换算成等效距离，即可把“钥匙是否真的在车旁”
//! 变成一个可测量、可标定的判定。

pub mod geo;
pub mod model;
pub mod protocol;
pub mod rng;
pub mod stats;

pub use geo::Vec2;
pub use model::{Params, PathLeg, RelayMode, Scenario, SPEED_OF_LIGHT};
pub use protocol::{run_authentication, AuthReport, RoundTrace};
pub use rng::Rng;
pub use stats::{
    analytical_late_prob, normal_cdf, threshold_sweep, OperatingPoint, RttSummary, Sample,
    SampleSet, SweepPoint, SweepResult,
};
