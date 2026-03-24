//! Declarative render graph: named passes, resource nodes, dependency edges,
//! topological sort, cycle detection, conditional passes, multi-resolution
//! passes, validation, merging, and DOT export.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;

use crate::rendergraph::resources::{
    ResourceDescriptor, ResourceHandle, ResourceLifetime, ResourceTable, SizePolicy, TextureFormat,
};

// ---------------------------------------------------------------------------
// Pass / node types
// ---------------------------------------------------------------------------

/// The kind of work a render pass performs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassType {
    Graphics,
    Compute,
    Transfer,
    Present,
}

/// Queue hint for the executor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QueueAffinity {
    Graphics,
    Compute,
    Transfer,
    Any,
}

/// Condition that controls whether a pass executes.
#[derive(Debug, Clone)]
pub enum PassCondition {
    /// Always execute.
    Always,
    /// Execute only when the named feature is enabled.
    FeatureEnabled(String),
    /// Execute only when a boolean callback returns true.
    Callback(String), // name of the callback — actual fn stored externally
    /// Combine conditions with AND.
    All(Vec<PassCondition>),
    /// Combine conditions with OR.
    Any(Vec<PassCondition>),
}

impl PassCondition {
    /// Evaluate the condition against a set of enabled features and named booleans.
    pub fn evaluate(&self, features: &HashSet<String>, callbacks: &HashMap<String, bool>) -> bool {
        match self {
            Self::Always => true,
            Self::FeatureEnabled(name) => features.contains(name),
            Self::Callback(name) => callbacks.get(name).copied().unwrap_or(false),
            Self::All(conds) => conds.iter().all(|c| c.evaluate(features, callbacks)),
            Self::Any(conds) => conds.iter().any(|c| c.evaluate(features, callbacks)),
        }
    }
}

/// Resolution multiplier for a pass (relative to its declared resource sizes).
#[derive(Debug, Clone, Copy)]
pub struct ResolutionScale {
    pub width_scale: f32,
    pub height_scale: f32,
}

impl ResolutionScale {
    pub fn full() -> Self {
        Self {
            width_scale: 1.0,
            height_scale: 1.0,
        }
    }
    pub fn half() -> Self {
        Self {
            width_scale: 0.5,
            height_scale: 0.5,
        }
    }
    pub fn quarter() -> Self {
        Self {
            width_scale: 0.25,
            height_scale: 0.25,
        }
    }
    pub fn custom(w: f32, h: f32) -> Self {
        Self {
            width_scale: w,
            height_scale: h,
        }
    }
}

// ---------------------------------------------------------------------------
// Render pass node
// ---------------------------------------------------------------------------

/// A named node in the render graph representing a render pass.
#[derive(Debug, Clone)]
pub struct RenderPass {
    pub name: String,
    pub pass_type: PassType,
    pub queue: QueueAffinity,
    pub condition: PassCondition,
    pub resolution: ResolutionScale,
    /// Resource handles this pass reads.
    pub inputs: Vec<ResourceHandle>,
    /// Resource handles this pass writes.
    pub outputs: Vec<ResourceHandle>,
    /// Names of input resources (for serialization / debug).
    pub input_names: Vec<String>,
    /// Names of output resources.
    pub output_names: Vec<String>,
    /// Explicit ordering dependencies (pass names that must run before this).
    pub explicit_deps: Vec<String>,
    /// Whether this pass has side effects (e.g., writes to swapchain).
    pub has_side_effects: bool,
    /// User-attached tag for grouping.
    pub tag: Option<String>,
}

impl RenderPass {
    pub fn new(name: &str, pass_type: PassType) -> Self {
        Self {
            name: name.to_string(),
            pass_type,
            queue: QueueAffinity::Graphics,
            condition: PassCondition::Always,
            resolution: ResolutionScale::full(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            input_names: Vec::new(),
            output_names: Vec::new(),
            explicit_deps: Vec::new(),
            has_side_effects: false,
            tag: None,
        }
    }

    pub fn with_queue(mut self, queue: QueueAffinity) -> Self {
        self.queue = queue;
        self
    }

    pub fn with_condition(mut self, condition: PassCondition) -> Self {
        self.condition = condition;
        self
    }

    pub fn with_resolution(mut self, scale: ResolutionScale) -> Self {
        self.resolution = scale;
        self
    }

    pub fn with_side_effects(mut self) -> Self {
        self.has_side_effects = true;
        self
    }

    pub fn with_tag(mut self, tag: &str) -> Self {
        self.tag = Some(tag.to_string());
        self
    }

    pub fn add_input(&mut self, handle: ResourceHandle, name: &str) {
        self.inputs.push(handle);
        self.input_names.push(name.to_string());
    }

    pub fn add_output(&mut self, handle: ResourceHandle, name: &str) {
        self.outputs.push(handle);
        self.output_names.push(name.to_string());
    }

    pub fn depends_on(&mut self, pass_name: &str) {
        if !self.explicit_deps.contains(&pass_name.to_string()) {
            self.explicit_deps.push(pass_name.to_string());
        }
    }

    /// True if this pass can potentially run on the async compute queue.
    pub fn is_async_compute_candidate(&self) -> bool {
        self.pass_type == PassType::Compute && self.queue != QueueAffinity::Graphics
    }
}

// ---------------------------------------------------------------------------
// Resource node
// ---------------------------------------------------------------------------

/// A resource node in the graph. Resources are vertices connected to passes
/// via read/write edges.
#[derive(Debug, Clone)]
pub struct ResourceNode {
    pub name: String,
    pub handle: ResourceHandle,
    pub descriptor: ResourceDescriptor,
    pub lifetime: ResourceLifetime,
    /// Pass that produces this resource (if any).
    pub producer: Option<String>,
    /// Passes that consume this resource.
    pub consumers: Vec<String>,
}

impl ResourceNode {
    pub fn new(name: &str, handle: ResourceHandle, descriptor: ResourceDescriptor, lifetime: ResourceLifetime) -> Self {
        Self {
            name: name.to_string(),
            handle,
            descriptor,
            lifetime,
            producer: None,
            consumers: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Dependency edge
// ---------------------------------------------------------------------------

/// An edge in the render graph, connecting a producer pass to a consumer pass
/// through a shared resource.
#[derive(Debug, Clone)]
pub struct PassDependency {
    pub from_pass: String,
    pub to_pass: String,
    pub resource: String,
    pub kind: DependencyKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyKind {
    /// Consumer reads resource written by producer.
    ReadAfterWrite,
    /// Consumer writes resource previously written by producer (execution ordering).
    WriteAfterWrite,
    /// Consumer writes resource previously read by producer.
    WriteAfterRead,
    /// Explicit ordering dependency (no resource involved).
    Explicit,
}

impl fmt::Display for PassDependency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} -> {} (via '{}', {:?})",
            self.from_pass, self.to_pass, self.resource, self.kind
        )
    }
}

// ---------------------------------------------------------------------------
// Validation result
// ---------------------------------------------------------------------------

/// Result of validating a render graph.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl ValidationResult {
    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn error(&mut self, msg: impl Into<String>) {
        self.errors.push(msg.into());
    }

    pub fn warning(&mut self, msg: impl Into<String>) {
        self.warnings.push(msg.into());
    }
}

impl Default for ValidationResult {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ValidationResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_ok() {
            write!(f, "Validation OK")?;
        } else {
            write!(f, "Validation FAILED ({} errors)", self.errors.len())?;
        }
        for e in &self.errors {
            write!(f, "\n  ERROR: {}", e)?;
        }
        for w in &self.warnings {
            write!(f, "\n  WARN:  {}", w)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Render graph
// ---------------------------------------------------------------------------

/// A declarative render graph: collection of passes (nodes) connected via
/// resource dependencies (edges). Supports topological sorting, cycle
/// detection, validation, conditional execution, and DOT export.
pub struct RenderGraph {
    /// All render passes, keyed by name.
    passes: HashMap<String, RenderPass>,
    /// Insertion order of passes.
    pass_order: Vec<String>,
    /// All resource nodes, keyed by name.
    resource_nodes: HashMap<String, ResourceNode>,
    /// Computed dependency edges.
    edges: Vec<PassDependency>,
    /// Topologically sorted pass names (computed lazily).
    sorted_passes: Vec<String>,
    /// Whether the sorted order is stale.
    dirty: bool,
    /// Resource table for handle bookkeeping.
    pub resource_table: ResourceTable,
    /// Enabled features for conditional passes.
    features: HashSet<String>,
    /// Named boolean callbacks for conditional passes.
    callback_values: HashMap<String, bool>,
    /// Graph label (for DOT export).
    label: String,
}

impl RenderGraph {
    pub fn new(label: &str) -> Self {
        Self {
            passes: HashMap::new(),
            pass_order: Vec::new(),
            resource_nodes: HashMap::new(),
            edges: Vec::new(),
            sorted_passes: Vec::new(),
            dirty: true,
            resource_table: ResourceTable::new(),
            features: HashSet::new(),
            callback_values: HashMap::new(),
            label: label.to_string(),
        }
    }

    // -- Feature / condition API ------------------------------------------

    pub fn enable_feature(&mut self, feature: &str) {
        self.features.insert(feature.to_string());
    }

    pub fn disable_feature(&mut self, feature: &str) {
        self.features.remove(feature);
    }

    pub fn is_feature_enabled(&self, feature: &str) -> bool {
        self.features.contains(feature)
    }

    pub fn set_callback(&mut self, name: &str, value: bool) {
        self.callback_values.insert(name.to_string(), value);
    }

    // -- Resource declaration API -----------------------------------------

    /// Declare a transient texture resource.
    pub fn declare_resource(&mut self, descriptor: ResourceDescriptor) -> ResourceHandle {
        let name = descriptor.name.clone();
        let handle = self.resource_table.declare_transient(descriptor.clone());
        self.resource_nodes
            .entry(name.clone())
            .or_insert_with(|| ResourceNode::new(&name, handle, descriptor, ResourceLifetime::Transient));
        self.dirty = true;
        handle
    }

    /// Declare an imported (externally managed) resource.
    pub fn import_resource(&mut self, descriptor: ResourceDescriptor) -> ResourceHandle {
        let name = descriptor.name.clone();
        let handle = self.resource_table.declare_imported(descriptor.clone());
        self.resource_nodes
            .entry(name.clone())
            .or_insert_with(|| ResourceNode::new(&name, handle, descriptor, ResourceLifetime::Imported));
        self.dirty = true;
        handle
    }

    // -- Pass API ---------------------------------------------------------

    /// Add a render pass to the graph.
    pub fn add_pass(&mut self, pass: RenderPass) {
        let name = pass.name.clone();
        // Update resource table writers/readers
        for (h, rname) in pass.outputs.iter().zip(pass.output_names.iter()) {
            self.resource_table.add_writer(*h, &name);
            if let Some(rn) = self.resource_nodes.get_mut(rname) {
                rn.producer = Some(name.clone());
            }
        }
        for (h, rname) in pass.inputs.iter().zip(pass.input_names.iter()) {
            self.resource_table.add_reader(*h, &name);
            if let Some(rn) = self.resource_nodes.get_mut(rname) {
                if !rn.consumers.contains(&name) {
                    rn.consumers.push(name.clone());
                }
            }
        }
        if !self.pass_order.contains(&name) {
            self.pass_order.push(name.clone());
        }
        self.passes.insert(name, pass);
        self.dirty = true;
    }

    /// Remove a pass by name.
    pub fn remove_pass(&mut self, name: &str) -> Option<RenderPass> {
        self.pass_order.retain(|n| n != name);
        self.dirty = true;
        self.passes.remove(name)
    }

    /// Get a pass by name.
    pub fn get_pass(&self, name: &str) -> Option<&RenderPass> {
        self.passes.get(name)
    }

    /// Get a mutable pass by name.
    pub fn get_pass_mut(&mut self, name: &str) -> Option<&mut RenderPass> {
        self.dirty = true;
        self.passes.get_mut(name)
    }

    /// All pass names in insertion order.
    pub fn pass_names(&self) -> &[String] {
        &self.pass_order
    }

    /// Number of passes.
    pub fn pass_count(&self) -> usize {
        self.passes.len()
    }

    /// Number of resource nodes.
    pub fn resource_count(&self) -> usize {
        self.resource_nodes.len()
    }

    // -- Dependency building ----------------------------------------------

    /// Rebuild dependency edges from resource read/write declarations.
    pub fn build_edges(&mut self) {
        self.edges.clear();

        // For each resource, connect producer -> consumers
        for (res_name, rn) in &self.resource_nodes {
            if let Some(ref producer) = rn.producer {
                for consumer in &rn.consumers {
                    if producer != consumer {
                        self.edges.push(PassDependency {
                            from_pass: producer.clone(),
                            to_pass: consumer.clone(),
                            resource: res_name.clone(),
                            kind: DependencyKind::ReadAfterWrite,
                        });
                    }
                }
            }
        }

        // Explicit dependencies
        let pass_names: Vec<String> = self.passes.keys().cloned().collect();
        for name in &pass_names {
            let deps = self.passes[name].explicit_deps.clone();
            for dep in deps {
                if self.passes.contains_key(&dep) {
                    self.edges.push(PassDependency {
                        from_pass: dep,
                        to_pass: name.clone(),
                        resource: String::new(),
                        kind: DependencyKind::Explicit,
                    });
                }
            }
        }
    }

    /// Get all edges.
    pub fn edges(&self) -> &[PassDependency] {
        &self.edges
    }

    // -- Topological sort / cycle detection --------------------------------

    /// Detect cycles in the dependency graph. Returns the set of passes
    /// involved in a cycle, or empty if acyclic.
    pub fn detect_cycles(&mut self) -> Vec<Vec<String>> {
        self.build_edges();

        let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
        for name in self.passes.keys() {
            adj.entry(name.as_str()).or_default();
        }
        for edge in &self.edges {
            adj.entry(edge.from_pass.as_str())
                .or_default()
                .push(edge.to_pass.as_str());
        }

        // Tarjan's SCC
        let mut index_counter: u32 = 0;
        let mut stack: Vec<&str> = Vec::new();
        let mut on_stack: HashSet<&str> = HashSet::new();
        let mut indices: HashMap<&str, u32> = HashMap::new();
        let mut lowlinks: HashMap<&str, u32> = HashMap::new();
        let mut sccs: Vec<Vec<String>> = Vec::new();

        fn strongconnect<'a>(
            v: &'a str,
            adj: &HashMap<&'a str, Vec<&'a str>>,
            index_counter: &mut u32,
            stack: &mut Vec<&'a str>,
            on_stack: &mut HashSet<&'a str>,
            indices: &mut HashMap<&'a str, u32>,
            lowlinks: &mut HashMap<&'a str, u32>,
            sccs: &mut Vec<Vec<String>>,
        ) {
            indices.insert(v, *index_counter);
            lowlinks.insert(v, *index_counter);
            *index_counter += 1;
            stack.push(v);
            on_stack.insert(v);

            if let Some(neighbors) = adj.get(v) {
                for &w in neighbors {
                    if !indices.contains_key(w) {
                        strongconnect(w, adj, index_counter, stack, on_stack, indices, lowlinks, sccs);
                        let lw = lowlinks[w];
                        let lv = lowlinks[v];
                        lowlinks.insert(v, lv.min(lw));
                    } else if on_stack.contains(w) {
                        let iw = indices[w];
                        let lv = lowlinks[v];
                        lowlinks.insert(v, lv.min(iw));
                    }
                }
            }

            if lowlinks[v] == indices[v] {
                let mut scc = Vec::new();
                while let Some(w) = stack.pop() {
                    on_stack.remove(w);
                    scc.push(w.to_string());
                    if w == v {
                        break;
                    }
                }
                if scc.len() > 1 {
                    sccs.push(scc);
                }
            }
        }

        let nodes: Vec<&str> = adj.keys().copied().collect();
        for node in nodes {
            if !indices.contains_key(node) {
                strongconnect(
                    node,
                    &adj,
                    &mut index_counter,
                    &mut stack,
                    &mut on_stack,
                    &mut indices,
                    &mut lowlinks,
                    &mut sccs,
                );
            }
        }

        sccs
    }

    /// Perform topological sort using Kahn's algorithm.
    /// Returns `Err` with cycle participants if a cycle is detected.
    pub fn topological_sort(&mut self) -> Result<Vec<String>, Vec<String>> {
        self.build_edges();

        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        for name in self.passes.keys() {
            in_degree.entry(name.as_str()).or_insert(0);
        }
        let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
        for edge in &self.edges {
            adj.entry(edge.from_pass.as_str())
                .or_default()
                .push(edge.to_pass.as_str());
            *in_degree.entry(edge.to_pass.as_str()).or_insert(0) += 1;
        }

        let mut queue: VecDeque<&str> = VecDeque::new();
        for (&node, &deg) in &in_degree {
            if deg == 0 {
                queue.push_back(node);
            }
        }

        // Sort the initial queue by pass insertion order for determinism
        let order_map: HashMap<&str, usize> = self
            .pass_order
            .iter()
            .enumerate()
            .map(|(i, n)| (n.as_str(), i))
            .collect();
        let mut initial: Vec<&str> = queue.drain(..).collect();
        initial.sort_by_key(|n| order_map.get(n).copied().unwrap_or(usize::MAX));
        for n in initial {
            queue.push_back(n);
        }

        let mut sorted: Vec<String> = Vec::new();
        let mut visited = 0usize;

        while let Some(node) = queue.pop_front() {
            sorted.push(node.to_string());
            visited += 1;
            if let Some(neighbors) = adj.get(node) {
                // Collect and sort neighbors for determinism
                let mut next: Vec<&str> = Vec::new();
                for &nb in neighbors {
                    let deg = in_degree.get_mut(nb).unwrap();
                    *deg -= 1;
                    if *deg == 0 {
                        next.push(nb);
                    }
                }
                next.sort_by_key(|n| order_map.get(n).copied().unwrap_or(usize::MAX));
                for nb in next {
                    queue.push_back(nb);
                }
            }
        }

        if visited != self.passes.len() {
            // Cycle: return nodes not in sorted
            let sorted_set: HashSet<&str> = sorted.iter().map(|s| s.as_str()).collect();
            let cycle_nodes: Vec<String> = self
                .passes
                .keys()
                .filter(|k| !sorted_set.contains(k.as_str()))
                .cloned()
                .collect();
            return Err(cycle_nodes);
        }

        self.sorted_passes = sorted.clone();
        self.dirty = false;
        Ok(sorted)
    }

    /// Get the sorted pass list (computes if dirty).
    pub fn sorted(&mut self) -> Result<&[String], Vec<String>> {
        if self.dirty {
            self.topological_sort()?;
        }
        Ok(&self.sorted_passes)
    }

    /// Filter sorted passes to only those whose conditions are satisfied.
    pub fn active_passes(&mut self) -> Result<Vec<String>, Vec<String>> {
        let sorted = self.topological_sort()?;
        let features = &self.features;
        let callbacks = &self.callback_values;
        Ok(sorted
            .into_iter()
            .filter(|name| {
                self.passes
                    .get(name)
                    .map(|p| p.condition.evaluate(features, callbacks))
                    .unwrap_or(false)
            })
            .collect())
    }

    // -- Validation -------------------------------------------------------

    /// Validate the graph: check for cycles, dangling resources, disconnected
    /// inputs, and other structural issues.
    pub fn validate(&mut self) -> ValidationResult {
        let mut result = ValidationResult::new();

        // 1. Cycle detection
        let cycles = self.detect_cycles();
        for cycle in &cycles {
            result.error(format!("Cycle detected involving passes: {}", cycle.join(", ")));
        }

        // 2. Dangling resources
        let dangling = self.resource_table.find_dangling();
        for d in &dangling {
            match d.kind {
                crate::rendergraph::resources::DanglingKind::NeverWritten => {
                    result.error(format!("Resource '{}' is never written by any pass", d.name));
                }
                crate::rendergraph::resources::DanglingKind::NeverRead => {
                    result.warning(format!("Resource '{}' is never read by any pass", d.name));
                }
            }
        }

        // 3. Check all pass inputs are connected
        for pass in self.passes.values() {
            for input_name in &pass.input_names {
                if self.resource_table.lookup(input_name).is_none() {
                    result.error(format!(
                        "Pass '{}' reads resource '{}' which is not declared",
                        pass.name, input_name
                    ));
                }
            }
            for output_name in &pass.output_names {
                if self.resource_table.lookup(output_name).is_none() {
                    result.error(format!(
                        "Pass '{}' writes resource '{}' which is not declared",
                        pass.name, output_name
                    ));
                }
            }
        }

        // 4. Check explicit deps reference real passes
        for pass in self.passes.values() {
            for dep in &pass.explicit_deps {
                if !self.passes.contains_key(dep) {
                    result.error(format!(
                        "Pass '{}' depends on '{}' which does not exist",
                        pass.name, dep
                    ));
                }
            }
        }

        // 5. Warn about passes with no inputs and no outputs (possible error)
        for pass in self.passes.values() {
            if pass.inputs.is_empty() && pass.outputs.is_empty() && !pass.has_side_effects {
                result.warning(format!(
                    "Pass '{}' has no inputs, no outputs, and no side effects",
                    pass.name
                ));
            }
        }

        // 6. Multi-resolution: warn if a pass reads a resource at a different
        //    resolution than it was written.
        // (Informational only — this is valid for bloom/SSAO but might be a mistake)

        result
    }

    // -- Graph merging ----------------------------------------------------

    /// Merge another graph into this one. Passes and resources from `other`
    /// are added. Conflicting names get a prefix.
    pub fn merge(&mut self, other: &RenderGraph, prefix: &str) {
        // Merge resources
        for (name, rn) in &other.resource_nodes {
            let new_name = if self.resource_nodes.contains_key(name) {
                format!("{}_{}", prefix, name)
            } else {
                name.clone()
            };
            let mut desc = rn.descriptor.clone();
            desc.name = new_name.clone();
            let handle = if rn.lifetime == ResourceLifetime::Imported {
                self.import_resource(desc)
            } else {
                self.declare_resource(desc)
            };
            // We need to remap, but for simplicity we store the handle in the new node
            let _ = handle;
        }

        // Merge passes with remapped resource names
        for (name, pass) in &other.passes {
            let new_name = if self.passes.contains_key(name) {
                format!("{}_{}", prefix, name)
            } else {
                name.clone()
            };
            let mut new_pass = RenderPass::new(&new_name, pass.pass_type);
            new_pass.queue = pass.queue;
            new_pass.condition = pass.condition.clone();
            new_pass.resolution = pass.resolution;
            new_pass.has_side_effects = pass.has_side_effects;
            new_pass.tag = pass.tag.clone();

            // Remap input/output names
            for iname in &pass.input_names {
                let mapped = if self.resource_nodes.contains_key(iname) && other.resource_nodes.contains_key(iname) {
                    // If it existed in both, it was prefixed
                    if self.resource_nodes.contains_key(&format!("{}_{}", prefix, iname)) {
                        format!("{}_{}", prefix, iname)
                    } else {
                        iname.clone()
                    }
                } else {
                    iname.clone()
                };
                if let Some(h) = self.resource_table.lookup(&mapped) {
                    new_pass.add_input(h, &mapped);
                }
            }
            for oname in &pass.output_names {
                let mapped = if self.resource_nodes.contains_key(oname) && other.resource_nodes.contains_key(oname) {
                    if self.resource_nodes.contains_key(&format!("{}_{}", prefix, oname)) {
                        format!("{}_{}", prefix, oname)
                    } else {
                        oname.clone()
                    }
                } else {
                    oname.clone()
                };
                if let Some(h) = self.resource_table.lookup(&mapped) {
                    new_pass.add_output(h, &mapped);
                }
            }

            // Remap explicit deps
            for dep in &pass.explicit_deps {
                let mapped_dep = if self.passes.contains_key(dep) && other.passes.contains_key(dep) {
                    format!("{}_{}", prefix, dep)
                } else {
                    dep.clone()
                };
                new_pass.depends_on(&mapped_dep);
            }

            self.add_pass(new_pass);
        }

        self.dirty = true;
    }

    // -- DOT export -------------------------------------------------------

    /// Export the graph in Graphviz DOT format for debug visualization.
    pub fn export_dot(&mut self) -> String {
        // Ensure edges are built
        self.build_edges();

        let mut dot = String::new();
        dot.push_str(&format!("digraph \"{}\" {{\n", self.label));
        dot.push_str("  rankdir=LR;\n");
        dot.push_str("  node [shape=box, style=filled];\n\n");

        // Pass nodes
        dot.push_str("  // Render passes\n");
        for (name, pass) in &self.passes {
            let color = match pass.pass_type {
                PassType::Graphics => "#4a90d9",
                PassType::Compute => "#d94a4a",
                PassType::Transfer => "#4ad94a",
                PassType::Present => "#d9d94a",
            };
            let active = pass.condition.evaluate(&self.features, &self.callback_values);
            let style = if active { "filled" } else { "filled,dashed" };
            let label = format!(
                "{}\\n[{:?}]{}",
                name,
                pass.pass_type,
                if !active { " (DISABLED)" } else { "" }
            );
            dot.push_str(&format!(
                "  \"pass_{}\" [label=\"{}\", fillcolor=\"{}\", style=\"{}\", fontcolor=white];\n",
                name, label, color, style
            ));
        }

        // Resource nodes
        dot.push_str("\n  // Resources\n");
        for (name, rn) in &self.resource_nodes {
            let shape = match rn.lifetime {
                ResourceLifetime::Transient => "ellipse",
                ResourceLifetime::Imported => "diamond",
            };
            let label = format!(
                "{}\\n{:?}",
                name, rn.descriptor.format
            );
            dot.push_str(&format!(
                "  \"res_{}\" [label=\"{}\", shape={}, fillcolor=\"#e0e0e0\", fontcolor=black];\n",
                name, label, shape
            ));
        }

        // Edges: pass -> resource (writes) and resource -> pass (reads)
        dot.push_str("\n  // Edges\n");
        for pass in self.passes.values() {
            for oname in &pass.output_names {
                dot.push_str(&format!(
                    "  \"pass_{}\" -> \"res_{}\" [color=red, label=\"write\"];\n",
                    pass.name, oname
                ));
            }
            for iname in &pass.input_names {
                dot.push_str(&format!(
                    "  \"res_{}\" -> \"pass_{}\" [color=blue, label=\"read\"];\n",
                    iname, pass.name
                ));
            }
        }

        // Explicit dep edges
        for pass in self.passes.values() {
            for dep in &pass.explicit_deps {
                dot.push_str(&format!(
                    "  \"pass_{}\" -> \"pass_{}\" [style=dashed, color=gray, label=\"explicit\"];\n",
                    dep, pass.name
                ));
            }
        }

        dot.push_str("}\n");
        dot
    }

    // -- Accessors --------------------------------------------------------

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn resource_node(&self, name: &str) -> Option<&ResourceNode> {
        self.resource_nodes.get(name)
    }

    pub fn all_passes(&self) -> impl Iterator<Item = &RenderPass> {
        self.passes.values()
    }

    pub fn all_resource_nodes(&self) -> impl Iterator<Item = &ResourceNode> {
        self.resource_nodes.values()
    }

    pub fn features(&self) -> &HashSet<String> {
        &self.features
    }

    /// Returns passes grouped by tag.
    pub fn passes_by_tag(&self) -> HashMap<String, Vec<&RenderPass>> {
        let mut map: HashMap<String, Vec<&RenderPass>> = HashMap::new();
        for pass in self.passes.values() {
            let tag = pass.tag.clone().unwrap_or_else(|| "untagged".to_string());
            map.entry(tag).or_default().push(pass);
        }
        map
    }
}

impl fmt::Display for RenderGraph {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "RenderGraph '{}': {} passes, {} resources, {} edges",
            self.label,
            self.passes.len(),
            self.resource_nodes.len(),
            self.edges.len(),
        )
    }
}

// ---------------------------------------------------------------------------
// Builder helpers
// ---------------------------------------------------------------------------

/// Fluent builder for constructing a RenderGraph.
pub struct RenderGraphBuilder {
    graph: RenderGraph,
    backbuffer_width: u32,
    backbuffer_height: u32,
}

impl RenderGraphBuilder {
    pub fn new(label: &str, width: u32, height: u32) -> Self {
        Self {
            graph: RenderGraph::new(label),
            backbuffer_width: width,
            backbuffer_height: height,
        }
    }

    pub fn backbuffer_size(&self) -> (u32, u32) {
        (self.backbuffer_width, self.backbuffer_height)
    }

    /// Declare a full-resolution transient texture.
    pub fn texture(&mut self, name: &str, format: TextureFormat) -> ResourceHandle {
        let desc = ResourceDescriptor::new(name, format);
        self.graph.declare_resource(desc)
    }

    /// Declare a texture at a specific resolution scale.
    pub fn texture_scaled(
        &mut self,
        name: &str,
        format: TextureFormat,
        width_scale: f32,
        height_scale: f32,
    ) -> ResourceHandle {
        let desc = ResourceDescriptor::new(name, format).with_size(SizePolicy::Relative {
            width_scale,
            height_scale,
        });
        self.graph.declare_resource(desc)
    }

    /// Declare a texture with explicit pixel dimensions.
    pub fn texture_absolute(
        &mut self,
        name: &str,
        format: TextureFormat,
        width: u32,
        height: u32,
    ) -> ResourceHandle {
        let desc = ResourceDescriptor::new(name, format).with_size(SizePolicy::Absolute { width, height });
        self.graph.declare_resource(desc)
    }

    /// Import an external resource.
    pub fn import(&mut self, name: &str, format: TextureFormat) -> ResourceHandle {
        let desc = ResourceDescriptor::new(name, format);
        self.graph.import_resource(desc)
    }

    /// Add a graphics pass.
    pub fn graphics_pass(&mut self, name: &str) -> PassBuilder<'_> {
        PassBuilder {
            graph: &mut self.graph,
            pass: RenderPass::new(name, PassType::Graphics),
        }
    }

    /// Add a compute pass.
    pub fn compute_pass(&mut self, name: &str) -> PassBuilder<'_> {
        PassBuilder {
            graph: &mut self.graph,
            pass: RenderPass::new(name, PassType::Compute),
        }
    }

    /// Enable a feature flag.
    pub fn enable_feature(&mut self, feature: &str) -> &mut Self {
        self.graph.enable_feature(feature);
        self
    }

    /// Finalize and return the built graph.
    pub fn build(self) -> RenderGraph {
        self.graph
    }
}

/// Fluent builder for a single pass within a graph.
pub struct PassBuilder<'a> {
    graph: &'a mut RenderGraph,
    pass: RenderPass,
}

impl<'a> PassBuilder<'a> {
    pub fn reads(mut self, handle: ResourceHandle, name: &str) -> Self {
        self.pass.add_input(handle, name);
        self
    }

    pub fn writes(mut self, handle: ResourceHandle, name: &str) -> Self {
        self.pass.add_output(handle, name);
        self
    }

    pub fn depends_on(mut self, pass_name: &str) -> Self {
        self.pass.depends_on(pass_name);
        self
    }

    pub fn condition(mut self, cond: PassCondition) -> Self {
        self.pass.condition = cond;
        self
    }

    pub fn resolution(mut self, scale: ResolutionScale) -> Self {
        self.pass.resolution = scale;
        self
    }

    pub fn queue(mut self, q: QueueAffinity) -> Self {
        self.pass.queue = q;
        self
    }

    pub fn side_effects(mut self) -> Self {
        self.pass.has_side_effects = true;
        self
    }

    pub fn tag(mut self, t: &str) -> Self {
        self.pass.tag = Some(t.to_string());
        self
    }

    /// Finalize the pass and add it to the graph.
    pub fn finish(self) {
        self.graph.add_pass(self.pass);
    }
}

// ---------------------------------------------------------------------------
// Config-driven graph building
// ---------------------------------------------------------------------------

/// A serializable pass description for config-driven graph rebuilding.
#[derive(Debug, Clone)]
pub struct PassConfig {
    pub name: String,
    pub pass_type: PassType,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub condition: Option<String>,
    pub resolution_scale: Option<(f32, f32)>,
    pub queue: QueueAffinity,
    pub explicit_deps: Vec<String>,
}

/// A serializable resource description.
#[derive(Debug, Clone)]
pub struct ResourceConfig {
    pub name: String,
    pub format: TextureFormat,
    pub size: SizePolicy,
    pub imported: bool,
}

/// A serializable graph configuration that can be hot-reloaded.
#[derive(Debug, Clone)]
pub struct GraphConfig {
    pub label: String,
    pub resources: Vec<ResourceConfig>,
    pub passes: Vec<PassConfig>,
    pub features: Vec<String>,
}

impl GraphConfig {
    /// Build a RenderGraph from this configuration.
    pub fn build(&self) -> RenderGraph {
        let mut graph = RenderGraph::new(&self.label);

        // Declare resources
        let mut handles: HashMap<String, ResourceHandle> = HashMap::new();
        for rc in &self.resources {
            let desc = ResourceDescriptor::new(&rc.name, rc.format).with_size(rc.size);
            let h = if rc.imported {
                graph.import_resource(desc)
            } else {
                graph.declare_resource(desc)
            };
            handles.insert(rc.name.clone(), h);
        }

        // Enable features
        for f in &self.features {
            graph.enable_feature(f);
        }

        // Add passes
        for pc in &self.passes {
            let mut pass = RenderPass::new(&pc.name, pc.pass_type);
            pass.queue = pc.queue;

            if let Some(ref cond) = pc.condition {
                pass.condition = PassCondition::FeatureEnabled(cond.clone());
            }
            if let Some((ws, hs)) = pc.resolution_scale {
                pass.resolution = ResolutionScale::custom(ws, hs);
            }

            for iname in &pc.inputs {
                if let Some(&h) = handles.get(iname) {
                    pass.add_input(h, iname);
                }
            }
            for oname in &pc.outputs {
                if let Some(&h) = handles.get(oname) {
                    pass.add_output(h, oname);
                }
            }
            for dep in &pc.explicit_deps {
                pass.depends_on(dep);
            }

            graph.add_pass(pass);
        }

        graph
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_graph() -> RenderGraph {
        let mut b = RenderGraphBuilder::new("test", 1920, 1080);
        let depth = b.texture("depth", TextureFormat::Depth32Float);
        let color = b.texture("color", TextureFormat::Rgba16Float);
        let final_rt = b.texture("final", TextureFormat::Rgba8Unorm);

        b.graphics_pass("depth_pre")
            .writes(depth, "depth")
            .tag("geometry")
            .finish();

        b.graphics_pass("lighting")
            .reads(depth, "depth")
            .writes(color, "color")
            .tag("lighting")
            .finish();

        b.graphics_pass("tonemap")
            .reads(color, "color")
            .writes(final_rt, "final")
            .tag("post")
            .finish();

        b.build()
    }

    #[test]
    fn test_topological_sort() {
        let mut g = simple_graph();
        let sorted = g.topological_sort().unwrap();
        assert_eq!(sorted, vec!["depth_pre", "lighting", "tonemap"]);
    }

    #[test]
    fn test_cycle_detection() {
        let mut g = RenderGraph::new("cycle_test");
        let r1 = g.declare_resource(ResourceDescriptor::new("r1", TextureFormat::Rgba8Unorm));
        let r2 = g.declare_resource(ResourceDescriptor::new("r2", TextureFormat::Rgba8Unorm));

        let mut pa = RenderPass::new("a", PassType::Graphics);
        pa.add_input(r2, "r2");
        pa.add_output(r1, "r1");
        g.add_pass(pa);

        let mut pb = RenderPass::new("b", PassType::Graphics);
        pb.add_input(r1, "r1");
        pb.add_output(r2, "r2");
        g.add_pass(pb);

        let result = g.topological_sort();
        assert!(result.is_err());
    }

    #[test]
    fn test_conditional_pass() {
        let mut g = simple_graph();
        // Disable tonemap via feature
        g.get_pass_mut("tonemap").unwrap().condition =
            PassCondition::FeatureEnabled("hdr_output".to_string());

        let active = g.active_passes().unwrap();
        assert!(!active.contains(&"tonemap".to_string()));
        assert!(active.contains(&"depth_pre".to_string()));

        // Enable feature
        g.enable_feature("hdr_output");
        let active = g.active_passes().unwrap();
        assert!(active.contains(&"tonemap".to_string()));
    }

    #[test]
    fn test_validation() {
        let mut g = simple_graph();
        let result = g.validate();
        assert!(result.is_ok());
    }

    #[test]
    fn test_dot_export() {
        let mut g = simple_graph();
        let dot = g.export_dot();
        assert!(dot.contains("digraph"));
        assert!(dot.contains("depth_pre"));
        assert!(dot.contains("lighting"));
        assert!(dot.contains("tonemap"));
    }

    #[test]
    fn test_merge() {
        let mut g1 = simple_graph();
        let g2 = simple_graph();
        g1.merge(&g2, "post");
        // Should have more passes now
        assert!(g1.pass_count() > 3);
    }

    #[test]
    fn test_graph_config_build() {
        let config = GraphConfig {
            label: "from_config".to_string(),
            resources: vec![
                ResourceConfig {
                    name: "depth".to_string(),
                    format: TextureFormat::Depth32Float,
                    size: SizePolicy::Relative {
                        width_scale: 1.0,
                        height_scale: 1.0,
                    },
                    imported: false,
                },
                ResourceConfig {
                    name: "color".to_string(),
                    format: TextureFormat::Rgba16Float,
                    size: SizePolicy::Relative {
                        width_scale: 1.0,
                        height_scale: 1.0,
                    },
                    imported: false,
                },
            ],
            passes: vec![
                PassConfig {
                    name: "depth_pre".to_string(),
                    pass_type: PassType::Graphics,
                    inputs: vec![],
                    outputs: vec!["depth".to_string()],
                    condition: None,
                    resolution_scale: None,
                    queue: QueueAffinity::Graphics,
                    explicit_deps: vec![],
                },
                PassConfig {
                    name: "lighting".to_string(),
                    pass_type: PassType::Graphics,
                    inputs: vec!["depth".to_string()],
                    outputs: vec!["color".to_string()],
                    condition: None,
                    resolution_scale: None,
                    queue: QueueAffinity::Graphics,
                    explicit_deps: vec![],
                },
            ],
            features: vec![],
        };
        let mut graph = config.build();
        let sorted = graph.topological_sort().unwrap();
        assert_eq!(sorted, vec!["depth_pre", "lighting"]);
    }

    #[test]
    fn test_pass_builder_chain() {
        let mut b = RenderGraphBuilder::new("builder_test", 1280, 720);
        let bloom_half = b.texture_scaled("bloom_half", TextureFormat::Rgba16Float, 0.5, 0.5);
        let bloom_quarter = b.texture_scaled("bloom_quarter", TextureFormat::Rgba16Float, 0.25, 0.25);
        let color = b.texture("hdr_color", TextureFormat::Rgba16Float);

        b.graphics_pass("bloom_down")
            .reads(color, "hdr_color")
            .writes(bloom_half, "bloom_half")
            .resolution(ResolutionScale::half())
            .tag("bloom")
            .finish();

        b.graphics_pass("bloom_down2")
            .reads(bloom_half, "bloom_half")
            .writes(bloom_quarter, "bloom_quarter")
            .resolution(ResolutionScale::quarter())
            .tag("bloom")
            .finish();

        let graph = b.build();
        assert_eq!(graph.pass_count(), 2);
        assert_eq!(graph.resource_count(), 3);
    }
}
