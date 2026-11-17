//! Yew 应用入口：组装布点画布、参数面板、单次/批量结果与 FRR/FAR 曲线。

mod chart;
mod controls;
mod field;
mod results;
mod state;

use db_core::Params;
use yew::prelude::*;

use crate::chart::SweepChart;
use crate::controls::ControlPanel;
use crate::field::FieldCanvas;
use crate::results::{BatchStats, SingleResult};
use crate::state::{AppState, DragKind};
use db_core::Vec2;

struct App {
    state: AppState,
}

enum Msg {
    Replace(AppState),
    MoveDevice(DragKind, Vec2),
}

impl Component for App {
    type Message = Msg;
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        Self { state: AppState::default() }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Msg) -> bool {
        match msg {
            Msg::Replace(s) => {
                self.state = s;
                true
            }
            Msg::MoveDevice(kind, pos) => {
                self.state.move_device(kind, pos);
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let state = self.state.clone();
        let link = ctx.link().clone();

        let on_change = link.callback(Msg::Replace);
        let on_move = link.callback(|(kind, pos): (DragKind, Vec2)| Msg::MoveDevice(kind, pos));
        // “重新统计”按钮：状态不变也触发一次重渲染（批量结果在 view 中按现参数重算）。
        let on_run_batch = on_change.clone();
        // 批量统计是纯函数，直接在渲染期计算（4000 次约百万级浮点运算，WASM 数十毫秒）。
        let batch = state.run_batch();
        let report = state.run_single();

        let scenario = state.scenario();
        let floor_ns = scenario.rtt_floor_ns(&state.params);
        let margin_ns = state.params.rtt_threshold_ns - floor_ns;
        let physical_note = if state.relay_count == 0 {
            format!(
                "直连场景：RTT 下界 {floor_ns:.1} ns，门限余量 {margin_ns:.1} ns。",
            )
        } else if margin_ns >= 0.0 {
            format!(
                "⚠️ 当前门限高于该中继布局的 RTT 下界 {:.1} ns，攻击可能被漏放——请收紧门限或观察 FAR。",
                margin_ns
            )
        } else {
            format!(
                "中继路径 RTT 下界 {floor_ns:.1} ns，已超门限 {:.1} ns（物理上必然超时），攻击无法靠转发通过。",
                -margin_ns
            )
        };

        html! {
            <div class="app">
                <header class="app-header">
                    <h1>{"距离边界认证仿真 · 中继攻击防御演示"}</h1>
                    <p class="subtitle">
                        {"基于 Hancke–Kuhn 快速挑战—应答：车端逐轮测量 RTT，以 2d/c 光速下界约束真实距离。"}
                        {"中继转发只能增加空中绕路与设备处理时延，无法缩短 RTT。"}
                    </p>
                </header>

                <div class="layout">
                    <aside class="sidebar">
                        <ControlPanel state={state.clone()} on_change={on_change.clone()}
                                      on_run_batch={on_run_batch} />
                    </aside>

                    <main class="main">
                        <FieldCanvas state={state.clone()} on_move={on_move} />

                        <div class="note-bar">
                            <strong>{"物理判读："}</strong>{ physical_note }
                            <span class="note-aux">
                                { format!(
                                    "门限等效距离 {:.2} m ｜ 1m 空气单程 {:.3} ns",
                                    Params::equivalent_distance_m(
                                        state.params.rtt_threshold_ns,
                                        state.params.key_proc_mean_ns
                                    ),
                                    Params::geometric_floor_ns(1.0) / 2.0
                                ) }
                            </span>
                        </div>

                        <SingleResult report={report} params={state.params.clone()} />

                        <BatchStats batch={batch.clone()} params={state.params.clone()} />

                        <div class="card">
                            <div class="card-head"><h3>{"FRR / FAR 门限权衡曲线"}</h3></div>
                            <SweepChart sweep={batch.sweep}
                                        current_threshold_ns={state.params.rtt_threshold_ns}
                                        key_proc_ns={state.params.key_proc_mean_ns} />
                            <div class="tradeoff-note">
                                <h4>{"取舍解读"}</h4>
                                <ul>
                                    <li><b>{"门限收紧"}</b>
                                        {"：等效距离更小、攻击面更窄（FAR ↓），但合法钥匙稍有遮挡/抖动就会被拒（FRR ↑）。"}</li>
                                    <li><b>{"EER 点"}</b>
                                        {"：FRR=FAR 的中性工作点，只用于衡量两套分布的可分性，不建议直接作为车规配置。"}</li>
                                    <li><b>{"安全优先推荐"}</b>
                                        {"：在 FRR ≤ 1% 约束下取最小门限——车辆场景漏放一辆被盗车的代价远高于让用户多掏一次手机，"}
                                        {"因此默认偏向压 FAR，用 1% 的可接受误拒换尽可能小的攻击面。"}</li>
                                    <li><b>{"提高 N 或收紧容错轮数"}</b>
                                        {"：对随机抖动更鲁棒（单轮超时被平均），但会拉长认证耗时；BER 高时过度收紧容错反而推高 FRR。"}</li>
                                </ul>
                            </div>
                        </div>

                        <footer class="app-footer">
                            {"仿真核心 db-core 为零依赖纯 Rust，确定性种子可复现；所有时延单位 ns，距离单位 m。"}
                        </footer>
                    </main>
                </div>
            </div>
        }
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
