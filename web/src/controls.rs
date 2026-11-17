//! 左侧参数控制面板：协议、物理层、中继三类参数 + 场景预设。

use db_core::{Params, RelayMode};
use yew::prelude::*;

use crate::state::{AppState, TrialCount};

#[derive(Properties, PartialEq)]
pub struct ControlProps {
    pub state: AppState,
    pub on_change: Callback<AppState>,
    pub on_run_batch: Callback<AppState>,
}

/// 数字滑杆控件。
#[derive(Properties, PartialEq, Clone)]
struct SliderProps {
    label: String,
    value: f64,
    min: f64,
    max: f64,
    step: f64,
    unit: String,
    /// 小数显示位数。
    decimals: usize,
    /// 指数刻度（用于误码率等跨数量级参数）。
    log_scale: bool,
    on_input: Callback<f64>,
}

#[function_component(Slider)]
fn slider(props: &SliderProps) -> Html {
    let SliderProps { label, value, min, max, step, unit, decimals, log_scale, on_input } =
        props.clone();

    // log 刻度下，滑杆位置走 log10 映射。
    let pos = if log_scale {
        let (lmin, lmax) = (min.max(1e-12).log10(), max.log10());
        (value.max(1e-12).log10() - lmin) / (lmax - lmin) * 100.0
    } else {
        (value - min) / (max - min) * 100.0
    };

    let cb = on_input.reform(move |e: InputEvent| {
        let input: web_sys::HtmlInputElement = e.target_unchecked_into();
        let raw: f64 = input.value().parse().unwrap_or(0.0);
        if log_scale {
            let (lmin, lmax) = (min.max(1e-12).log10(), max.log10());
            10f64.powf(lmin + raw / 100.0 * (lmax - lmin))
        } else {
            min + raw / 100.0 * (max - min)
        }
    });

    let display = if log_scale {
        format!("{value:.1e}")
    } else {
        format!("{value:.decimals$}")
    };

    html! {
        <label class="control">
            <span class="control-label">
                { label }
                <em class="control-value">{ display }{ " " }{ unit }</em>
            </span>
            <input type="range" min="0" max="100" step={step.to_string()}
                   value={pos.to_string()} oninput={cb} />
        </label>
    }
}

#[function_component(ControlPanel)]
pub fn control_panel(props: &ControlProps) -> Html {
    let state = props.state.clone();
    let on_change = props.on_change.clone();

    /// 生成“修改 params 单个 f64 字段”的滑杆回调。
    fn param_cb<F>(state: AppState, on_change: Callback<AppState>, f: F) -> Callback<f64>
    where
        F: Fn(&mut Params, f64) + 'static,
    {
        Callback::from(move |v: f64| {
            let mut s = state.clone();
            f(&mut s.params, v);
            on_change.emit(s);
        })
    }

    /// 生成“修改 AppState 单个 f64 字段”的滑杆回调。
    fn state_cb<F>(state: AppState, on_change: Callback<AppState>, f: F) -> Callback<f64>
    where
        F: Fn(&mut AppState, f64) + 'static,
    {
        Callback::from(move |v: f64| {
            let mut s = state.clone();
            f(&mut s, v);
            on_change.emit(s);
        })
    }

    let p = &state.params;

    html! {
        <div class="panel">
            <section class="panel-section">
                <h3>{"场景预设"}</h3>
                <div class="btn-row">
                    <button onclick={on_change.reform({
                        let state = state.clone();
                        move |_| { let mut s = state.clone(); s.preset_legit(); s }
                    })}>{"合法近距"}</button>
                    <button class="danger" onclick={on_change.reform({
                        let state = state.clone();
                        move |_| { let mut s = state.clone(); s.preset_classic_attack(); s }
                    })}>{"经典双中继盗窃"}</button>
                    <button onclick={on_change.reform({
                        let state = state.clone();
                        move |_| { let mut s = state.clone(); s.preset_borderline(); s }
                    })}>{"临界对抗"}</button>
                </div>
                <div class="control-row">
                    <label class="control inline">
                        <span class="control-label">{"中继台数"}</span>
                        <select onchange={on_change.reform({
                            let state = state.clone();
                            move |e: Event| {
                                let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                                let mut s = state.clone();
                                s.relay_count = input.value().parse().unwrap_or(0);
                                s
                            }
                        })} value={state.relay_count.to_string()}>
                            <option value="0">{"0（直连）"}</option>
                            <option value="1">{"1 台"}</option>
                            <option value="2">{"2 台（典型攻击）"}</option>
                        </select>
                    </label>
                    <label class="control inline">
                        <span class="control-label">{"中继模式"}</span>
                        <select onchange={on_change.reform({
                            let state = state.clone();
                            move |e: Event| {
                                let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                                let mut s = state.clone();
                                s.relay_mode = match input.value().as_str() {
                                    "decode" => RelayMode::DecodeForward,
                                    _ => RelayMode::AnalogAmplify,
                                };
                                // 切换模式时同步默认转发时延，用户随后仍可手动覆盖。
                                s.params.relay_proc_mean_ns = s.relay_mode.default_processing_ns();
                                s
                            }
                        })}>
                            <option value="analog" selected={state.relay_mode == RelayMode::AnalogAmplify}>
                                { RelayMode::AnalogAmplify.label() }
                            </option>
                            <option value="decode" selected={state.relay_mode == RelayMode::DecodeForward}>
                                { RelayMode::DecodeForward.label() }
                            </option>
                        </select>
                    </label>
                </div>
            </section>

            <section class="panel-section">
                <h3>{"距离边界协议"}</h3>
                <Slider label="RTT 门限" value={p.rtt_threshold_ns} min={40.0} max={400.0} step={0.5}
                        unit="ns" decimals={1} log_scale={false}
                        on_input={param_cb(state.clone(), on_change.clone(), |pp, v| pp.rtt_threshold_ns = v)} />
                <Slider label="快速交换轮数 N" value={p.rounds as f64} min={8.0} max={64.0} step={1.0}
                        unit="轮" decimals={0} log_scale={false}
                        on_input={param_cb(state.clone(), on_change.clone(), |pp, v| pp.rounds = v.round() as usize)} />
                <Slider label="容错失败轮数" value={p.max_fail_rounds as f64} min={0.0} max={8.0} step={1.0}
                        unit="轮" decimals={0} log_scale={false}
                        on_input={param_cb(state.clone(), on_change.clone(), |pp, v| pp.max_fail_rounds = v.round() as usize)} />
                <Slider label="诚实基准距离" value={state.honest_distance_m} min={0.5} max={6.0} step={0.1}
                        unit="m" decimals={1} log_scale={false}
                        on_input={state_cb(state.clone(), on_change.clone(), |s, v| s.honest_distance_m = v)} />
            </section>

            <section class="panel-section">
                <h3>{"物理层与设备时延"}</h3>
                <Slider label="钥匙处理时延均值" value={p.key_proc_mean_ns} min={20.0} max={200.0} step={1.0}
                        unit="ns" decimals={0} log_scale={false}
                        on_input={param_cb(state.clone(), on_change.clone(), |pp, v| pp.key_proc_mean_ns = v)} />
                <Slider label="钥匙处理抖动 σ" value={p.key_proc_std_ns} min={0.0} max={20.0} step={0.5}
                        unit="ns" decimals={1} log_scale={false}
                        on_input={param_cb(state.clone(), on_change.clone(), |pp, v| pp.key_proc_std_ns = v)} />
                <Slider label="RTT 测量噪声 σ" value={p.rtt_noise_std_ns} min={0.0} max={15.0} step={0.5}
                        unit="ns" decimals={1} log_scale={false}
                        on_input={param_cb(state.clone(), on_change.clone(), |pp, v| pp.rtt_noise_std_ns = v)} />
                <Slider label="信道误码率 BER" value={p.bit_error_rate} min={1e-6} max={1e-2} step={0.01}
                        unit="" decimals={0} log_scale={true}
                        on_input={param_cb(state.clone(), on_change.clone(), |pp, v| pp.bit_error_rate = v)} />
            </section>

            <section class="panel-section">
                <h3>{"中继设备"}</h3>
                <Slider label="单台转发处理均值" value={p.relay_proc_mean_ns} min={0.0} max={500.0} step={1.0}
                        unit="ns" decimals={0} log_scale={false}
                        on_input={param_cb(state.clone(), on_change.clone(), |pp, v| pp.relay_proc_mean_ns = v)} />
                <Slider label="转发处理抖动 σ" value={p.relay_proc_std_ns} min={0.0} max={40.0} step={0.5}
                        unit="ns" decimals={1} log_scale={false}
                        on_input={param_cb(state.clone(), on_change.clone(), |pp, v| pp.relay_proc_std_ns = v)} />
                <Slider label="中继间回传额外时延（单向）" value={p.relay_backhaul_ns} min={0.0} max={500.0} step={1.0}
                        unit="ns" decimals={0} log_scale={false}
                        on_input={param_cb(state.clone(), on_change.clone(), |pp, v| pp.relay_backhaul_ns = v)} />
            </section>

            <section class="panel-section">
                <h3>{"仿真"}</h3>
                <label class="control inline">
                    <span class="control-label">{"蒙特卡洛次数"}</span>
                    <select onchange={on_change.reform({
                        let state = state.clone();
                        move |e: Event| {
                            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                            let mut s = state.clone();
                            s.trial_count = match input.value().as_str() {
                                "low" => TrialCount::Low,
                                "high" => TrialCount::High,
                                _ => TrialCount::Medium,
                            };
                            s
                        }
                    })}>
                        <option value="low" selected={state.trial_count == TrialCount::Low}>{TrialCount::Low.label()}</option>
                        <option value="medium" selected={state.trial_count == TrialCount::Medium}>{TrialCount::Medium.label()}</option>
                        <option value="high" selected={state.trial_count == TrialCount::High}>{TrialCount::High.label()}</option>
                    </select>
                </label>
                <Slider label="随机种子" value={state.seed as f64} min={1.0} max={9999.0} step={1.0}
                        unit="" decimals={0} log_scale={false}
                        on_input={state_cb(state.clone(), on_change.clone(), |s, v| s.seed = v.round() as u64)} />
                <div class="btn-row">
                    <button class="primary" onclick={props.on_run_batch.reform({
                        let state = state.clone();
                        move |_| state.clone()
                    })}>{"重新统计 / 扫描"}</button>
                </div>
            </section>
        </div>
    }
}
