//! FRR/FAR 随门限变化的权衡曲线（纯 SVG 手绘，无图表库依赖）。

use db_core::{Params, SweepResult};
use yew::prelude::*;

const W: f64 = 740.0;
const H: f64 = 320.0;
const M_L: f64 = 56.0;
const M_R: f64 = 24.0;
const M_T: f64 = 24.0;
const M_B: f64 = 52.0;

#[derive(Properties, PartialEq)]
pub struct ChartProps {
    pub sweep: SweepResult,
    pub current_threshold_ns: f64,
    pub key_proc_ns: f64,
}

#[function_component(SweepChart)]
pub fn sweep_chart(props: &ChartProps) -> Html {
    let sweep = &props.sweep;
    let pts = &sweep.points;
    let x_min = pts.first().map(|p| p.threshold_ns).unwrap_or(0.0);
    let x_max = pts.last().map(|p| p.threshold_ns).unwrap_or(1.0);

    let x = |t: f64| M_L + (t - x_min) / (x_max - x_min) * (W - M_L - M_R);
    let y = |rate: f64| M_T + (1.0 - rate) * (H - M_T - M_B);

    let line_path = |rate_fn: fn(&db_core::SweepPoint) -> f64| {
        pts.iter()
            .enumerate()
            .map(|(i, p)| {
                let px = x(p.threshold_ns);
                let py = y(rate_fn(p));
                format!("{}{px:.1},{py:.1}", if i == 0 { "M" } else { "L" })
            })
            .collect::<Vec<_>>()
            .join(" ")
    };
    let frr_path = line_path(|p| p.frr);
    let far_path = line_path(|p| p.far);

    let y_ticks: Vec<f64> = (0..=5).map(|i| i as f64 / 5.0).collect();
    let x_tick_count = 6usize;
    let x_ticks: Vec<f64> = (0..x_tick_count)
        .map(|i| x_min + (x_max - x_min) * i as f64 / (x_tick_count - 1) as f64)
        .collect();

    let cur_x = x(props.current_threshold_ns.clamp(x_min, x_max));
    let eer = sweep.eer;
    let rec = sweep.recommended;

    html! {
        <div class="chart-wrap">
            <svg class="chart" viewBox={format!("0 0 {W} {H}")} role="img" aria-label="FRR/FAR 门限权衡曲线">
                // Y 轴网格与刻度（百分比）
                { for y_ticks.iter().map(|r| {
                    let py = y(*r);
                    html! {
                        <>
                            <line class="chart-grid" x1={M_L.to_string()} y1={py.to_string()}
                                  x2={(W - M_R).to_string()} y2={py.to_string()} />
                            <text class="axis-text" x={(M_L - 8.0).to_string()} y={(py + 4.0).to_string()}
                                  text-anchor="end">{ format!("{:.0}%", r * 100.0) }</text>
                        </>
                    }
                })}

                // X 轴刻度：门限 ns + 等效距离 m
                { for x_ticks.iter().map(|t| {
                    let px = x(*t);
                    let d = Params::equivalent_distance_m(*t, props.key_proc_ns);
                    html! {
                        <>
                            <line class="axis-tick" x1={px.to_string()} y1={(H - M_B).to_string()}
                                  x2={px.to_string()} y2={(H - M_B + 5.0).to_string()} />
                            <text class="axis-text" x={px.to_string()} y={(H - M_B + 18.0).to_string()}
                                  text-anchor="middle">{ format!("{t:.0} ns") }</text>
                            <text class="axis-subtext" x={px.to_string()} y={(H - M_B + 34.0).to_string()}
                                  text-anchor="middle">{ format!("{d:.1} m") }</text>
                        </>
                    }
                })}

                // 坐标轴
                <line class="axis-line" x1={M_L.to_string()} y1={M_T.to_string()}
                      x2={M_L.to_string()} y2={(H - M_B).to_string()} />
                <line class="axis-line" x1={M_L.to_string()} y1={(H - M_B).to_string()}
                      x2={(W - M_R).to_string()} y2={(H - M_B).to_string()} />
                <text class="axis-title" x={M_L.to_string()} y="14">{"比率"}</text>
                <text class="axis-title" x={(W - M_R).to_string()} y={(H - 6.0).to_string()} text-anchor="end">
                    {"RTT 门限（上：ns / 下：等效距离 m）"}
                </text>

                // 当前门限竖线
                <line class="current-line" x1={cur_x.to_string()} y1={M_T.to_string()}
                      x2={cur_x.to_string()} y2={(H - M_B).to_string()} />
                <text class="current-label" x={cur_x.to_string()} y={(M_T + 10.0).to_string()} text-anchor="middle">
                    {"当前门限"}
                </text>

                // 曲线
                <path class="frr-line" d={frr_path} />
                <path class="far-line" d={far_path} />

                // EER 与推荐点
                <circle class="eer-dot" cx={x(eer.threshold_ns).to_string()} cy={y(eer.frr).to_string()} r="6" />
                <text class="dot-label eer-label" x={x(eer.threshold_ns).to_string()}
                      y={(y(eer.frr) - 10.0).clamp(M_T + 4.0, H).to_string()} text-anchor="middle">
                    { format!("EER {:.2}%", eer.frr * 100.0) }
                </text>
                <rect class="rec-dot" x={(x(rec.threshold_ns) - 5.0).to_string()}
                      y={(y(rec.frr) - 5.0).to_string()} width="10" height="10" />
                <text class="dot-label rec-label" x={x(rec.threshold_ns).to_string()}
                      y={(y(rec.frr) + 22.0).clamp(0.0, H - M_B).to_string()} text-anchor="middle">
                    { format!("推荐 {:.0}ns", rec.threshold_ns) }
                </text>

                // 图例
                <g transform={format!("translate({},{})", W - M_R - 168.0, M_T + 6.0)}>
                    <rect class="legend-bg" x="0" y="0" width="162" height="44" rx="6" />
                    <line class="frr-line" x1="10" y1="15" x2="38" y2="15" />
                    <text class="legend-text" x="46" y="19">{"误拒率 FRR（合法被拒）"}</text>
                    <line class="far-line" x1="10" y1="34" x2="38" y2="34" />
                    <text class="legend-text" x="46" y="38">{"漏放率 FAR（攻击放行）"}</text>
                </g>
            </svg>
        </div>
    }
}
