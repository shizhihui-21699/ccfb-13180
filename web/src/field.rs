//! 2D 仿真场地：SVG 渲染设备、通信路径与门限等效距离，支持指针拖拽布点。

use db_core::{Params, Vec2};
use wasm_bindgen::JsCast;
use yew::prelude::*;
use web_sys::{Element, PointerEvent, SvgsvgElement};

use crate::state::{AppState, DragKind, FIELD_H, FIELD_W};

/// SVG 视口尺寸（px）。
pub const VIEW_W: f64 = 760.0;
pub const VIEW_H: f64 = 532.0;

fn scale_x(x: f64) -> f64 {
    x / FIELD_W * VIEW_W
}
fn scale_y(y: f64) -> f64 {
    y / FIELD_H * VIEW_H
}
fn to_svg(p: Vec2) -> (f64, f64) {
    (scale_x(p.x), scale_y(p.y))
}

#[derive(Properties, PartialEq)]
pub struct FieldProps {
    pub state: AppState,
    pub on_move: Callback<(DragKind, Vec2)>,
}

#[derive(Properties, PartialEq, Clone)]
struct NodeProps {
    kind: DragKind,
    pos: Vec2,
    color: String,
    glyph: String,
    svg_ref: NodeRef,
    on_move: Callback<(DragKind, Vec2)>,
    drag: UseStateHandle<Option<DragKind>>,
}

#[function_component(DraggableNode)]
fn draggable_node(props: &NodeProps) -> Html {
    let NodeProps { kind, pos, color, glyph, svg_ref, on_move, drag } = props.clone();
    let (cx, cy) = to_svg(pos);

    let handler = Callback::from(move |e: PointerEvent| {
        match e.type_().as_str() {
            "pointerdown" => {
                if let Some(target) = e.current_target() {
                    if let Ok(el) = target.dyn_into::<Element>() {
                        let _ = el.set_pointer_capture(e.pointer_id());
                    }
                }
                drag.set(Some(kind));
            }
            "pointermove" => {
                if *drag == Some(kind) {
                    if let Some(svg) = svg_ref.cast::<SvgsvgElement>() {
                        let rect = svg.get_bounding_client_rect();
                        let x = (e.client_x() as f64 - rect.left()) / rect.width() * FIELD_W;
                        let y = (e.client_y() as f64 - rect.top()) / rect.height() * FIELD_H;
                        on_move.emit((kind, Vec2::new(x, y).clamp(FIELD_W, FIELD_H)));
                    }
                }
            }
            "pointerup" | "pointercancel" if *drag == Some(kind) => {
                drag.set(None);
            }
            _ => {}
        }
    });

    let slug = match kind {
        DragKind::Vehicle => "node-vehicle",
        DragKind::Key => "node-key",
        DragKind::RelayA | DragKind::RelayB => "node-relay",
    };

    html! {
        <g class={classes!("node", slug)}
           style={format!("--node-color:{color}")}
           onpointerdown={handler.clone()}
           onpointermove={handler.clone()}
           onpointerup={handler}>
            <circle cx={cx.to_string()} cy={cy.to_string()} r="15" />
            <text x={cx.to_string()} y={(cy + 5.0).to_string()} text-anchor="middle">{ glyph }</text>
            <text class="node-label" x={cx.to_string()} y={(cy + 34.0).to_string()} text-anchor="middle">
                { kind.label() }
            </text>
        </g>
    }
}

#[function_component(FieldCanvas)]
pub fn field_canvas(props: &FieldProps) -> Html {
    let drag = use_state(|| None::<DragKind>);
    let svg_ref = use_node_ref();

    let s = &props.state;
    let threshold_r_m = Params::equivalent_distance_m(
        s.params.rtt_threshold_ns,
        s.params.key_proc_mean_ns,
    );
    let (vx, vy) = to_svg(s.vehicle);
    let threshold_px = threshold_r_m / FIELD_W * VIEW_W;

    // 路径折线（当前实际消息路径）。
    let path_points: Vec<Vec2> = match s.relay_count {
        0 => vec![s.vehicle, s.key],
        1 => vec![s.vehicle, s.relay_a, s.key],
        _ => vec![s.vehicle, s.relay_a, s.relay_b, s.key],
    };
    let path_d = path_points
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let (x, y) = to_svg(*p);
            format!("{}{x:.1},{y:.1}", if i == 0 { "M" } else { "L" })
        })
        .collect::<Vec<_>>()
        .join(" ");

    let direct_d = {
        let (x1, y1) = to_svg(s.vehicle);
        let (x2, y2) = to_svg(s.key);
        format!("M{x1:.1},{y1:.1} L{x2:.1},{y2:.1}")
    };

    let relay_proc = s.params.relay_proc_mean_ns;
    let floor_ns = s.scenario().rtt_floor_ns(&s.params);
    let eq_m = Params::equivalent_distance_m(floor_ns, s.params.key_proc_mean_ns);

    html! {
        <div class="field-wrap">
            <svg ref={svg_ref.clone()} class="field" viewBox={format!("0 0 {VIEW_W} {VIEW_H}")}
                 role="img" aria-label="设备布点仿真场地">
                { grid_lines() }

                // 门限等效距离圈
                <circle class="threshold-circle" cx={vx.to_string()} cy={vy.to_string()}
                        r={threshold_px.to_string()} />
                <text class="threshold-label" x={vx.to_string()}
                      y={((vy - threshold_px - 6.0).clamp(12.0, VIEW_H)).to_string()}
                      text-anchor="middle">
                    { format!("门限等效半径 {threshold_r_m:.2} m（{:.0} ns）", s.params.rtt_threshold_ns) }
                </text>

                // 车—钥匙直线距离参考
                <path class="direct-line" d={direct_d} />

                // 实际消息路径
                <path class={classes!("msg-path", if s.relay_count == 0 { "direct" } else { "relayed" })}
                      d={path_d} />

                <DraggableNode kind={DragKind::Vehicle} pos={s.vehicle} color={"#3b82f6".to_string()}
                    glyph={"🚗".to_string()} svg_ref={svg_ref.clone()} on_move={props.on_move.clone()} drag={drag.clone()} />
                <DraggableNode kind={DragKind::Key} pos={s.key} color={"#f59e0b".to_string()}
                    glyph={"🔑".to_string()} svg_ref={svg_ref.clone()} on_move={props.on_move.clone()} drag={drag.clone()} />
                if s.relay_count >= 1 {
                    <DraggableNode kind={DragKind::RelayA} pos={s.relay_a} color={"#ef4444".to_string()}
                        glyph={"📡".to_string()} svg_ref={svg_ref.clone()} on_move={props.on_move.clone()} drag={drag.clone()} />
                }
                if s.relay_count >= 2 {
                    <DraggableNode kind={DragKind::RelayB} pos={s.relay_b} color={"#ef4444".to_string()}
                        glyph={"📡".to_string()} svg_ref={svg_ref.clone()} on_move={props.on_move.clone()} drag={drag} />
                }
            </svg>

            <div class="field-hint">
                { format!(
                    "车—钥匙直线 {:.2} m ｜ 消息路径 {:.2} m ｜ 路径 RTT 下界 {floor_ns:.1} ns ｜ 下界等效距离 {eq_m:.2} m ｜ 中继转发处理 {relay_proc:.0} ns/台",
                    s.vehicle.distance(s.key),
                    s.scenario().path_distance_m(&s.params),
                ) }
            </div>
        </div>
    }
}

fn grid_lines() -> Html {
    let mut lines = Vec::new();
    let step_x = VIEW_W / FIELD_W;
    let step_y = VIEW_H / FIELD_H;
    for i in 0..=FIELD_W as usize {
        let x = i as f64 * step_x;
        lines.push(html! {
            <line x1={x.to_string()} y1="0" x2={x.to_string()} y2={VIEW_H.to_string()}
                  class={classes!("grid", (i % 5 == 0).then_some("grid-major"))} />
        });
    }
    for j in 0..=FIELD_H as usize {
        let y = j as f64 * step_y;
        lines.push(html! {
            <line x1="0" y1={y.to_string()} x2={VIEW_W.to_string()} y2={y.to_string()}
                  class={classes!("grid", (j % 5 == 0).then_some("grid-major"))} />
        });
    }
    html! { <>{ lines }</> }
}
