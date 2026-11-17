//! 结果区：单次认证判定卡、逐轮 RTT 表、批量 FRR/FAR 指标与取舍说明。

use db_core::{AuthReport, Params};
use yew::prelude::*;

// 批处理结果类型定义在 state 模块。
use crate::state::BatchResult;

#[derive(Properties, PartialEq)]
pub struct SingleProps {
    pub report: AuthReport,
    pub params: Params,
}

#[function_component(SingleResult)]
pub fn single_result(props: &SingleProps) -> Html {
    let r = &props.report;
    let verdict_class = if r.accepted { "verdict accept" } else { "verdict reject" };
    let verdict_text = if r.accepted { "✅ 放行" } else { "⛔ 拒绝" };

    let shown = r.rounds.len().min(12);

    html! {
        <div class="card">
            <div class="card-head">
                <h3>{"单次认证（当前布点）"}</h3>
                <span class={verdict_class}>{ verdict_text }</span>
            </div>
            <div class="metric-grid">
                <Metric label="RTT 均值" value={format!("{:.1} ns", r.mean_rtt_ns)} />
                <Metric label="RTT 最大值" value={format!("{:.1} ns", r.max_rtt_ns)} />
                <Metric label="确定性下界" value={format!("{:.1} ns", r.floor_ns)} />
                <Metric label="超时轮数" value={format!("{} / {}", r.late_count, props.params.rounds)}
                    warn={r.late_count > 0} />
                <Metric label="位错轮数" value={format!("{}", r.bit_error_count)}
                    warn={r.bit_error_count > 0} />
                <Metric label="失败合计 / 容错"
                    value={format!("{} / {}", r.fail_count, props.params.max_fail_rounds)}
                    warn={r.fail_count > props.params.max_fail_rounds} />
                <Metric label="门限等效距离" value={format!("{:.2} m", r.threshold_equivalent_m)} />
                <Metric label="实测等效距离" value={format!("{:.2} m", r.measured_equivalent_m)}
                    warn={r.measured_equivalent_m > r.threshold_equivalent_m} />
            </div>
            <table class="round-table">
                <thead>
                    <tr>
                        <th>{"轮次"}</th><th>{"RTT (ns)"}</th><th>{"下界 (ns)"}</th>
                        <th>{"门限 (ns)"}</th><th>{"状态"}</th>
                    </tr>
                </thead>
                <tbody>
                    { for r.rounds.iter().take(shown).map(|rt| {
                        let status = match (rt.late, rt.bit_wrong) {
                            (true, _) => ("超时", "bad"),
                            (false, true) => ("位错", "warn"),
                            (false, false) => ("正常", "ok"),
                        };
                        html! {
                            <tr key={rt.index}>
                                <td>{ rt.index + 1 }</td>
                                <td>{ format!("{:.1}", rt.rtt_ns) }</td>
                                <td>{ format!("{:.1}", rt.floor_ns) }</td>
                                <td>{ format!("{:.1}", props.params.rtt_threshold_ns) }</td>
                                <td><span class={classes!("pill", status.1)}>{ status.0 }</span></td>
                            </tr>
                        }
                    })}
                </tbody>
            </table>
            if r.rounds.len() > shown {
                <p class="table-note">{ format!("仅展示前 {shown} 轮，共 {} 轮。", r.rounds.len()) }</p>
            }
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct MetricProps {
    label: String,
    value: String,
    #[prop_or_default]
    warn: bool,
}

#[function_component(Metric)]
fn metric(props: &MetricProps) -> Html {
    html! {
        <div class={classes!("metric", props.warn.then_some("metric-warn"))}>
            <span class="metric-label">{ props.label.clone() }</span>
            <span class="metric-value">{ props.value.clone() }</span>
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub struct BatchProps {
    pub batch: BatchResult,
    pub params: Params,
}

#[function_component(BatchStats)]
pub fn batch_stats(props: &BatchProps) -> Html {
    let b = &props.batch;
    let sweep = &b.sweep;

    html! {
        <div class="card">
            <div class="card-head">
                <h3>{ format!("蒙特卡洛统计（每场景 {} 次）", b.trials) }</h3>
            </div>
            <div class="metric-grid">
                <Metric label="误拒率 FRR（当前门限）"
                    value={format!("{:.3}%", b.frr * 100.0)}
                    warn={b.frr > 0.01} />
                <Metric label="漏放率 FAR（当前门限）"
                    value={format!("{:.3}%", b.far * 100.0)}
                    warn={b.far > 0.01} />
                <Metric label="诚实 RTT 均值"
                    value={format!("{:.1} ± {:.1} ns", b.honest_mean_rtt, b.honest_std_rtt)} />
                <Metric label="攻击 RTT 均值 / 下界"
                    value={format!("{:.1} / {:.1} ns", b.attack_mean_rtt, b.attack_floor)} />
            </div>

            <div class="op-table-wrap">
                <table class="op-table">
                    <thead>
                        <tr><th>{"工作点"}</th><th>{"门限"}</th><th>{"等效距离"}</th><th>{"FRR"}</th><th>{"FAR"}</th></tr>
                    </thead>
                    <tbody>
                        <tr>
                            <td>{"当前设置"}</td>
                            <td>{ format!("{:.1} ns", props.params.rtt_threshold_ns) }</td>
                            <td>{ format!("{:.2} m", Params::equivalent_distance_m(props.params.rtt_threshold_ns, props.params.key_proc_mean_ns)) }</td>
                            <td>{ format!("{:.3}%", b.frr * 100.0) }</td>
                            <td>{ format!("{:.3}%", b.far * 100.0) }</td>
                        </tr>
                        <tr>
                            <td>{"EER 等错误点"}</td>
                            <td>{ format!("{:.1} ns", sweep.eer.threshold_ns) }</td>
                            <td>{ format!("{:.2} m", Params::equivalent_distance_m(sweep.eer.threshold_ns, props.params.key_proc_mean_ns)) }</td>
                            <td>{ format!("{:.2}%", sweep.eer.frr * 100.0) }</td>
                            <td>{ format!("{:.2}%", sweep.eer.far * 100.0) }</td>
                        </tr>
                        <tr class="recommended-row">
                            <td>{"★ 安全优先推荐"}</td>
                            <td>{ format!("{:.1} ns", sweep.recommended.threshold_ns) }</td>
                            <td>{ format!("{:.2} m", Params::equivalent_distance_m(sweep.recommended.threshold_ns, props.params.key_proc_mean_ns)) }</td>
                            <td>{ format!("{:.2}%", sweep.recommended.frr * 100.0) }</td>
                            <td>{ format!("{:.3}%", sweep.recommended.far * 100.0) }</td>
                        </tr>
                    </tbody>
                </table>
            </div>
        </div>
    }
}
