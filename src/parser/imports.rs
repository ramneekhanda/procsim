use crate::parser::graphv2::{File, GraphDefinition};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// An import definition allowing inclusion of external or preset YAML modules.
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, JsonSchema)]
pub struct ImportDef {
    /// Remote URL (e.g. `https://...`) or preset identifier (e.g. `theme:cyberpunk`, `stdlib:load_balancer`)
    pub from: String,
    /// Optional list of specific node type IDs to import from this module
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub import: Option<Vec<String>>,
    /// Optional alias / renaming for imported node types
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#as: Option<String>,
}

pub const PRESET_THEME_CYBERPUNK: &str = include_str!("../../plibs/themes/cyberpunk.yml");
pub const PRESET_THEME_CLOUD: &str = include_str!("../../plibs/themes/cloud.yml");
pub const PRESET_THEME_DATACENTER: &str = include_str!("../../plibs/themes/datacenter.yml");
pub const PRESET_THEME_MINIMAL: &str = include_str!("../../plibs/themes/minimal.yml");

pub const PRESET_STDLIB_LOAD_BALANCER: &str = r##"
graph_defn:
  node_types:
    - id: round_robin_lb
      attrs:
        ticks: 1
      fn: |
        fn on_init() {
          state.idx = 0;
        }
        fn on_timer() {}
        fn on_message(msg) {
          if links.len() == 0 { return; }
          let target = links[state.idx % links.len()];
          state.idx += 1;
          send(target, msg);
        }
    - id: weighted_lb
      attrs:
        ticks: 1
      fn: |
        fn on_init() {
          state.count = 0;
        }
        fn on_timer() {}
        fn on_message(msg) {
          if links.len() == 0 { return; }
          let target = links[state.count % links.len()];
          state.count += 1;
          send(target, msg);
        }
"##;

pub const PRESET_STDLIB_CIRCUIT_BREAKER: &str = r##"
graph_defn:
  node_types:
    - id: circuit_breaker
      attrs:
        ticks: 1
      fn: |
        fn on_init() {
          state.status = "CLOSED";
          state.failures = 0;
          state.threshold = 3;
        }
        fn on_timer() {}
        fn on_message(msg) {
          if state.status == "OPEN" {
            log("Circuit breaker OPEN - dropping message");
            return;
          }
          if links.len() > 0 {
            send(links[0], msg);
          }
        }
"##;

pub const PRESET_STDLIB_CACHE: &str = r##"
graph_defn:
  node_types:
    - id: lru_cache
      attrs:
        ticks: 1
      fn: |
        fn on_init() {
          state.store = #{};
        }
        fn on_timer() {}
        fn on_message(msg) {
          if state.store.contains(msg) {
            log("Cache HIT for: " + msg);
          } else {
            log("Cache MISS for: " + msg);
            state.store[msg] = true;
            if links.len() > 0 {
              send(links[0], msg);
            }
          }
        }
"##;

pub const PRESET_THEME_SYNTHWAVE: &str = include_str!("../../plibs/themes/synthwave.yml");
pub const PRESET_THEME_NORDIC: &str = include_str!("../../plibs/themes/nordic.yml");
pub const PRESET_THEME_DRACULA: &str = include_str!("../../plibs/themes/dracula.yml");
pub const PRESET_THEME_MATRIX: &str = include_str!("../../plibs/themes/matrix.yml");
pub const PRESET_THEME_SOLARIZED_LIGHT: &str =
    include_str!("../../plibs/themes/solarized_light.yml");

pub const PRESET_AWS_COMPUTE: &str = include_str!("../../plibs/aws/compute.yml");
pub const PRESET_AWS_NETWORKING: &str = include_str!("../../plibs/aws/networking.yml");
pub const PRESET_AWS_DATABASE: &str = include_str!("../../plibs/aws/database.yml");
pub const PRESET_AWS_MESSAGING: &str = include_str!("../../plibs/aws/messaging.yml");
pub const PRESET_AWS_ALL: &str = include_str!("../../plibs/aws/all.yml");

/// Looks up a built-in preset by name/shorthand.
pub fn get_builtin_preset(name: &str) -> Option<&'static str> {
    let normalized = name.trim().to_lowercase();
    match normalized.as_str() {
        // Theme library: plibs:themes/<name>
        "plibs:themes/cyberpunk"
        | "theme:cyberpunk"
        | "themes:cyberpunk"
        | "plibs:cyberpunk"
        | "cyberpunk"
        | "plibs/themes/cyberpunk.yml" => Some(PRESET_THEME_CYBERPUNK),

        "plibs:themes/cloud"
        | "theme:cloud"
        | "themes:cloud"
        | "theme:cloud_cards"
        | "themes:cloud_cards"
        | "plibs:cloud"
        | "cloud"
        | "cloud_cards"
        | "plibs/themes/cloud.yml" => Some(PRESET_THEME_CLOUD),

        "plibs:themes/datacenter"
        | "theme:datacenter"
        | "themes:datacenter"
        | "theme:rack"
        | "themes:rack"
        | "plibs:datacenter"
        | "datacenter"
        | "rack"
        | "plibs/themes/datacenter.yml" => Some(PRESET_THEME_DATACENTER),

        "plibs:themes/minimal"
        | "theme:minimal"
        | "themes:minimal"
        | "theme:capsule"
        | "themes:capsule"
        | "plibs:minimal"
        | "minimal"
        | "capsule"
        | "plibs/themes/minimal.yml" => Some(PRESET_THEME_MINIMAL),

        "plibs:themes/synthwave"
        | "theme:synthwave"
        | "themes:synthwave"
        | "plibs:synthwave"
        | "synthwave"
        | "plibs/themes/synthwave.yml" => Some(PRESET_THEME_SYNTHWAVE),

        "plibs:themes/nordic"
        | "theme:nordic"
        | "themes:nordic"
        | "plibs:nordic"
        | "nordic"
        | "nord"
        | "plibs/themes/nordic.yml" => Some(PRESET_THEME_NORDIC),

        "plibs:themes/dracula"
        | "theme:dracula"
        | "themes:dracula"
        | "plibs:dracula"
        | "dracula"
        | "plibs/themes/dracula.yml" => Some(PRESET_THEME_DRACULA),

        "plibs:themes/matrix"
        | "theme:matrix"
        | "themes:matrix"
        | "plibs:matrix"
        | "matrix"
        | "plibs/themes/matrix.yml" => Some(PRESET_THEME_MATRIX),

        "plibs:themes/solarized_light"
        | "plibs:themes/solarized"
        | "theme:solarized_light"
        | "themes:solarized_light"
        | "theme:solarized"
        | "themes:solarized"
        | "plibs:solarized_light"
        | "plibs:solarized"
        | "solarized_light"
        | "solarized"
        | "plibs/themes/solarized_light.yml" => Some(PRESET_THEME_SOLARIZED_LIGHT),

        // AWS library: plibs:aws/<module>
        "plibs:aws/compute" | "aws:compute" | "plibs/aws/compute.yml" => Some(PRESET_AWS_COMPUTE),
        "plibs:aws/networking" | "aws:networking" | "plibs/aws/networking.yml" => {
            Some(PRESET_AWS_NETWORKING)
        }
        "plibs:aws/database" | "aws:database" | "plibs/aws/database.yml" => {
            Some(PRESET_AWS_DATABASE)
        }
        "plibs:aws/messaging" | "aws:messaging" | "plibs/aws/messaging.yml" => {
            Some(PRESET_AWS_MESSAGING)
        }
        "plibs:aws/all" | "plibs:aws" | "aws:all" | "aws" | "plibs/aws/all.yml" => {
            Some(PRESET_AWS_ALL)
        }

        // Standard library presets: stdlib:*
        "stdlib:load_balancer" | "load_balancer" | "stdlib:lb" | "lb" => {
            Some(PRESET_STDLIB_LOAD_BALANCER)
        }
        "stdlib:circuit_breaker" | "circuit_breaker" => Some(PRESET_STDLIB_CIRCUIT_BREAKER),
        "stdlib:cache" | "cache" | "stdlib:lru_cache" | "lru_cache" => Some(PRESET_STDLIB_CACHE),
        _ => None,
    }
}

/// Scans raw YAML string for remote HTTP/HTTPS import URLs.
pub fn scan_import_urls(raw_yaml: &str) -> Vec<String> {
    let mut urls = Vec::new();

    // Fast heuristic line scanner to avoid failing if incomplete YAML is being edited
    for line in raw_yaml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("from:") || trimmed.starts_with("- from:") {
            if let Some(val_idx) = trimmed.find("from:") {
                let after = trimmed[val_idx + 5..].trim();
                let url = after
                    .trim_matches(|c| c == '\'' || c == '"' || c == ' ')
                    .to_string();
                if url.starts_with("http://") || url.starts_with("https://") {
                    if !urls.contains(&url) {
                        urls.push(url);
                    }
                }
            }
        }
    }

    // Also attempt parsing as File to catch full structured imports
    if let Ok(file) = serde_yaml::from_str::<File>(raw_yaml) {
        for imp in file.imports.iter().chain(file.graph_defn.imports.iter()) {
            if imp.from.starts_with("http://") || imp.from.starts_with("https://") {
                if !urls.contains(&imp.from) {
                    urls.push(imp.from.clone());
                }
            }
        }
    }

    urls
}

/// Merges an imported `GraphDefinition` into a base `GraphDefinition`.
/// Local elements always take precedence over imported ones.
pub fn merge_graph_definitions(
    base: &mut GraphDefinition,
    imported: GraphDefinition,
    filter: Option<&[String]>,
    alias: Option<&str>,
) {
    // 1. Merge graph_attrs if base still has default values
    let default_attrs = crate::parser::graphv2::GraphAttrs::default();
    if base.graph_attrs.background == default_attrs.background
        && imported.graph_attrs.background != default_attrs.background
    {
        base.graph_attrs.background = imported.graph_attrs.background;
    }
    if base.graph_attrs.connection_color == default_attrs.connection_color
        && imported.graph_attrs.connection_color != default_attrs.connection_color
    {
        base.graph_attrs.connection_color = imported.graph_attrs.connection_color;
    }
    if base.graph_attrs.text_color == default_attrs.text_color
        && imported.graph_attrs.text_color != default_attrs.text_color
    {
        base.graph_attrs.text_color = imported.graph_attrs.text_color;
    }
    if base.graph_attrs.title.is_empty() && !imported.graph_attrs.title.is_empty() {
        base.graph_attrs.title = imported.graph_attrs.title;
    }
    if base.graph_attrs.connector_style == default_attrs.connector_style
        && imported.graph_attrs.connector_style != default_attrs.connector_style
    {
        base.graph_attrs.connector_style = imported.graph_attrs.connector_style;
    }
    if base.graph_attrs.message_theme.is_none() && imported.graph_attrs.message_theme.is_some() {
        base.graph_attrs.message_theme = imported.graph_attrs.message_theme;
    }
    if base.graph_attrs.explain_theme.is_none() && imported.graph_attrs.explain_theme.is_some() {
        base.graph_attrs.explain_theme = imported.graph_attrs.explain_theme;
    }
    if base.graph_attrs.font.is_none() && imported.graph_attrs.font.is_some() {
        base.graph_attrs.font = imported.graph_attrs.font;
    }

    // 2. Merge icons (deduplicated by id)
    for icon in imported.icons {
        if !base.icons.iter().any(|i| i.id == icon.id) {
            base.icons.push(icon);
        }
    }

    // 3. Merge node_templates (deduplicated by id)
    for tmpl in imported.node_templates {
        if !base.node_templates.iter().any(|t| t.id == tmpl.id) {
            base.node_templates.push(tmpl);
        }
    }

    // 4. Merge node_types (respecting filter & local override precedence)
    for mut nt in imported.node_types {
        if let Some(flt) = filter {
            if !flt.iter().any(|f| f == &nt.id) {
                continue;
            }
        }
        if let Some(al) = alias {
            if filter.map_or(true, |f| f.len() == 1) {
                nt.id = al.to_string();
            } else {
                nt.id = format!("{}_{}", al, nt.id);
            }
        }
        if let Some(existing) = base.node_types.iter_mut().find(|n| n.id == nt.id) {
            if existing.func.as_ref().map_or(true, |s| s.trim().is_empty()) {
                existing.func = nt.func;
            }
        } else {
            base.node_types.push(nt);
        }
    }

    // 5. Merge graph connections if base doesn't have an instance with the same name
    for conn in imported.graph {
        if !base.graph.iter().any(|c| c.name == conn.name) {
            base.graph.push(conn);
        }
    }

    // 6. Merge layout if base does not have one
    if base.layout.is_none() && imported.layout.is_some() {
        base.layout = imported.layout;
    }

    // 7. Merge groups (deduplicated by id)
    for group in imported.groups {
        if !base.groups.iter().any(|g| g.id == group.id) {
            base.groups.push(group);
        }
    }
}

fn resolve_file_recursive(
    raw_yaml: &str,
    external_sources: &HashMap<String, String>,
    depth: usize,
) -> Result<File, serde_yaml::Error> {
    use serde::de::Error as SerdeError;
    if depth > 8 {
        return Err(serde_yaml::Error::custom(
            "Cyclic or too deeply nested imports",
        ));
    }

    let mut file: File = serde_yaml::from_str(raw_yaml)?;

    // Collect all imports: theme shorthand + top-level imports + graph_defn.imports
    let mut all_imports = Vec::new();

    // 1. Theme shorthand
    let theme_opt = file.theme.clone().or_else(|| file.graph_defn.theme.clone());
    if let Some(theme_name) = theme_opt {
        all_imports.push(ImportDef {
            from: theme_name,
            import: None,
            r#as: None,
        });
    }

    // 2. Explicit imports
    for imp in file.imports.clone() {
        all_imports.push(imp);
    }
    for imp in file.graph_defn.imports.clone() {
        all_imports.push(imp);
    }

    // Base definition starts with local file's graph_defn
    let mut merged_graph_defn = file.graph_defn.clone();

    // Process each import in sequence
    for imp in all_imports {
        let source_yaml: String = if let Some(preset_str) = get_builtin_preset(&imp.from) {
            preset_str.to_string()
        } else if let Some(fetched) = external_sources.get(&imp.from) {
            fetched.clone()
        } else if imp.from.starts_with("http://") || imp.from.starts_with("https://") {
            return Err(serde_yaml::Error::custom(format!(
                "Remote import '{}' is not yet loaded or failed to fetch",
                imp.from
            )));
        } else {
            return Err(serde_yaml::Error::custom(format!(
                "Unknown import or preset '{}'",
                imp.from
            )));
        };

        // Recursively resolve imported YAML
        let imported_file = resolve_file_recursive(&source_yaml, external_sources, depth + 1)?;
        let filter_slice = imp.import.as_deref();
        merge_graph_definitions(
            &mut merged_graph_defn,
            imported_file.graph_defn,
            filter_slice,
            imp.r#as.as_deref(),
        );
    }

    file.graph_defn = merged_graph_defn;
    Ok(file)
}

/// Resolves all imports (both built-in presets and external URLs) for a given raw YAML string.
pub fn resolve_file_imports(
    raw_yaml: &str,
    external_sources: &HashMap<String, String>,
) -> Result<File, serde_yaml::Error> {
    resolve_file_recursive(raw_yaml, external_sources, 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::graphv2::{parse_graph2, parse_graph2_with_sources, MessageBubbleShape};

    #[test]
    fn test_builtin_theme_cyberpunk() {
        let yaml = r#"
theme: cyberpunk
graph_defn:
  graph:
    - name: node_a
      node_type: client
      links: []
  node_types:
    - id: client
      attrs:
        ticks: 2
"#
        .to_string();

        let file = parse_graph2(&yaml).expect("Should parse with cyberpunk theme");
        assert_eq!(
            file.graph_defn
                .graph_attrs
                .message_theme
                .as_ref()
                .unwrap()
                .shape,
            MessageBubbleShape::Chamfered
        );
        assert!(file
            .graph_defn
            .node_templates
            .iter()
            .any(|t| t.id == "cyber_hud"));
        assert_eq!(file.graph_defn.node_instances.len(), 1);
    }

    #[test]
    fn test_theme_switching_template_cross_compatibility() {
        let yaml_cyber = r#"
theme: cyberpunk
graph_defn:
  graph:
    - name: n1
      node_type: t1
      links: []
  node_types:
    - id: t1
      attrs:
        template_ref: cyber_hud
"#;
        assert!(parse_graph2(&yaml_cyber.to_string()).is_ok());

        let yaml_cloud = r#"
theme: cloud
graph_defn:
  graph:
    - name: n1
      node_type: t1
      links: []
  node_types:
    - id: t1
      attrs:
        template_ref: cloud_card
"#;
        assert!(parse_graph2(&yaml_cloud.to_string()).is_ok());

        let yaml_datacenter = r#"
theme: datacenter
graph_defn:
  graph:
    - name: n1
      node_type: t1
      links: []
  node_types:
    - id: t1
      attrs:
        template_ref: rack_blade
"#;
        assert!(parse_graph2(&yaml_datacenter.to_string()).is_ok());

        let yaml_minimal = r#"
theme: minimal
graph_defn:
  graph:
    - name: n1
      node_type: t1
      links: []
  node_types:
    - id: t1
      attrs:
        template_ref: minimal_pill
"#;
        assert!(parse_graph2(&yaml_minimal.to_string()).is_ok());
    }

    #[test]
    fn test_builtin_stdlib_load_balancer() {
        let yaml = r#"
imports:
  - from: "stdlib:load_balancer"
graph_defn:
  graph:
    - name: lb
      node_type: round_robin_lb
      links: ["srv1"]
    - name: srv1
      node_type: server
      links: []
  node_types:
    - id: server
      attrs:
        ticks: 1
"#
        .to_string();

        let file = parse_graph2(&yaml).expect("Should parse with stdlib load balancer");
        assert_eq!(file.graph_defn.node_instances.len(), 2);
        assert!(file
            .graph_defn
            .node_types
            .iter()
            .any(|t| t.id == "round_robin_lb"));
        assert!(file
            .graph_defn
            .node_types
            .iter()
            .any(|t| t.id == "weighted_lb"));
    }

    #[test]
    fn test_selective_import_and_aliasing() {
        let yaml = r#"
imports:
  - from: "stdlib:load_balancer"
    import: ["round_robin_lb"]
    as: "my_lb"
graph_defn:
  graph:
    - name: lb
      node_type: my_lb
      links: []
"#
        .to_string();

        let file = parse_graph2(&yaml).expect("Should parse with aliased selective import");
        assert!(file.graph_defn.node_types.iter().any(|t| t.id == "my_lb"));
        assert!(!file
            .graph_defn
            .node_types
            .iter()
            .any(|t| t.id == "weighted_lb"));
        assert_eq!(file.graph_defn.node_instances[0].node_data.id, "my_lb");
    }

    #[test]
    fn test_remote_url_scanning_and_resolution() {
        let yaml = r#"
imports:
  - from: "https://example.com/custom_nodes.yml"
graph_defn:
  graph:
    - name: worker
      node_type: remote_worker
      links: []
"#
        .to_string();

        let urls = scan_import_urls(&yaml);
        assert_eq!(
            urls,
            vec!["https://example.com/custom_nodes.yml".to_string()]
        );

        let remote_content = r#"
graph_defn:
  node_types:
    - id: remote_worker
      attrs:
        ticks: 5
      fn: |
        fn on_init() { log("Remote worker ready"); }
"#;
        let mut sources = HashMap::new();
        sources.insert(
            "https://example.com/custom_nodes.yml".to_string(),
            remote_content.to_string(),
        );

        let file =
            parse_graph2_with_sources(&yaml, &sources).expect("Should parse with remote sources");
        assert_eq!(file.graph_defn.node_instances.len(), 1);
        assert_eq!(
            file.graph_defn.node_instances[0].node_data.id,
            "remote_worker"
        );
    }

    #[test]
    fn test_local_override_precedence() {
        let yaml = r#"
theme: cyberpunk
graph_defn:
  graph_attrs:
    title: "My Custom Cyberpunk Graph"
  node_types:
    - id: round_robin_lb
      attrs:
        ticks: 99
imports:
  - from: "stdlib:load_balancer"
"#
        .to_string();

        let file = parse_graph2(&yaml).expect("Should parse with override");
        assert_eq!(
            file.graph_defn.graph_attrs.title,
            "My Custom Cyberpunk Graph"
        );
        let lb_type = file
            .graph_defn
            .node_types
            .iter()
            .find(|t| t.id == "round_robin_lb")
            .unwrap();
        assert_eq!(
            lb_type.attrs.ticks,
            crate::parser::graphv2::Ticks::Fixed(99)
        );
        assert!(
            lb_type.func.is_some(),
            "Should inherit func from stdlib:load_balancer"
        );
    }

    #[test]
    fn test_all_builtin_presets_validity() {
        let presets = [
            "plibs:themes/cyberpunk",
            "plibs:themes/cloud",
            "plibs:themes/datacenter",
            "plibs:themes/minimal",
            "plibs:themes/synthwave",
            "plibs:themes/nordic",
            "plibs:themes/dracula",
            "plibs:themes/matrix",
            "plibs:themes/solarized_light",
            "theme:cyberpunk",
            "theme:cloud",
            "theme:datacenter",
            "theme:minimal",
            "theme:synthwave",
            "theme:nordic",
            "theme:dracula",
            "theme:matrix",
            "theme:solarized_light",
            "plibs:aws/compute",
            "plibs:aws/networking",
            "plibs:aws/database",
            "plibs:aws/messaging",
            "plibs:aws/all",
            "stdlib:load_balancer",
            "stdlib:circuit_breaker",
            "stdlib:cache",
        ];

        for preset in presets {
            let yaml = format!("imports:\n  - from: \"{}\"\n", preset);
            let file = parse_graph2(&yaml)
                .unwrap_or_else(|e| panic!("Preset '{}' failed to parse: {}", preset, e));
            let _ = file;
        }
    }

    #[test]
    fn test_all_builtin_themes_have_explain_theme() {
        let themes = [
            (
                "plibs:themes/cyberpunk",
                crate::parser::graphv2::MessageBubbleShape::Chamfered,
            ),
            (
                "plibs:themes/cloud",
                crate::parser::graphv2::MessageBubbleShape::Rounded,
            ),
            (
                "plibs:themes/datacenter",
                crate::parser::graphv2::MessageBubbleShape::Box,
            ),
            (
                "plibs:themes/minimal",
                crate::parser::graphv2::MessageBubbleShape::Pill,
            ),
            (
                "plibs:themes/synthwave",
                crate::parser::graphv2::MessageBubbleShape::Chamfered,
            ),
            (
                "plibs:themes/nordic",
                crate::parser::graphv2::MessageBubbleShape::Rounded,
            ),
            (
                "plibs:themes/dracula",
                crate::parser::graphv2::MessageBubbleShape::Rounded,
            ),
            (
                "plibs:themes/matrix",
                crate::parser::graphv2::MessageBubbleShape::Box,
            ),
            (
                "plibs:themes/solarized_light",
                crate::parser::graphv2::MessageBubbleShape::Pill,
            ),
        ];

        for (theme_name, expected_shape) in themes {
            let yaml = format!("imports:\n  - from: \"{}\"\n", theme_name);
            let file = parse_graph2(&yaml)
                .unwrap_or_else(|e| panic!("Theme '{}' failed to parse: {}", theme_name, e));
            let explain_theme = file
                .graph_defn
                .graph_attrs
                .explain_theme
                .unwrap_or_else(|| panic!("Theme '{}' missing explain_theme", theme_name));
            assert_eq!(
                explain_theme.shape, expected_shape,
                "Theme '{}' had unexpected explain_theme shape",
                theme_name
            );
            assert!(
                explain_theme.bg.is_some(),
                "Theme '{}' missing bg",
                theme_name
            );
            assert!(
                explain_theme.border.is_some(),
                "Theme '{}' missing border",
                theme_name
            );
            assert!(
                explain_theme.accent.is_some(),
                "Theme '{}' missing accent",
                theme_name
            );
        }
    }

    #[test]
    fn test_all_builtin_themes_have_shared_node_template() {
        // Every theme preset must register a template literally named "node"
        // (alongside its own specialized one, e.g. cyber_hud/cloud_card) that
        // uses only params with global fallback defaults
        // (`node_name`/`icon`/`role_tag`/`status_bg`/`status_color`/
        // `status_text`/`accent_color` - see `template_params()` in
        // graphv2.rs) - this is what lets a node type declare
        // `template_ref: node` once and keep rendering no matter which theme
        // gets swapped in, instead of failing to compile the way
        // `template_ref: minimal_pill` does under a non-minimal theme.
        let themes = [
            "plibs:themes/cyberpunk",
            "plibs:themes/cloud",
            "plibs:themes/datacenter",
            "plibs:themes/minimal",
            "plibs:themes/synthwave",
            "plibs:themes/nordic",
            "plibs:themes/dracula",
            "plibs:themes/matrix",
            "plibs:themes/solarized_light",
        ];
        for theme_name in themes {
            let yaml = format!(
                "imports:\n  - from: \"{}\"\ngraph_defn:\n  graph:\n    - name: n1\n      node_type: t1\n      links: []\n  node_types:\n    - id: t1\n      attrs:\n        template_ref: node\n",
                theme_name
            );
            let file = parse_graph2(&yaml).unwrap_or_else(|e| {
                panic!(
                    "Theme '{}' failed to compile with template_ref: node: {}",
                    theme_name, e
                )
            });
            assert!(
                file.graph_defn
                    .node_templates
                    .iter()
                    .any(|t| t.id == "node"),
                "Theme '{}' did not register a 'node' template",
                theme_name
            );
        }
    }

    #[test]
    fn test_aws_all_icons_and_cloud_theme() {
        let yaml = r#"
imports:
  - from: "plibs:themes/cloud"
  - from: "plibs:aws/all"

graph_defn:
  node_types:
    - id: user_client
      attrs:
        ticks: 2
        template_ref: cloud_card
        icon: aws_alb
        params:
          icon: "aws_alb"
  graph:
    - name: client
      node_type: user_client
      links: []
"#;
        let file = parse_graph2(&yaml.to_string()).expect("Should parse aws all");
        assert!(file.graph_defn.icons.iter().any(|i| i.id == "aws_lambda"));
        assert!(file.graph_defn.icons.iter().any(|i| i.id == "aws_ec2"));
        assert!(file.graph_defn.icons.iter().any(|i| i.id == "aws_api_gw"));
        assert!(file.graph_defn.icons.iter().any(|i| i.id == "aws_alb"));
        assert!(file.graph_defn.icons.iter().any(|i| i.id == "aws_dynamodb"));
        assert!(file.graph_defn.icons.iter().any(|i| i.id == "aws_sqs"));
        assert!(file
            .graph_defn
            .node_templates
            .iter()
            .any(|t| t.id == "cloud_card"));
        let lambda_node = file
            .graph_defn
            .node_types
            .iter()
            .find(|t| t.id == "lambda_func")
            .unwrap();
        assert_eq!(
            lambda_node.attrs.template_ref,
            Some("cloud_card".to_string())
        );
        assert!(lambda_node.attrs.template_params.is_some());
        let params = lambda_node.attrs.template_params.as_ref().unwrap();
        assert_eq!(params.get("role").map(|s| s.as_str()), Some("AWS Lambda"));
        assert_eq!(params.get("icon").map(|s| s.as_str()), Some("aws_lambda"));

        let dynamo_node = file
            .graph_defn
            .node_types
            .iter()
            .find(|t| t.id == "dynamodb")
            .unwrap();
        assert_eq!(
            dynamo_node.attrs.template_ref,
            Some("cloud_card".to_string())
        );
        let dynamo_params = dynamo_node.attrs.template_params.as_ref().unwrap();
        assert_eq!(
            dynamo_params.get("role").map(|s| s.as_str()),
            Some("Amazon DynamoDB")
        );
        assert_eq!(
            dynamo_params.get("icon").map(|s| s.as_str()),
            Some("aws_dynamodb")
        );
    }
}
