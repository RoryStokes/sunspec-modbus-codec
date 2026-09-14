use heck::{ToPascalCase, ToSnakeCase};
use std::collections::HashSet;

use crate::naming::{Name, NameRef, NameTable, Named};
use crate::sunspec_schema::{
    Group, GroupCount, Point, PointAccess, PointMandatory, PointType, SunspecModel,
};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum CodegenFeature {
    String,
}

pub type DocLines = Vec<String>;

#[derive(Clone)]
pub struct ResolvedType {
    pub rust_type: String,
    pub c_type: String,
    pub size: u16,
    pub writer_function_name: String,
    pub reader_function_name: String,
    pub writer_allow_offset: bool,
    pub enum_repr: Option<String>,
    pub array_length: Option<i64>,
    pub cast_from_c: Option<fn(&str) -> String>,
}

#[derive(Clone, PartialEq)]
pub enum PointValueType {
    Adapter,
    StaticValue(String),
    ModelLength,
}

#[derive(Clone)]
pub struct ResolvedPoint {
    pub name: NameRef,
    pub internal_name: String,
    pub value_type: PointValueType,
    pub point_type: ResolvedType,
    pub access: PointAccess,
    pub mandatory: PointMandatory,
    pub block_indices: Vec<BlockIndex>,
    pub doc: DocLines,
    pub size: u16,
}

impl Named for ResolvedPoint {
    fn name_ref(&self) -> &NameRef {
        &self.name
    }
}

#[derive(Clone)]
pub struct BlockIndex {
    pub group_name: String,
    pub index_name: String,
}

#[derive(Clone)]
pub struct ResolvedEnum {
    pub name: NameRef,
    pub discriminant_type: String,
    pub values: Vec<EnumValue>,
}

impl Named for ResolvedEnum {
    fn name_ref(&self) -> &NameRef {
        &self.name
    }
}

#[derive(Clone)]
pub struct EnumValue {
    pub name_pascal_case: String,
    pub discriminant: String,
    pub doc: DocLines,
}

pub struct CountPoint {
    pub point: ResolvedPoint,
}

pub struct ResolvedModel {
    pub name: NameRef,
    pub features: HashSet<CodegenFeature>,
    pub model_number: u16,
    pub group: ResolvedGroup,
    pub count_points: Vec<CountPoint>,
}

impl Named for ResolvedModel {
    fn name_ref(&self) -> &NameRef {
        &self.name
    }
}

#[derive(Clone)]
pub struct ResolvedGroup {
    pub name: NameRef,
    pub name_short: String,
    /// This group's own short name alone, ignoring any derived prefixes.
    /// Used for identifiers that only ever need to be unique within their own function,
    /// closure, or enum variant - a loop index, a getter argument, a `Point` field
    pub local_name_short: String,
    pub static_size: u16,
    pub points: Vec<ResolvedPoint>,
    pub enums: Vec<ResolvedEnum>,
    /// A group can have more than one repeating child - e.g. model 708's `Crv`, whose
    /// `MustTrip`/`MayTrip`/`MomCess` fixed subgroups each wrap their own independent
    /// curve-point array - so this is a list rather than the single `Option` it once was.
    pub repeating_children: Vec<(ResolvedPoint, Box<ResolvedGroup>)>,
    pub writable: bool,
}

impl Named for ResolvedGroup {
    fn name_ref(&self) -> &NameRef {
        &self.name
    }
}

fn cast_string_from_c(value: &str) -> String {
    format!("unsafe {{ CStr::from_ptr({}) }}", value)
}
fn cast_eui48_from_c(value: &str) -> String {
    format!("unsafe {{ &*({} as *const [u8; 6]) }}", value)
}
fn cast_ipv6_from_c(value: &str) -> String {
    format!("unsafe {{ &*({} as *const [u16; 8]) }}", value)
}

fn resolve_point_type(point: &Point, features: &mut HashSet<CodegenFeature>) -> ResolvedType {
    let base_type = match point.type_ {
        PointType::Uint16
        | PointType::Raw16
        | PointType::Acc16
        | PointType::Bitfield16
        | PointType::Pad
        | PointType::Count
        | PointType::Enum16 => "u16".to_string(),
        PointType::Uint32 | PointType::Acc32 | PointType::Bitfield32 | PointType::Enum32 => {
            "u32".to_string()
        }
        PointType::Uint64 | PointType::Acc64 | PointType::Bitfield64 => "u64".to_string(),
        PointType::Int16 | PointType::Sunssf => "i16".to_string(),
        PointType::Int32 => "i32".to_string(),
        PointType::Int64 => "i64".to_string(),
        PointType::Float32 => "f32".to_string(),
        PointType::Float64 => "f64".to_string(),
        PointType::String => {
            features.insert(CodegenFeature::String);
            "string".to_string()
        }
        PointType::Ipaddr => "u32".to_string(),
        PointType::Ipv6addr => "Ipv6Addr".to_string(),
        PointType::Eui48 => "eui48".to_string(),
    };

    let is_enum = (point.type_ == PointType::Enum16 || point.type_ == PointType::Enum32)
        && !point.symbols.is_empty();

    let rust_type = match point.type_ {
        PointType::String => "&CStr".to_string(),
        PointType::Eui48 => "&[u8; 6]".to_string(),
        PointType::Ipv6addr => "&[u16; 8]".to_string(),
        _ if is_enum => point.name.to_pascal_case(),
        _ => base_type.clone(),
    };

    let c_type = match point.type_ {
        PointType::String => "c_char".to_string(),
        PointType::Eui48 => "u8".to_string(),
        PointType::Ipv6addr => "u16".to_string(),
        _ if is_enum => point.name.to_pascal_case(),
        _ => base_type.clone(),
    };

    let array_length = match point.type_ {
        PointType::String => Some(point.size * 2),
        PointType::Eui48 => Some(6),
        PointType::Ipv6addr => Some(8),
        _ => None,
    };

    let writer_function_name = format!("write_{}", base_type.to_snake_case());
    let reader_function_name = match point.type_ {
        PointType::String => format!("read_{}::<{}>", base_type.to_snake_case(), point.size * 2),
        _ => format!("read_{}", base_type.to_snake_case()),
    };

    let writer_allow_offset = base_type != "u16" && base_type != "i16";

    let cast_from_c: Option<fn(&str) -> String> = match point.type_ {
        PointType::String => Some(cast_string_from_c),
        PointType::Eui48 => Some(cast_eui48_from_c),
        PointType::Ipv6addr => Some(cast_ipv6_from_c),
        _ => None,
    };

    let enum_repr: Option<String> = match point.type_ {
        _ if is_enum => Some(base_type),
        _ => None,
    };

    ResolvedType {
        rust_type,
        c_type,
        size: point.size as u16,
        array_length,
        writer_function_name,
        reader_function_name,
        writer_allow_offset,
        enum_repr,
        cast_from_c,
    }
}

pub(crate) fn resolve_point(
    point: &Point,
    features: &mut HashSet<CodegenFeature>,
    identifier_text: &str,
    name_prefix: Option<String>,
    block_indices: Vec<BlockIndex>,
) -> Option<ResolvedPoint> {
    let point_type = resolve_point_type(point, features);

    let main_name = point
        .label
        .clone()
        .map(|label| {
            if point.access == PointAccess::Rw && label.starts_with("Set ") {
                label[4..].to_string()
            } else {
                label
            }
        })
        .unwrap_or(point.name.clone());

    let name = format!(
        "{identifier_text} {} {main_name}",
        name_prefix.clone().get_or_insert_default(),
    );

    let label = if let Some(label) = &point.label {
        format!("{} ({})", label, point.name)
    } else {
        point.name.clone()
    };

    let doc = [Some(label), point.desc.clone(), point.detail.clone()]
        .into_iter()
        .flatten()
        .collect();

    let value_type = if point.type_ == PointType::Pad {
        PointValueType::StaticValue("0".to_string())
    } else if let Some(value) = &point.value {
        PointValueType::StaticValue(value.to_string())
    } else if point.name == "L" {
        PointValueType::ModelLength
    } else {
        PointValueType::Adapter
    };

    Some(ResolvedPoint {
        name: Name::new(name.to_snake_case(), name.to_pascal_case()),
        internal_name: point.name.clone(),
        point_type,
        value_type,
        block_indices,
        access: point.access,
        mandatory: point.mandatory,
        size: point.size as u16,
        doc,
    })
}

pub fn resolve_enum(point: &Point) -> Option<ResolvedEnum> {
    if point.type_ == PointType::Enum16 || point.type_ == PointType::Enum32 {
        let values: Vec<EnumValue> = point
            .symbols
            .iter()
            .map(|symbol| EnumValue {
                name_pascal_case: symbol.name.to_pascal_case(),
                discriminant: symbol.value.to_string(),
                doc: [
                    symbol.label.as_ref(),
                    symbol.desc.as_ref(),
                    symbol.detail.as_ref(),
                ]
                .iter()
                .flat_map(|r| r.cloned())
                .collect(),
            })
            .collect();

        if values.is_empty() {
            None
        } else {
            Some(ResolvedEnum {
                name: Name::new(point.name.to_snake_case(), point.name.to_pascal_case()),
                discriminant_type: match point.type_ {
                    PointType::Enum16 => "u16".to_string(),
                    PointType::Enum32 => "u32".to_string(),
                    _ => "".to_string(),
                },
                values,
            })
        }
    } else {
        None
    }
}

/// The (snake_case name, PascalCase name, local short name) a schema group will resolve to,
/// given the naming context inherited from its ancestors - computed straight from the schema,
/// before the group itself is resolved. `resolve_group` uses this for its own identity; a
/// parent discovering a repeating child also uses it to build that child's [`BlockIndex`]
/// (which needs the child's own final name) before recursing into it, since both reduce to
/// exactly the same values.
fn group_identity(schema_group: &Group, identifier_text: &str) -> (String, String, String) {
    let label = schema_group.label.as_ref().unwrap_or(&schema_group.name);
    let name = format!("{identifier_text} {label}");
    (
        name.to_snake_case(),
        name.to_pascal_case(),
        schema_group.name.to_snake_case(),
    )
}

pub(crate) fn resolve_group(
    group: &Group,
    top_level_points_opt: Option<&[ResolvedPoint]>,
    features: &mut HashSet<CodegenFeature>,
    identifier_text: &str,
    short_text: &str,
    name_prefix: Option<String>,
    block_indices: Vec<BlockIndex>,
) -> ResolvedGroup {
    let (name_snake, name_pascal, local_name_short) = group_identity(group, identifier_text);

    let root_points: Vec<ResolvedPoint> = group
        .points
        .iter()
        .flat_map(|point| {
            resolve_point(
                point,
                features,
                identifier_text,
                name_prefix.clone(),
                block_indices.clone(),
            )
        })
        .collect();

    let top_level_points = top_level_points_opt.unwrap_or(&root_points);

    // Fixed subgroups don't create a separate repeating structure on the wire - SunSpec uses
    // them purely to group related points/subgroups under a label. They don't add a block
    // index either - only a repeating group does - so `block_indices` passes through unchanged.
    let fixed_children: Vec<ResolvedGroup> = group
        .groups
        .iter()
        .filter(|g| matches!(g.count, GroupCount::Integer(_)))
        .map(|child| {
            let child_label = child.label.as_ref().unwrap_or(&child.name);
            resolve_group(
                child,
                Some(top_level_points),
                features,
                &format!("{child_label} {identifier_text}"),
                &format!("{} {short_text}", child.name),
                name_prefix.clone(),
                block_indices.clone(),
            )
        })
        .collect();
    
    let repeating_children: Vec<(ResolvedPoint, Box<ResolvedGroup>)> = group
        .groups
        .iter()
        .filter_map(|g| match &g.count {
            GroupCount::String(count_name) => {
                let count_point = top_level_points
                    .iter()
                    .find(|p| p.internal_name == *count_name);

                count_point.map(|p| {
                    let (group_name, _, index_prefix) = group_identity(g, identifier_text);
                    let mut child_block_indices = block_indices.clone();
                    child_block_indices.push(BlockIndex {
                        group_name,
                        index_name: format!("{index_prefix}_index"),
                    });

                    (
                        p.clone(),
                        Box::new(resolve_group(
                            g,
                            Some(top_level_points),
                            features,
                            identifier_text,
                            short_text,
                            Some(g.name.clone()),
                            child_block_indices,
                        )),
                    )
                })
            }
            _ => None,
        })
        .chain(
            // A repeating group can also be nested inside one of this group's fixed children to logically identify it
            // within the model.
            // e.g. model 708's curve types MustTrip, MayTrip and MomCess are themselves not repeating, but wrap their own
            // `Pt` array.
            // These are simply spliced in alongside any direct repeating children as naming concerns are resolved by the
            // call to resolve_group when computing fixed_children.
            fixed_children
                .iter()
                .flat_map(|f| f.repeating_children.iter().cloned()),
        )
        .collect();

    let points: Vec<ResolvedPoint> = root_points
        .into_iter()
        .chain(fixed_children.iter().flat_map(|f| f.points.iter().cloned()))
        .collect();

    let mut enums: Vec<ResolvedEnum> = group
        .points
        .iter()
        .flat_map(resolve_enum)
        .chain(fixed_children.iter().flat_map(|f| f.enums.iter().cloned()))
        .chain(
            repeating_children
                .iter()
                .flat_map(|(_, g)| g.enums.iter().cloned()),
        )
        .collect();

    enums.sort_by_key(|e| e.name_snake_case());
    enums.dedup_by_key(|e| e.name_snake_case());

    let size = points.iter().map(|point| point.size).sum();

    let name_short = format!("{short_text} {}", group.name);

    let writable = repeating_children.iter().any(|(_, g)| g.writable)
        || points.iter().any(|p| p.access == PointAccess::Rw);

    ResolvedGroup {
        name: Name::new(name_snake, name_pascal),
        name_short: name_short.to_snake_case(),
        local_name_short,
        static_size: size,
        points,
        enums,
        repeating_children,
        writable,
    }
}

/// Registers every point and group in `group`'s subtree - both those found directly and any
/// promoted up through a fixed subgroup - with the model-wide `point_names`/`group_names`
/// tables ahead of a single [`NameTable::deduplicate`] pass over each.
fn register_names(group: &ResolvedGroup, point_names: &mut NameTable, group_names: &mut NameTable) {
    group_names.register_unique(group.name.clone());
    for point in &group.points {
        point_names.register(
            point.name.clone(),
            (
                point.internal_name.to_snake_case(),
                point.internal_name.to_pascal_case(),
            ),
        );
    }
    for (_, child) in &group.repeating_children {
        register_names(child, point_names, group_names);
    }
}

/// Collects one [`CountPoint`] per distinct repeat count in `group`'s subtree, in the order
/// each is first encountered. A count point reused by more than one repeating child (see
/// `resolve_model`) is collected only once.
fn collect_count_points(group: &ResolvedGroup, count_points: &mut Vec<CountPoint>) {
    for (count_point, child) in &group.repeating_children {
        if !count_points
            .iter()
            .any(|cp| cp.point.internal_name == count_point.internal_name)
        {
            count_points.push(CountPoint {
                point: count_point.clone(),
            });
        }
        collect_count_points(child, count_points);
    }
}

pub fn resolve_model(model: &SunspecModel, file_name: String) -> ResolvedModel {
    let model_number: u16 = file_name[6..].parse().expect(
        "Unable to extract model number from name (expected name in structure model_X.json)",
    );
    let mut features: HashSet<CodegenFeature> = HashSet::new();
    let group = resolve_group(&model.group, None, &mut features, "", "", None, vec![]);

    let mut point_names = NameTable::default();
    let mut group_names = NameTable::default();
    register_names(&group, &mut point_names, &mut group_names);
    point_names.deduplicate();
    group_names.deduplicate();

    // `group.enums` already collects every enum in the model: resolve_group bubbles them up
    // through fixed subgroups and every repeating child into the top-level group's own list.
    let mut enum_names = NameTable::default();
    for resolved_enum in &group.enums {
        enum_names.register_unique(resolved_enum.name.clone());
    }
    enum_names.deduplicate();

    // A count point can be shared by more than one repeating child - model 708's `NPt` governs
    // all three of `Crv`'s curve-point arrays. Per the SunSpec spec that's one field on the
    // model, read independently by each array, not one field per array, so it's collected once
    // (by its underlying SunSpec name) no matter how many repeating children reuse it.
    let mut count_points = vec![];
    collect_count_points(&group, &mut count_points);

    ResolvedModel {
        model_number,
        name: Name::new(file_name.to_snake_case(), file_name.to_pascal_case()),
        group,
        features,
        count_points,
    }
}
