//! 主机端演示：不依赖 Yew，直接跑典型诚实/攻击场景与门限扫描。
//! 运行：`cargo run -p db-core --example demo`

use db_core::{
    normal_cdf, run_authentication, threshold_sweep, Params, RelayMode, Rng, SampleSet, Scenario,
    Vec2,
};

fn section(title: &str) {
    println!("\n=== {title} ===");
}

fn main() {
    let params = Params::default();
    println!("默认参数：N={} 轮, RTT 门限 {:.1} ns, 容错 {} 轮",
        params.rounds, params.rtt_threshold_ns, params.max_fail_rounds);
    println!("钥匙处理 {:.0}±{:.0} ns，测量噪声 σ={:.0} ns，误码率 {:.0e}",
        params.key_proc_mean_ns, params.key_proc_std_ns,
        params.rtt_noise_std_ns, params.bit_error_rate);

    // 场景 1：合法钥匙在车旁 1.5 m，直连。
    let honest = Scenario {
        vehicle: Vec2::new(10.0, 7.0),
        key: Vec2::new(11.5, 7.0),
        relay_a: Vec2::new(5.0, 7.0),
        relay_b: Vec2::new(15.0, 7.0),
        relay_count: 0,
        relay_mode: RelayMode::Bypass,
    };

    // 场景 2：经典双中继盗窃——钥匙在 20 m 外屋内，两台中继贴车贴门。
    let attack = Scenario {
        vehicle: Vec2::new(2.0, 7.0),
        key: Vec2::new(22.0, 7.0),
        relay_a: Vec2::new(3.5, 7.0),
        relay_b: Vec2::new(20.5, 7.0),
        relay_count: 2,
        relay_mode: RelayMode::AnalogAmplify,
    };

    section("单次认证对照（种子 42）");
    for (name, s) in [("合法近距", &honest), ("中继攻击", &attack)] {
        let rep = run_authentication(s, &params, &mut Rng::new(42));
        println!(
            "{name}: {} | RTT 均值 {:.1} ns（下界 {:.1}），超时 {} 轮，位错 {} 轮，等效距离 {:.2} m",
            if rep.accepted { "放行" } else { "拒绝" },
            rep.mean_rtt_ns, rep.floor_ns, rep.late_count, rep.bit_error_count,
            rep.measured_equivalent_m
        );
    }

    section("蒙特卡洛统计（各 2000 次）");
    let trials = 2000;
    let h_set = SampleSet::collect(&honest, &params, trials, 1000);
    let a_set = SampleSet::collect(&attack, &params, trials, 2000);
    let frr = h_set.reject_rate_at(params.rtt_threshold_ns, params.max_fail_rounds);
    let far = 1.0 - a_set.reject_rate_at(params.rtt_threshold_ns, params.max_fail_rounds);
    println!("默认门限 {:.0} ns 下：FRR = {:.3}%，FAR = {:.3}%",
        params.rtt_threshold_ns, frr * 100.0, far * 100.0);

    let hs = h_set.rtt_summary();
    let as_ = a_set.rtt_summary();
    println!("诚实 RTT 均值 {:.1}±{:.1} ns（p99 {:.1}），攻击 RTT 均值 {:.1}±{:.1} ns（下界 {:.1}）",
        hs.mean_ns, hs.std_ns, hs.p99_ns, as_.mean_ns, as_.std_ns, as_.min_floor_ns);

    section("门限扫描与取舍");
    let sweep = threshold_sweep(&h_set, &a_set, &params, 120, 0.01);
    println!("EER 工作点：门限 {:.1} ns，FRR = {:.2}%，FAR = {:.2}%",
        sweep.eer.threshold_ns, sweep.eer.frr * 100.0, sweep.eer.far * 100.0);
    println!("安全优先推荐（FRR ≤ 1%）：门限 {:.1} ns，FRR = {:.2}%，FAR = {:.3}%",
        sweep.recommended.threshold_ns, sweep.recommended.frr * 100.0,
        sweep.recommended.far * 100.0);

    println!("\n门限(ns)  等效距离(m)   FRR       FAR");
    for p in sweep.points.iter().step_by(10) {
        let d = Params::equivalent_distance_m(p.threshold_ns, params.key_proc_mean_ns);
        println!("{:7.1}  {:9.2}  {:6.2}%  {:7.3}%",
            p.threshold_ns, d, p.frr * 100.0, p.far * 100.0);
    }

    section("解析对照：高斯假设下单轮超时概率");
    let z = (params.rtt_threshold_ns - hs.mean_ns) / hs.std_ns.max(1e-9);
    println!("诚实场景 z={z:.2}，预测单轮超时概率 {:.4}%",
        (1.0 - normal_cdf(z)) * 100.0);
}
