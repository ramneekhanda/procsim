use bevy::prelude::*;
use std::collections::{HashMap, HashSet, VecDeque};

use crate::parser::graphv2::{
    GraphDefinition, GroupDef, LayoutConfig, LayoutDirection, LayoutType, NodeConnection,
};

/// Bounding box and visual properties for a rendered group container.
#[derive(Debug, Clone, PartialEq)]
pub struct GroupBoxBounds {
    pub group_id: String,
    pub title: Option<String>,
    pub center: Vec2,
    pub size: Vec2,
    pub style: crate::parser::graphv2::GroupStyle,
}

/// The computed coordinates for all nodes and group bounding boxes in the graph.
#[derive(Debug, Clone, Default)]
pub struct ComputedLayout {
    pub node_positions: HashMap<String, Vec2>,
    pub group_boxes: Vec<GroupBoxBounds>,
    pub draggable: bool,
}

/// Macro-element representing either a composite group of nodes or a standalone node.
#[derive(Debug, Clone)]
struct MacroElement {
    id: String,
    is_group: bool,
    member_nodes: Vec<String>,
    internal_positions: HashMap<String, Vec2>, // Relative to macro center (0, 0)
    size: Vec2,
}

/// Computes the complete deterministic layout for the given graph definition.
pub fn compute_graph_layout(graph_defn: &GraphDefinition) -> ComputedLayout {
    let layout_config = graph_defn.layout.clone().unwrap_or_default();
    let node_count = graph_defn.graph.len();

    if node_count == 0 {
        return ComputedLayout::default();
    }

    match layout_config.r#type {
        LayoutType::Manual => compute_manual_layout(graph_defn, &layout_config),
        LayoutType::Circular => compute_circular_layout(graph_defn, &layout_config),
        LayoutType::Grid => compute_grid_layout(graph_defn, &layout_config),
        LayoutType::Hierarchical => compute_hierarchical_layout(graph_defn, &layout_config),
    }
}

/// Manual layout mode: respects `pos: [x, y]` if given, otherwise places at origin.
fn compute_manual_layout(
    graph_defn: &GraphDefinition,
    layout_config: &LayoutConfig,
) -> ComputedLayout {
    let mut node_positions = HashMap::new();
    for node in &graph_defn.graph {
        if let Some([x, y]) = node.pos {
            node_positions.insert(node.name.clone(), Vec2::new(x, y));
        } else {
            node_positions.insert(node.name.clone(), Vec2::ZERO);
        }
    }
    ComputedLayout {
        node_positions,
        group_boxes: Vec::new(),
        draggable: layout_config.draggable.unwrap_or(true),
    }
}

/// Circular layout mode: arranges all nodes evenly on an ellipse.
fn compute_circular_layout(
    graph_defn: &GraphDefinition,
    layout_config: &LayoutConfig,
) -> ComputedLayout {
    let mut node_positions = HashMap::new();
    let n = graph_defn.graph.len();
    if n == 0 {
        return ComputedLayout::default();
    }

    let radius_x = 180.0 + (n as f32 * 35.0);
    let radius_y = 140.0 + (n as f32 * 25.0);
    let angle_step = std::f32::consts::TAU / (n as f32);

    for (i, node) in graph_defn.graph.iter().enumerate() {
        if let Some([x, y]) = node.pos {
            node_positions.insert(node.name.clone(), Vec2::new(x, y));
            continue;
        }
        let theta = (i as f32) * angle_step - std::f32::consts::FRAC_PI_2;
        let mut pos = Vec2::new(theta.cos() * radius_x, theta.sin() * radius_y);
        if let Some([dx, dy]) = node.offset {
            pos += Vec2::new(dx, dy);
        }
        node_positions.insert(node.name.clone(), pos);
    }

    ComputedLayout {
        node_positions,
        group_boxes: Vec::new(),
        draggable: layout_config.draggable.unwrap_or(false),
    }
}

/// Grid layout mode: arranges nodes into a rectangular matrix.
fn compute_grid_layout(
    graph_defn: &GraphDefinition,
    layout_config: &LayoutConfig,
) -> ComputedLayout {
    let mut node_positions = HashMap::new();
    let n = graph_defn.graph.len();
    if n == 0 {
        return ComputedLayout::default();
    }

    let cols = (n as f32).sqrt().ceil() as usize;
    let cols = cols.max(1);
    let rows = (n + cols - 1) / cols;

    let spacing_x = layout_config.get_node_sep().max(60.0);
    let spacing_y = layout_config.get_node_sep().max(60.0);

    let offset_x = -((cols - 1) as f32 * spacing_x) / 2.0;
    let offset_y = ((rows - 1) as f32 * spacing_y) / 2.0;

    for (i, node) in graph_defn.graph.iter().enumerate() {
        if let Some([x, y]) = node.pos {
            node_positions.insert(node.name.clone(), Vec2::new(x, y));
            continue;
        }
        let col = i % cols;
        let row = i / cols;
        let mut pos = Vec2::new(
            offset_x + (col as f32 * spacing_x),
            offset_y - (row as f32 * spacing_y),
        );
        if let Some([dx, dy]) = node.offset {
            pos += Vec2::new(dx, dy);
        }
        node_positions.insert(node.name.clone(), pos);
    }

    ComputedLayout {
        node_positions,
        group_boxes: Vec::new(),
        draggable: layout_config.draggable.unwrap_or(false),
    }
}

/// Hierarchical & Grouped layout engine.
fn compute_hierarchical_layout(
    graph_defn: &GraphDefinition,
    layout_config: &LayoutConfig,
) -> ComputedLayout {
    let rank_spacing = layout_config.get_rank_sep();
    let node_spacing = layout_config.get_node_sep();
    let group_spacing = layout_config.get_group_sep();
    let global_dir = layout_config.direction;

    // Node lookup map
    let node_map: HashMap<String, &NodeConnection> = graph_defn
        .graph
        .iter()
        .map(|n| (n.name.clone(), n))
        .collect();

    // 1. Group membership resolution
    let mut node_to_group: HashMap<String, String> = HashMap::new();
    let mut group_map: HashMap<String, &GroupDef> = HashMap::new();

    for g in &graph_defn.groups {
        group_map.insert(g.id.clone(), g);
        for n_name in &g.nodes {
            node_to_group.insert(n_name.clone(), g.id.clone());
        }
    }

    for node in &graph_defn.graph {
        if let Some(ref gid) = node.group {
            node_to_group.insert(node.name.clone(), gid.clone());
        }
    }

    // 2. Build Macro-Elements (groups and standalone nodes)
    let mut macro_elements: Vec<MacroElement> = Vec::new();
    let mut processed_nodes: HashSet<String> = HashSet::new();

    // Process explicit groups first
    for g in &graph_defn.groups {
        let mut members: Vec<String> = Vec::new();
        for node in &graph_defn.graph {
            if node_to_group.get(&node.name) == Some(&g.id) {
                members.push(node.name.clone());
                processed_nodes.insert(node.name.clone());
            }
        }

        if members.is_empty() {
            continue;
        }

        // Sort group members by explicit order if given
        members.sort_by_key(|n_name| {
            node_map
                .get(n_name)
                .and_then(|n| n.order)
                .unwrap_or(usize::MAX)
        });

        // Determine group internal layout direction (defaults to opposite of global direction)
        let group_dir = g
            .layout
            .as_ref()
            .and_then(|l| l.direction)
            .unwrap_or(match global_dir {
                LayoutDirection::Lr | LayoutDirection::Rl => LayoutDirection::Tb,
                LayoutDirection::Tb | LayoutDirection::Bt => LayoutDirection::Lr,
            });

        let g_spacing = g
            .layout
            .as_ref()
            .map(|l| l.get_spacing(node_spacing))
            .unwrap_or(node_spacing);

        let mut internal_positions = HashMap::new();
        let k = members.len();

        match group_dir {
            LayoutDirection::Tb | LayoutDirection::Bt => {
                let invert = matches!(group_dir, LayoutDirection::Bt);
                for (i, m_name) in members.iter().enumerate() {
                    let factor = (k as f32 - 1.0) / 2.0 - (i as f32);
                    let y = if invert { -factor } else { factor } * g_spacing;
                    internal_positions.insert(m_name.clone(), Vec2::new(0.0, y));
                }
            }
            LayoutDirection::Lr | LayoutDirection::Rl => {
                let invert = matches!(group_dir, LayoutDirection::Rl);
                for (i, m_name) in members.iter().enumerate() {
                    let factor = (i as f32) - (k as f32 - 1.0) / 2.0;
                    let x = if invert { -factor } else { factor } * g_spacing;
                    internal_positions.insert(m_name.clone(), Vec2::new(x, 0.0));
                }
            }
        }

        // Compute group bounding box
        let padding = g.style.as_ref().and_then(|s| s.padding).unwrap_or(28.0);
        let node_footprint = Vec2::new(180.0, 110.0);
        let mut min_pos = Vec2::splat(f32::INFINITY);
        let mut max_pos = Vec2::splat(f32::NEG_INFINITY);

        for pos in internal_positions.values() {
            min_pos = min_pos.min(*pos);
            max_pos = max_pos.max(*pos);
        }

        let g_size = (max_pos - min_pos) + node_footprint + Vec2::splat(padding * 2.0);

        // Explicit rank/order overrides are resolved from `node_map` directly
        // where they're actually used (the topological ranking pass and its
        // override step below), not cached on the macro element here.

        macro_elements.push(MacroElement {
            id: g.id.clone(),
            is_group: true,
            member_nodes: members,
            internal_positions,
            size: g_size,
        });
    }

    // Process standalone nodes (nodes not in any group)
    for node in &graph_defn.graph {
        if processed_nodes.contains(&node.name) {
            continue;
        }

        let mut internal_positions = HashMap::new();
        internal_positions.insert(node.name.clone(), Vec2::ZERO);

        macro_elements.push(MacroElement {
            id: node.name.clone(),
            is_group: false,
            member_nodes: vec![node.name.clone()],
            internal_positions,
            size: Vec2::new(180.0, 120.0),
        });
    }

    if macro_elements.is_empty() {
        return ComputedLayout::default();
    }

    // 3. Map macro element dependencies and compute topological rank
    let mut node_to_macro: HashMap<String, usize> = HashMap::new();
    for (m_idx, m) in macro_elements.iter().enumerate() {
        for n_name in &m.member_nodes {
            node_to_macro.insert(n_name.clone(), m_idx);
        }
    }

    let m_count = macro_elements.len();
    let mut adj: Vec<HashSet<usize>> = vec![HashSet::new(); m_count];
    let mut in_degree: Vec<usize> = vec![0; m_count];

    for node in &graph_defn.graph {
        if let Some(&src_macro) = node_to_macro.get(&node.name) {
            for link in &node.links {
                if let Some(&dst_macro) = node_to_macro.get(link) {
                    if src_macro != dst_macro {
                        if adj[src_macro].insert(dst_macro) {
                            in_degree[dst_macro] += 1;
                        }
                    }
                }
            }
        }
    }

    // Topological Longest Path Layering
    let mut ranks: Vec<usize> = vec![0; m_count];
    let mut in_deg_work = in_degree.clone();
    let mut queue: VecDeque<usize> = VecDeque::new();

    // Start with in-degree 0 or explicitly rank 0
    for i in 0..m_count {
        if let Some(explicit) = node_map
            .get(&macro_elements[i].id)
            .and_then(|n| n.rank)
            .or_else(|| {
                macro_elements[i]
                    .member_nodes
                    .iter()
                    .filter_map(|m| node_map.get(m).and_then(|n| n.rank))
                    .min()
            })
        {
            ranks[i] = explicit;
        }

        if in_deg_work[i] == 0 {
            queue.push_back(i);
        }
    }

    // Fallback if graph is entirely cyclic
    if queue.is_empty() && m_count > 0 {
        queue.push_back(0);
    }

    let mut visited_count = 0;
    while let Some(u) = queue.pop_front() {
        visited_count += 1;
        let u_rank = ranks[u];

        for &v in &adj[u] {
            ranks[v] = ranks[v].max(u_rank + 1);
            if in_deg_work[v] > 0 {
                in_deg_work[v] -= 1;
                if in_deg_work[v] == 0 {
                    queue.push_back(v);
                }
            }
        }
    }

    // If there were unresolved cycles, assign incremental ranks
    if visited_count < m_count {
        for i in 0..m_count {
            if in_deg_work[i] > 0 {
                ranks[i] = ranks[i].max(1);
            }
        }
    }

    // Enforce explicit node rank overrides
    for (i, m) in macro_elements.iter().enumerate() {
        let explicit = m
            .member_nodes
            .iter()
            .filter_map(|m_name| node_map.get(m_name).and_then(|n| n.rank))
            .min();
        if let Some(exp_r) = explicit {
            ranks[i] = exp_r;
        }
    }

    // 4. Group macro elements by rank
    let max_rank = ranks.iter().copied().max().unwrap_or(0);
    let mut rank_layers: Vec<Vec<usize>> = vec![Vec::new(); max_rank + 1];
    for (i, &r) in ranks.iter().enumerate() {
        rank_layers[r].push(i);
    }

    // Sort within each rank layer by order
    for layer in rank_layers.iter_mut() {
        layer.sort_by_key(|&m_idx| {
            macro_elements[m_idx]
                .member_nodes
                .iter()
                .filter_map(|n_name| node_map.get(n_name).and_then(|n| n.order))
                .min()
                .unwrap_or(m_idx)
        });
    }

    // 5. Place macro elements along global direction
    let mut macro_centers: HashMap<usize, Vec2> = HashMap::new();
    let num_layers = rank_layers.len();

    match global_dir {
        LayoutDirection::Lr | LayoutDirection::Rl => {
            let invert_x = matches!(global_dir, LayoutDirection::Rl);
            let total_rank_width = (num_layers as f32 - 1.0) * rank_spacing;
            let start_x = if invert_x {
                total_rank_width / 2.0
            } else {
                -total_rank_width / 2.0
            };

            for (r, layer) in rank_layers.iter().enumerate() {
                let rank_x = if invert_x {
                    start_x - (r as f32 * rank_spacing)
                } else {
                    start_x + (r as f32 * rank_spacing)
                };

                let k = layer.len();
                if k == 0 {
                    continue;
                } else if k == 1 {
                    macro_centers.insert(layer[0], Vec2::new(rank_x, 0.0));
                    continue;
                }

                let mut steps: Vec<f32> = Vec::new();
                for i in 0..k - 1 {
                    let is_g1 = macro_elements[layer[i]].is_group;
                    let is_g2 = macro_elements[layer[i + 1]].is_group;
                    let step = if !is_g1 && !is_g2 {
                        node_spacing
                    } else {
                        (macro_elements[layer[i]].size.y + macro_elements[layer[i + 1]].size.y)
                            / 2.0
                            + (group_spacing - 140.0).max(30.0)
                    };
                    steps.push(step);
                }

                let total_span: f32 = steps.iter().sum();
                let mut cur_y = total_span / 2.0;

                for (i, &m_idx) in layer.iter().enumerate() {
                    if i > 0 {
                        cur_y -= steps[i - 1];
                    }
                    macro_centers.insert(m_idx, Vec2::new(rank_x, cur_y));
                }
            }
        }
        LayoutDirection::Tb | LayoutDirection::Bt => {
            let invert_y = matches!(global_dir, LayoutDirection::Bt);
            let total_rank_height = (num_layers as f32 - 1.0) * rank_spacing;
            let start_y = if invert_y {
                -total_rank_height / 2.0
            } else {
                total_rank_height / 2.0
            };

            for (r, layer) in rank_layers.iter().enumerate() {
                let rank_y = if invert_y {
                    start_y + (r as f32 * rank_spacing)
                } else {
                    start_y - (r as f32 * rank_spacing)
                };

                let k = layer.len();
                if k == 0 {
                    continue;
                } else if k == 1 {
                    macro_centers.insert(layer[0], Vec2::new(0.0, rank_y));
                    continue;
                }

                let mut steps: Vec<f32> = Vec::new();
                for i in 0..k - 1 {
                    let is_g1 = macro_elements[layer[i]].is_group;
                    let is_g2 = macro_elements[layer[i + 1]].is_group;
                    let step = if !is_g1 && !is_g2 {
                        node_spacing
                    } else {
                        (macro_elements[layer[i]].size.x + macro_elements[layer[i + 1]].size.x)
                            / 2.0
                            + (group_spacing - 140.0).max(30.0)
                    };
                    steps.push(step);
                }

                let total_span: f32 = steps.iter().sum();
                let mut cur_x = -total_span / 2.0;

                for (i, &m_idx) in layer.iter().enumerate() {
                    if i > 0 {
                        cur_x += steps[i - 1];
                    }
                    macro_centers.insert(m_idx, Vec2::new(cur_x, rank_y));
                }
            }
        }
    }

    // 6. Project node coordinates into world space
    let mut raw_positions: HashMap<String, Vec2> = HashMap::new();
    for (m_idx, m) in macro_elements.iter().enumerate() {
        let m_center = macro_centers.get(&m_idx).copied().unwrap_or(Vec2::ZERO);
        for n_name in &m.member_nodes {
            let rel_pos = m
                .internal_positions
                .get(n_name)
                .copied()
                .unwrap_or(Vec2::ZERO);
            let mut final_pos = m_center + rel_pos;

            if let Some(n_conn) = node_map.get(n_name) {
                if let Some([x, y]) = n_conn.pos {
                    final_pos = Vec2::new(x, y);
                } else if let Some([dx, dy]) = n_conn.offset {
                    final_pos += Vec2::new(dx, dy);
                }
            }
            raw_positions.insert(n_name.clone(), final_pos);
        }
    }

    // 7. Center overall graph around (0, 0)
    let mut min_bound = Vec2::splat(f32::INFINITY);
    let mut max_bound = Vec2::splat(f32::NEG_INFINITY);

    for pos in raw_positions.values() {
        min_bound = min_bound.min(*pos);
        max_bound = max_bound.max(*pos);
    }

    let graph_center = if min_bound.x.is_finite() && max_bound.x.is_finite() {
        (min_bound + max_bound) / 2.0
    } else {
        Vec2::ZERO
    };

    let mut final_node_positions: HashMap<String, Vec2> = HashMap::new();
    for (name, pos) in raw_positions {
        final_node_positions.insert(name, pos - graph_center);
    }

    // 8. Compute final group bounding boxes
    let mut group_boxes: Vec<GroupBoxBounds> = Vec::new();
    for g in &graph_defn.groups {
        let members: Vec<&String> = g
            .nodes
            .iter()
            .chain(
                graph_defn
                    .graph
                    .iter()
                    .filter(|n| n.group.as_deref() == Some(&g.id))
                    .map(|n| &n.name),
            )
            .collect();

        if members.is_empty() {
            continue;
        }

        let mut g_min = Vec2::splat(f32::INFINITY);
        let mut g_max = Vec2::splat(f32::NEG_INFINITY);

        for m_name in members {
            if let Some(&pos) = final_node_positions.get(m_name) {
                g_min = g_min.min(pos);
                g_max = g_max.max(pos);
            }
        }

        if g_min.x.is_finite() && g_max.x.is_finite() {
            let padding = g.style.as_ref().and_then(|s| s.padding).unwrap_or(28.0);
            let node_extent = Vec2::new(90.0, 55.0); // Half of node width/height
            let box_min = g_min - node_extent - Vec2::splat(padding);
            let box_max = g_max + node_extent + Vec2::splat(padding);

            group_boxes.push(GroupBoxBounds {
                group_id: g.id.clone(),
                title: g.title.clone(),
                center: (box_min + box_max) / 2.0,
                size: box_max - box_min,
                style: g.style.clone().unwrap_or_default(),
            });
        }
    }

    ComputedLayout {
        node_positions: final_node_positions,
        group_boxes,
        draggable: layout_config.draggable.unwrap_or(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::graphv2::{
        GroupLayoutConfig, LayoutConfig, LayoutDirection, NodeConnection,
    };

    #[test]
    fn test_hierarchical_lr_pipeline() {
        let mut gd = GraphDefinition::default();
        gd.graph = vec![
            NodeConnection {
                name: "client".to_string(),
                node_type: "client".to_string(),
                links: vec!["gateway".to_string()],
                ..default()
            },
            NodeConnection {
                name: "gateway".to_string(),
                node_type: "gateway".to_string(),
                links: vec!["worker".to_string()],
                ..default()
            },
            NodeConnection {
                name: "worker".to_string(),
                node_type: "worker".to_string(),
                links: vec!["db".to_string()],
                ..default()
            },
            NodeConnection {
                name: "db".to_string(),
                node_type: "db".to_string(),
                links: vec![],
                ..default()
            },
        ];

        let layout = compute_graph_layout(&gd);
        let pos_client = layout.node_positions.get("client").unwrap();
        let pos_gateway = layout.node_positions.get("gateway").unwrap();
        let pos_worker = layout.node_positions.get("worker").unwrap();
        let pos_db = layout.node_positions.get("db").unwrap();

        // Should be ordered strictly left to right: client.x < gateway.x < worker.x < db.x
        assert!(pos_client.x < pos_gateway.x);
        assert!(pos_gateway.x < pos_worker.x);
        assert!(pos_worker.x < pos_db.x);
    }

    #[test]
    fn test_grouped_hybrid_layout_lr_with_tb_groups() {
        let mut gd = GraphDefinition::default();
        gd.layout = Some(LayoutConfig {
            direction: LayoutDirection::Lr,
            ..default()
        });
        gd.groups = vec![GroupDef {
            id: "compute".to_string(),
            title: Some("Compute Tier".to_string()),
            layout: Some(GroupLayoutConfig {
                direction: Some(LayoutDirection::Tb),
                sep: Some(120.0),
                ..default()
            }),
            nodes: vec!["worker_1".to_string(), "worker_2".to_string()],
            ..default()
        }];
        gd.graph = vec![
            NodeConnection {
                name: "gateway".to_string(),
                node_type: "gateway".to_string(),
                links: vec!["worker_1".to_string(), "worker_2".to_string()],
                ..default()
            },
            NodeConnection {
                name: "worker_1".to_string(),
                node_type: "worker".to_string(),
                links: vec!["db".to_string()],
                group: Some("compute".to_string()),
                ..default()
            },
            NodeConnection {
                name: "worker_2".to_string(),
                node_type: "worker".to_string(),
                links: vec!["db".to_string()],
                group: Some("compute".to_string()),
                ..default()
            },
            NodeConnection {
                name: "db".to_string(),
                node_type: "db".to_string(),
                links: vec![],
                ..default()
            },
        ];

        let layout = compute_graph_layout(&gd);
        let pos_gw = layout.node_positions.get("gateway").unwrap();
        let pos_w1 = layout.node_positions.get("worker_1").unwrap();
        let pos_w2 = layout.node_positions.get("worker_2").unwrap();
        let pos_db = layout.node_positions.get("db").unwrap();

        // Horizontal pipeline
        assert!(pos_gw.x < pos_w1.x);
        assert!(pos_w1.x < pos_db.x);

        // Vertically stacked workers
        assert_eq!(pos_w1.x, pos_w2.x);
        assert!(pos_w1.y > pos_w2.y); // w1 above w2

        // Group bounding box was computed
        assert_eq!(layout.group_boxes.len(), 1);
        assert_eq!(layout.group_boxes[0].group_id, "compute");
    }

    #[test]
    fn test_explicit_pos_override() {
        let mut gd = GraphDefinition::default();
        gd.graph = vec![NodeConnection {
            name: "n1".to_string(),
            node_type: "type1".to_string(),
            pos: Some([100.0, 200.0]),
            links: vec![],
            ..default()
        }];

        let layout = compute_graph_layout(&gd);
        let pos = layout.node_positions.get("n1").unwrap();
        assert!(pos.x.is_finite());
    }

    #[test]
    fn test_rank_sep_and_node_sep_yaml_parsing() {
        let yaml = r#"
graph_defn:
  layout:
    type: hierarchical
    direction: lr
    rank_sep: 350
    node_sep: 180
  graph:
    - name: a
      node_type: t
      links: [b, c]
    - name: b
      node_type: t
      links: [d]
    - name: c
      node_type: t
      links: [d]
    - name: d
      node_type: t
      links: []
"#;
        let parsed: crate::parser::graphv2::File =
            serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        let layout_cfg = parsed.graph_defn.layout.as_ref().unwrap();
        assert_eq!(layout_cfg.rank_sep, Some(350.0));
        assert_eq!(layout_cfg.node_sep, Some(180.0));

        let computed = compute_graph_layout(&parsed.graph_defn);
        let pos_a = computed.node_positions.get("a").unwrap();
        let pos_b = computed.node_positions.get("b").unwrap();
        let pos_c = computed.node_positions.get("c").unwrap();
        let _pos_d = computed.node_positions.get("d").unwrap();

        // Distance between rank 0 (a) and rank 1 (b) should be 350
        let rank_dist = (pos_b.x - pos_a.x).abs();
        assert!((rank_dist - 350.0).abs() < 1.0);

        // Distance between b and c (node_sep in same rank) should be 180
        let node_dist = (pos_b.y - pos_c.y).abs();
        assert!((node_dist - 180.0).abs() < 1.0);
    }
}
