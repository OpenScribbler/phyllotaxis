use std::fmt::Write as FmtWrite;

use crate::commands::overview::OverviewData;
use crate::models::resource::ResourceGroup;

/// Strip terminal control characters from spec-sourced strings before output.
/// Prevents ANSI escape injection (ESC = 0x1B, and other non-printing controls).
///
/// Allowed: tab (0x09), newline (0x0A), carriage return (0x0D) — these are legitimate
/// whitespace. Everything else below 0x20 (including ESC 0x1B) and DEL (0x7F) is stripped.
///
/// Fast path: if no control characters are present the function avoids any allocation by
/// returning the input string unchanged as an owned copy of its contents. This keeps the
/// hot path (normal clean specs) zero-overhead aside from the one unavoidable byte scan.
fn sanitize(s: &str) -> String {
    if !s.bytes().any(|b| (b < 0x20 && !matches!(b, 0x09 | 0x0A | 0x0D)) || b == 0x7F) {
        return s.to_owned();
    }
    s.chars()
        .filter(|&c| {
            let n = c as u32;
            (n >= 0x20 || matches!(n, 0x09 | 0x0A | 0x0D)) && n != 0x7F
        })
        .collect()
}

pub fn render_overview(data: &OverviewData, _is_tty: bool) -> String {
    let mut out = String::new();

    writeln!(out, "API: {}", sanitize(&data.title)).unwrap();

    if let Some(ref desc) = data.description {
        writeln!(out, "{}", sanitize(desc)).unwrap();
    }

    if data.base_urls.len() == 1 {
        writeln!(out, "Base URL: {}", sanitize(&data.base_urls[0])).unwrap();
    } else if !data.base_urls.is_empty() {
        out.push_str("Base URLs:\n");
        for url in &data.base_urls {
            writeln!(out, "  {}", sanitize(url)).unwrap();
        }
    }

    if !data.server_variables.is_empty() {
        out.push_str("  Variables:\n");
        for var in &data.server_variables {
            let req = if var.required { "required" } else { "optional" };
            let desc = sanitize(var.description.as_deref().unwrap_or(""));
            let default = var
                .default
                .as_ref()
                .map(|d| format!("  default: {}", sanitize(d)))
                .unwrap_or_default();
            writeln!(out, "    {}  ({})  {}{}", sanitize(&var.name), req, desc, default).unwrap();
        }
    }

    if !data.auth_schemes.is_empty() {
        let schemes: Vec<String> = data.auth_schemes.iter().map(|s| sanitize(s)).collect();
        writeln!(out, "Auth: {}", schemes.join(", ")).unwrap();
    }

    out.push('\n');
    out.push_str("Commands:\n");
    writeln!(
        out,
        "  phyllotaxis resources    List all resource groups ({} available)",
        data.resource_count
    ).unwrap();
    writeln!(
        out,
        "  phyllotaxis schemas      List all data models ({} available)",
        data.schema_count
    ).unwrap();
    out.push_str("  phyllotaxis auth         Authentication details\n");
    writeln!(
        out,
        "  phyllotaxis callbacks    List all webhook callbacks ({} available)",
        data.callback_count
    ).unwrap();
    out.push_str("  phyllotaxis search       Search across all endpoints and schemas\n");

    out
}

pub fn render_endpoint_detail(endpoint: &crate::models::resource::Endpoint, is_tty: bool) -> String {
    use crate::models::resource::ParameterLocation;

    let mut out = String::new();

    // Header
    let marker = if endpoint.is_deprecated {
        " [DEPRECATED]"
    } else if endpoint.is_alpha {
        " [ALPHA]"
    } else {
        ""
    };
    writeln!(out, "{} {}{}", sanitize(&endpoint.method), sanitize(&endpoint.path), marker).unwrap();

    if let Some(ref desc) = endpoint.description {
        writeln!(out, "{}", sanitize(desc)).unwrap();
    }

    // Auth
    if !endpoint.security_schemes.is_empty() {
        let schemes: Vec<String> = endpoint.security_schemes.iter().map(|s| sanitize(s)).collect();
        writeln!(out, "\nAuthentication: {} (required)", schemes.join(", ")).unwrap();
    }

    // Parameters by location
    let path_params: Vec<_> = endpoint
        .parameters
        .iter()
        .filter(|p| matches!(p.location, ParameterLocation::Path))
        .collect();
    let query_params: Vec<_> = endpoint
        .parameters
        .iter()
        .filter(|p| matches!(p.location, ParameterLocation::Query))
        .collect();
    let header_params: Vec<_> = endpoint
        .parameters
        .iter()
        .filter(|p| matches!(p.location, ParameterLocation::Header))
        .collect();

    render_param_section(&mut out, "Path Parameters", &path_params);
    render_param_section(&mut out, "Query Parameters", &query_params);
    if !header_params.is_empty() {
        render_param_section(&mut out, "Header Parameters", &header_params);
    }

    // Request body
    if let Some(ref body) = endpoint.request_body {
        writeln!(out, "\nRequest Body ({}):", sanitize(&body.content_type)).unwrap();

        if !body.options.is_empty() {
            // OneOf/AnyOf body: show variant options
            writeln!(out, "  One of ({} options):", body.options.len()).unwrap();
            for opt in &body.options {
                writeln!(out, "    phyllotaxis schemas {}", sanitize(opt)).unwrap();
            }
        } else if body.fields.is_empty() {
            out.push_str("  Raw body (no schema)\n");
        } else {
            render_fields_section(&mut out, &body.fields);
        }

        if let Some(ref example) = body.example {
            out.push_str("\nRequest Example:\n");
            let pretty =
                serde_json::to_string_pretty(example).unwrap_or_else(|_| example.to_string());
            for line in pretty.lines() {
                writeln!(out, "  {}", line).unwrap();
            }
        }
    }

    // Responses: success first, then errors
    let successes: Vec<_> = endpoint
        .responses
        .iter()
        .filter(|r| r.status_code.starts_with('2'))
        .collect();
    let errors: Vec<_> = endpoint
        .responses
        .iter()
        .filter(|r| !r.status_code.starts_with('2'))
        .collect();

    let arrow = if is_tty { "→" } else { "->" };

    if !successes.is_empty() {
        out.push_str("\nResponses:\n");
        for resp in &successes {
            let schema = resp
                .schema_ref
                .as_ref()
                .map(|s| format!(" {} {}", arrow, sanitize(s)))
                .unwrap_or_default();
            writeln!(out, "  {} {}{}", sanitize(&resp.status_code), sanitize(&resp.description), schema).unwrap();

            if let Some(ref example) = resp.example {
                out.push_str("  Example:\n");
                let pretty =
                    serde_json::to_string_pretty(example).unwrap_or_else(|_| example.to_string());
                for line in pretty.lines() {
                    writeln!(out, "    {}", line).unwrap();
                }
            }

            if !resp.headers.is_empty() {
                out.push_str("    Headers:\n");
                for h in &resp.headers {
                    match h.description.as_deref() {
                        Some(desc) => writeln!(out, "      {}  {}  {}", sanitize(&h.name), sanitize(&h.type_display), sanitize(desc)).unwrap(),
                        None => writeln!(out, "      {}  {}", sanitize(&h.name), sanitize(&h.type_display)).unwrap(),
                    }
                }
            }
        }
    }

    if !errors.is_empty() {
        out.push_str("\nErrors:\n");
        for resp in &errors {
            writeln!(out, "  {} {}", sanitize(&resp.status_code), sanitize(&resp.description)).unwrap();
        }
    }

    if !endpoint.links.is_empty() {
        out.push_str("\nLinks:\n");
        for link in &endpoint.links {
            writeln!(out, "  {} -> {}", sanitize(&link.name), sanitize(&link.operation_id)).unwrap();
            if let Some(ref desc) = link.description {
                writeln!(out, "    {}", sanitize(desc)).unwrap();
            }
            for param in &link.parameters {
                writeln!(out, "    {}", sanitize(param)).unwrap();
            }
            if let Some(ref cmd) = link.drill_command {
                writeln!(out, "    {}", sanitize(cmd)).unwrap();
            }
        }
    }

    if !endpoint.callbacks.is_empty() {
        out.push_str("\nCallbacks:\n");
        for cb in &endpoint.callbacks {
            for op in &cb.operations {
                writeln!(
                    out,
                    "  {} -> {} {}",
                    sanitize(&cb.name),
                    sanitize(&op.method),
                    sanitize(&op.url_expression)
                ).unwrap();
                if let Some(ref schema) = op.body_schema {
                    writeln!(out, "    Body: {}", sanitize(schema)).unwrap();
                }
                if !op.responses.is_empty() {
                    let codes: Vec<String> = op.responses.iter()
                        .map(|r| sanitize(&r.status_code))
                        .collect();
                    writeln!(out, "    Responses: {}", codes.join(", ")).unwrap();
                }
            }
        }
    }

    if is_tty && !endpoint.drill_deeper.is_empty() {
        out.push_str("\nDrill deeper:\n");
        for cmd in &endpoint.drill_deeper {
            writeln!(out, "  {}", sanitize(cmd)).unwrap();
        }
    }

    out
}

fn render_param_section(
    out: &mut String,
    title: &str,
    params: &[&crate::models::resource::Parameter],
) {
    writeln!(out, "\n{}:", title).unwrap();
    if params.is_empty() {
        out.push_str("  (none)\n");
        return;
    }
    let max_name = params.iter().map(|p| p.name.len()).max().unwrap_or(0);
    for p in params {
        let req = if p.required { "required" } else { "optional" };
        let enums = if p.enum_values.is_empty() {
            String::new()
        } else {
            let sanitized: Vec<String> = p.enum_values.iter().map(|v| sanitize(v)).collect();
            format!("  Enum: [{}]", sanitized.join(", "))
        };
        let desc = sanitize(p.description.as_deref().unwrap_or(""));
        writeln!(
            out,
            "  {:<width$}  {}  ({})  {}{}",
            sanitize(&p.name),
            sanitize(&p.schema_type),
            req,
            desc,
            enums,
            width = max_name,
        ).unwrap();
    }
}

fn render_fields_section(out: &mut String, fields: &[crate::models::resource::Field]) {
    if fields.is_empty() {
        return;
    }
    let max_name = fields.iter().map(|f| f.name.len()).max().unwrap_or(0);
    let max_type = fields.iter().map(|f| f.type_display.len()).max().unwrap_or(0);

    for f in fields {
        let mut flags = Vec::new();
        if f.required {
            flags.push("required");
        }
        if f.optional {
            flags.push("optional");
        }
        if f.nullable {
            flags.push("nullable");
        }
        if f.read_only {
            flags.push("read-only");
        }
        if f.write_only {
            flags.push("write-only");
        }
        if f.deprecated {
            flags.push("DEPRECATED");
        }
        let flag_str = if flags.is_empty() {
            String::new()
        } else {
            format!("({})", flags.join(", "))
        };

        let constraints_str = if f.constraints.is_empty() {
            String::new()
        } else {
            format!("  {}", f.constraints.join(" "))
        };

        let enums = if f.enum_values.is_empty() {
            String::new()
        } else {
            let sanitized: Vec<String> = f.enum_values.iter().map(|v| sanitize(v)).collect();
            format!("  Enum: [{}]", sanitized.join(", "))
        };

        let desc = sanitize(f.description.as_deref().unwrap_or(""));

        if !f.nested_fields.is_empty() {
            writeln!(
                out,
                "  {:<nw$}  {}:",
                sanitize(&f.name),
                sanitize(&f.type_display),
                nw = max_name,
            ).unwrap();
            render_schema_fields(out, &f.nested_fields, 4);
            continue;
        }

        writeln!(
            out,
            "  {:<nw$}  {:<tw$}  {:<20}  {}{}{}",
            sanitize(&f.name),
            sanitize(&f.type_display),
            flag_str,
            desc,
            constraints_str,
            enums,
            nw = max_name,
            tw = max_type,
        ).unwrap();
    }
}

pub fn render_schema_list(names: &[String], is_tty: bool) -> String {
    let mut out = String::new();
    writeln!(out, "Schemas ({} total):", names.len()).unwrap();

    if names.is_empty() {
        out.push_str("  (none)\n");
    } else {
        for name in names {
            writeln!(out, "  {}", sanitize(name)).unwrap();
        }
    }

    if is_tty {
        out.push_str("\nDrill deeper:\n");
        out.push_str("  phyllotaxis schemas <name>\n");
    }

    out
}

pub fn render_schema_detail(
    model: &crate::models::schema::SchemaModel,
    expanded: bool,
    is_tty: bool,
) -> String {
    use crate::models::schema::Composition;

    let mut out = String::new();

    // Header
    if expanded {
        writeln!(out, "Schema: {} (expanded)", sanitize(&model.name)).unwrap();
    } else {
        writeln!(out, "Schema: {}", sanitize(&model.name)).unwrap();
    }

    if let Some(ref title) = model.title {
        if title != &model.name {
            writeln!(out, "Title: {}", sanitize(title)).unwrap();
        }
    }

    if let Some(ref desc) = model.description {
        writeln!(out, "{}", sanitize(desc)).unwrap();
    }

    // Composition info
    if let Some(ref comp) = model.composition {
        match comp {
            Composition::AllOf => {
                out.push_str("\nComposition: allOf (fields merged below)\n");
            }
            Composition::OneOf(variants) => {
                out.push_str("\nComposition: oneOf\n");
                out.push_str("  One of:\n");
                for v in variants {
                    writeln!(out, "    phyllotaxis schemas {}", sanitize(v)).unwrap();
                }
            }
            Composition::AnyOf(variants) => {
                out.push_str("\nComposition: anyOf\n");
                out.push_str("  Any of:\n");
                for v in variants {
                    writeln!(out, "    phyllotaxis schemas {}", sanitize(v)).unwrap();
                }
            }
            Composition::Enum(values) => {
                let sanitized: Vec<String> = values.iter().map(|v| sanitize(v)).collect();
                writeln!(
                    out,
                    "\nEnum values ({}):\n  [{}]",
                    sanitized.len(),
                    sanitized.join(", ")
                ).unwrap();
            }
        }
    }

    let arrow = if is_tty { "→" } else { "->" };

    // Discriminator
    if let Some(ref disc) = model.discriminator {
        writeln!(out, "\nDiscriminator: {}", sanitize(&disc.property_name)).unwrap();
        if !disc.mapping.is_empty() {
            out.push_str("  Subtypes:\n");
            let max_key = disc.mapping.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
            for (value, schema_name) in &disc.mapping {
                writeln!(
                    out,
                    "    {:<width$}  {} phyllotaxis schemas {}",
                    sanitize(value),
                    arrow,
                    sanitize(schema_name),
                    width = max_key
                ).unwrap();
            }
        }
    }

    // Fields
    if !model.fields.is_empty() {
        out.push_str("\nFields:\n");
        render_schema_fields(&mut out, &model.fields, 2);
    }

    // Related schemas (only when NOT expanded)
    if !expanded {
        let nested: Vec<&str> = model
            .fields
            .iter()
            .filter_map(|f| f.nested_schema_name.as_deref())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();

        if !nested.is_empty() {
            out.push_str("\nRelated schemas:\n");
            let mut sorted = nested;
            sorted.sort();
            for name in sorted {
                writeln!(out, "  phyllotaxis schemas {}", name).unwrap();
            }
        }
    }

    // External docs
    if let Some(ref docs) = model.external_docs {
        writeln!(out, "\nSee also: {}", sanitize(&docs.url)).unwrap();
        if let Some(ref desc) = docs.description {
            writeln!(out, "  {}", sanitize(desc)).unwrap();
        }
    }

    out
}

fn render_schema_fields(out: &mut String, fields: &[crate::models::resource::Field], indent: usize) {
    let prefix = " ".repeat(indent);
    let max_name = fields.iter().map(|f| f.name.len()).max().unwrap_or(0);
    let max_type = fields.iter().map(|f| f.type_display.len()).max().unwrap_or(0);

    for f in fields {
        let mut flags = Vec::new();
        if f.required {
            flags.push("required");
        }
        if f.optional {
            flags.push("optional");
        }
        if f.nullable {
            flags.push("nullable");
        }
        if f.read_only {
            flags.push("read-only");
        }
        if f.write_only {
            flags.push("write-only");
        }
        if f.deprecated {
            flags.push("DEPRECATED");
        }
        let flag_str = if flags.is_empty() {
            String::new()
        } else {
            format!("({})", flags.join(", "))
        };

        let constraints_str = if f.constraints.is_empty() {
            String::new()
        } else {
            format!("  {}", f.constraints.join(" "))
        };

        let enums = if f.enum_values.is_empty() {
            String::new()
        } else {
            let sanitized: Vec<String> = f.enum_values.iter().map(|v| sanitize(v)).collect();
            format!("  Enum: [{}]", sanitized.join(", "))
        };

        let desc = sanitize(f.description.as_deref().unwrap_or(""));

        if !f.nested_fields.is_empty() {
            // Expanded nested: show type with colon then nested fields indented
            writeln!(
                out,
                "{}{:<nw$}  {}:",
                prefix,
                sanitize(&f.name),
                sanitize(&f.type_display),
                nw = max_name,
            ).unwrap();
            render_schema_fields(out, &f.nested_fields, indent + 2);
        } else {
            writeln!(
                out,
                "{}{:<nw$}  {:<tw$}  {:<20}  {}{}{}",
                prefix,
                sanitize(&f.name),
                sanitize(&f.type_display),
                flag_str,
                desc,
                constraints_str,
                enums,
                nw = max_name,
                tw = max_type,
            ).unwrap();
        }
    }
}

pub fn render_callback_list(callbacks: &[crate::models::resource::CallbackEntry], is_tty: bool) -> String {
    let mut out = String::new();
    if callbacks.is_empty() {
        out.push_str("Callbacks: (none)\n");
        return out;
    }
    writeln!(out, "Callbacks ({} total):", callbacks.len()).unwrap();
    for cb in callbacks {
        let op_count = cb.operations.len();
        let op_label = if op_count == 1 { "operation" } else { "operations" };
        writeln!(
            out,
            "  {} ({} {})  (on {} {})",
            sanitize(&cb.name),
            op_count,
            op_label,
            sanitize(&cb.defined_on_method),
            sanitize(&cb.defined_on_path)
        ).unwrap();
    }
    if is_tty {
        out.push_str("\nDrill deeper:\n");
        out.push_str("  phyllotaxis callbacks <name>\n");
    }
    out
}

pub fn render_callback_detail(cb: &crate::models::resource::CallbackEntry, is_tty: bool) -> String {
    let mut out = String::new();
    writeln!(out, "Callback: {}", sanitize(&cb.name)).unwrap();
    writeln!(
        out,
        "Defined on: {} {}",
        sanitize(&cb.defined_on_method),
        sanitize(&cb.defined_on_path)
    ).unwrap();

    for op in &cb.operations {
        writeln!(out, "\n  {} {}", sanitize(&op.method), sanitize(&op.url_expression)).unwrap();
        if let Some(ref schema) = op.body_schema {
            writeln!(out, "    Body: {}", sanitize(schema)).unwrap();
        }
        if !op.responses.is_empty() {
            out.push_str("    Responses:\n");
            for r in &op.responses {
                writeln!(out, "      {}  {}", sanitize(&r.status_code), sanitize(&r.description)).unwrap();
            }
        }
    }

    if is_tty {
        let schema_names: Vec<&str> = cb.operations.iter()
            .filter_map(|op| op.body_schema.as_deref())
            .filter(|s| *s != "inline object")
            .collect();
        if !schema_names.is_empty() {
            out.push_str("\nDrill deeper:\n");
            for name in schema_names {
                writeln!(out, "  phyllotaxis schemas {}", sanitize(name)).unwrap();
            }
        }
    }

    out
}

pub fn render_search(results: &crate::commands::search::SearchResults, is_tty: bool) -> String {
    let mut out = String::new();

    let has_any = !results.resources.is_empty()
        || !results.endpoints.is_empty()
        || !results.schemas.is_empty()
        || !results.callbacks.is_empty();

    if !has_any {
        writeln!(out, "No results found for \"{}\".", results.term).unwrap();
        return out;
    }

    writeln!(out, "Results for \"{}\":", results.term).unwrap();

    if !results.resources.is_empty() {
        out.push_str("\nResources:\n");
        let max_slug = results.resources.iter().map(|r| r.slug.len()).max().unwrap_or(0);
        for r in &results.resources {
            let desc = sanitize(r.description.as_deref().unwrap_or(""));
            writeln!(out, "  {:<width$}  {}", sanitize(&r.slug), desc, width = max_slug).unwrap();
        }
    }

    if !results.endpoints.is_empty() {
        out.push_str("\nEndpoints:\n");
        let max_path = results.endpoints.iter().map(|e| e.path.len()).max().unwrap_or(0);
        for e in &results.endpoints {
            let summary = sanitize(e.summary.as_deref().unwrap_or(""));
            writeln!(
                out,
                "  {:<7} {:<width$}  {}",
                sanitize(&e.method), sanitize(&e.path), summary, width = max_path
            ).unwrap();
            if !e.resource_slug.is_empty() {
                writeln!(
                    out,
                    "    phyllotaxis resources {} {} {}",
                    sanitize(&e.resource_slug),
                    sanitize(&e.method),
                    sanitize(&e.path),
                ).unwrap();
            }
        }
    }

    if !results.schemas.is_empty() {
        out.push_str("\nSchemas:\n");
        for s in &results.schemas {
            writeln!(out, "  {}", sanitize(&s.name)).unwrap();
        }
    }

    if !results.callbacks.is_empty() {
        out.push_str("\nCallbacks:\n");
        for cb in &results.callbacks {
            writeln!(
                out,
                "  {}  (on {})",
                sanitize(&cb.name),
                sanitize(&cb.defined_on_path)
            ).unwrap();
            if is_tty {
                writeln!(out, "    phyllotaxis callbacks {}", sanitize(&cb.name)).unwrap();
            }
        }
    }

    // Drill deeper (TTY only)
    if is_tty {
        let mut drill = Vec::new();
        for r in results.resources.iter().take(5) {
            drill.push(format!("phyllotaxis resources {}", sanitize(&r.slug)));
        }
        for s in results.schemas.iter().take(5) {
            drill.push(format!("phyllotaxis schemas {}", sanitize(&s.name)));
        }
        if !drill.is_empty() {
            out.push_str("\nDrill deeper:\n");
            for cmd in &drill {
                writeln!(out, "  {}", cmd).unwrap();
            }
        }
    }

    out
}

pub fn render_auth(model: &crate::commands::auth::AuthModel, is_tty: bool) -> String {
    let mut out = String::new();
    out.push_str("Authentication:\n");

    if model.schemes.is_empty() {
        out.push_str("\n  (none configured)\n");
    } else {
        for scheme in &model.schemes {
            writeln!(
                out,
                "\n  {} ({})",
                sanitize(&scheme.name),
                sanitize(&scheme.scheme_type).to_uppercase()
            ).unwrap();
            writeln!(out, "    Scheme: {}", sanitize(&scheme.detail)).unwrap();
            if let Some(ref desc) = scheme.description {
                writeln!(out, "    Description: {}", sanitize(desc)).unwrap();
            }
            let qualifier = if model.total_operations > 0
                && scheme.usage_count == model.total_operations
            {
                " (all endpoints)"
            } else {
                ""
            };
            writeln!(
                out,
                "\n    Used by: {} operation(s){}",
                scheme.usage_count, qualifier
            ).unwrap();
        }
    }

    if is_tty {
        out.push_str("\nDrill deeper:\n");
        out.push_str("  phyllotaxis resources    Browse endpoints by resource group\n");
    }

    out
}

pub fn render_resource_detail(group: &ResourceGroup, is_tty: bool) -> String {
    let mut out = String::new();

    writeln!(out, "Resource: {}", sanitize(&group.display_name)).unwrap();
    if let Some(ref desc) = group.description {
        writeln!(out, "Description: {}", sanitize(desc)).unwrap();
    }

    out.push_str("\nEndpoints:\n");

    let max_path = group
        .endpoints
        .iter()
        .map(|e| e.path.len())
        .max()
        .unwrap_or(0);

    for ep in &group.endpoints {
        let marker = if ep.is_deprecated {
            " [DEPRECATED]"
        } else if ep.is_alpha {
            " [ALPHA]"
        } else {
            ""
        };
        let summary = sanitize(ep.summary.as_deref().unwrap_or(""));
        writeln!(
            out,
            "  {:<7} {:<width$}  {}{}",
            sanitize(&ep.method),
            sanitize(&ep.path),
            summary,
            marker,
            width = max_path,
        ).unwrap();
    }

    if is_tty {
        out.push_str("\nDrill deeper:\n");
        for ep in &group.endpoints {
            writeln!(
                out,
                "  phyllotaxis resources {} {} {}",
                sanitize(&group.slug), sanitize(&ep.method), sanitize(&ep.path)
            ).unwrap();
        }
    }

    out
}

pub fn render_resource_list(groups: &[ResourceGroup], is_tty: bool) -> String {
    let mut out = String::new();
    out.push_str("Resources:\n");

    let max_slug = groups.iter().map(|g| g.slug.len()).max().unwrap_or(0);

    for group in groups {
        let marker = if group.is_deprecated {
            "[DEPRECATED]"
        } else if group.is_alpha {
            "[ALPHA]     "
        } else {
            "            "
        };

        let desc = sanitize(group.description.as_deref().unwrap_or(""));
        writeln!(
            out,
            "  {:<width$}  {}  {}",
            sanitize(&group.slug),
            marker,
            desc,
            width = max_slug,
        ).unwrap();
    }

    if is_tty {
        out.push_str("\nDrill deeper:\n");
        out.push_str("  phyllotaxis resources <name>\n");
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::overview::{OverviewData, ServerVar};

    #[test]
    fn test_render_overview_basic() {
        let data = OverviewData {
            title: "Petstore API".to_string(),
            description: None,
            base_urls: vec!["https://api.example.com".to_string()],
            server_variables: vec![],
            auth_schemes: vec!["bearerAuth".to_string()],
            resource_count: 3,
            schema_count: 4,
            callback_count: 0,
        };
        let output = render_overview(&data, true);
        assert!(output.contains("API: Petstore API"), "Missing title");
        assert!(
            output.contains("phyllotaxis resources"),
            "Missing resources command"
        );
        assert!(
            output.contains("phyllotaxis schemas"),
            "Missing schemas command"
        );
        assert!(output.contains("3 available"), "Missing resource count");
        assert!(output.contains("Auth:"), "Missing auth line");
    }

    #[test]
    fn test_render_overview_no_auth() {
        let data = OverviewData {
            title: "Test".to_string(),
            description: None,
            base_urls: vec![],
            server_variables: vec![],
            auth_schemes: vec![],
            resource_count: 0,
            schema_count: 0,
            callback_count: 0,
        };
        let output = render_overview(&data, true);
        assert!(!output.contains("Auth:"), "Auth line should be omitted");
    }

    #[test]
    fn test_render_overview_with_description() {
        let data = OverviewData {
            title: "Test".to_string(),
            description: Some("A simple API".to_string()),
            base_urls: vec!["https://api.example.com".to_string()],
            server_variables: vec![],
            auth_schemes: vec![],
            resource_count: 0,
            schema_count: 0,
            callback_count: 0,
        };
        let output = render_overview(&data, true);
        assert!(output.contains("A simple API"), "Missing description");
    }

    #[test]
    fn test_render_overview_with_variables() {
        let data = OverviewData {
            title: "Test".to_string(),
            description: None,
            base_urls: vec!["https://prod.example.com".to_string()],
            server_variables: vec![ServerVar {
                name: "env".to_string(),
                required: false,
                description: Some("Environment name".to_string()),
                default: Some("prod".to_string()),
            }],
            auth_schemes: vec![],
            resource_count: 0,
            schema_count: 0,
            callback_count: 0,
        };
        let output = render_overview(&data, true);
        assert!(output.contains("Variables:"), "Missing variables section");
        assert!(output.contains("env"), "Missing variable name");
    }

    #[test]
    fn test_render_resource_list() {
        let groups = vec![
            crate::models::resource::ResourceGroup {
                slug: "pets".to_string(),
                display_name: "Pets".to_string(),
                description: Some("Pet management".to_string()),
                is_deprecated: false,
                is_alpha: false,
                endpoints: vec![],
            },
            crate::models::resource::ResourceGroup {
                slug: "old-pets".to_string(),
                display_name: "Old Pets".to_string(),
                description: Some("Legacy endpoints".to_string()),
                is_deprecated: true,
                is_alpha: false,
                endpoints: vec![],
            },
        ];
        let output = render_resource_list(&groups, true);
        assert!(output.contains("Resources:"), "Missing header");
        assert!(output.contains("pets"), "Missing pets");
        assert!(output.contains("[DEPRECATED]"), "Missing deprecated marker");
        assert!(
            output.find("[DEPRECATED]").unwrap() > output.find("old-pets").unwrap(),
            "DEPRECATED marker should be on the old-pets line"
        );
        assert!(output.contains("Drill deeper:"), "Missing drill deeper hint");
        assert!(
            output.contains("phyllotaxis resources <name>"),
            "Missing drill command"
        );
    }

    #[test]
    fn test_render_resource_detail() {
        use crate::models::resource::Endpoint;

        let group = ResourceGroup {
            slug: "pets".to_string(),
            display_name: "Pets".to_string(),
            description: Some("Pet management".to_string()),
            is_deprecated: false,
            is_alpha: false,
            endpoints: vec![
                Endpoint {
                    method: "GET".to_string(),
                    path: "/pets".to_string(),
                    summary: Some("List all pets".to_string()),
                    description: None,
                    is_deprecated: false,
                    is_alpha: false,
                    external_docs: None,
                    parameters: vec![],
                    request_body: None,
                    responses: vec![],
                    security_schemes: vec![],
                    callbacks: vec![],
                    links: vec![],
                    drill_deeper: vec![],
                },
                Endpoint {
                    method: "DELETE".to_string(),
                    path: "/pets/{id}".to_string(),
                    summary: Some("Delete a pet".to_string()),
                    description: None,
                    is_deprecated: true,
                    is_alpha: false,
                    external_docs: None,
                    parameters: vec![],
                    request_body: None,
                    responses: vec![],
                    security_schemes: vec![],
                    callbacks: vec![],
                    links: vec![],
                    drill_deeper: vec![],
                },
            ],
        };

        let output = render_resource_detail(&group, true);
        assert!(output.contains("Resource: Pets"));
        assert!(output.contains("Description: Pet management"));
        assert!(output.contains("GET") && output.contains("/pets"));
        assert!(
            output.contains("[DEPRECATED]"),
            "DELETE endpoint should be marked deprecated"
        );
        assert!(output.contains("Drill deeper:"));
        assert!(output.contains("phyllotaxis resources pets"));
    }

    #[test]
    fn test_render_schema_list() {
        let names = vec![
            "Owner".to_string(),
            "Pet".to_string(),
            "PetList".to_string(),
        ];
        let output = render_schema_list(&names, true);
        assert!(output.contains("Schemas (3 total):"), "Missing header with count");
        assert!(output.contains("  Pet"), "Missing Pet in list");
        assert!(output.contains("  Owner"), "Missing Owner in list");
        assert!(output.contains("Drill deeper:"), "Missing drill deeper");
        assert!(output.contains("phyllotaxis schemas <name>"), "Missing drill command");
    }

    #[test]
    fn test_render_schema_list_empty() {
        let output = render_schema_list(&[], true);
        assert!(output.contains("Schemas (0 total):"));
        assert!(output.contains("(none)"));
    }

    #[test]
    fn test_render_schema_detail_simple() {
        use crate::models::schema::SchemaModel;
        use crate::models::resource::Field;

        let model = SchemaModel {
            name: "Pet".to_string(),
            title: None,
            description: Some("A pet in the store".to_string()),
            fields: vec![
                Field {
                    name: "id".to_string(),
                    type_display: "string/uuid".to_string(),
                    required: true,
                    optional: false,
                    nullable: false,
                    read_only: true,
                    write_only: false,
                    deprecated: false,
                    constraints: vec![],
                    description: Some("Unique identifier".to_string()),
                    enum_values: vec![],
                    default_value: None,
                    example: None,
                    nested_schema_name: None,
                    nested_fields: vec![],
                },
                Field {
                    name: "status".to_string(),
                    type_display: "string".to_string(),
                    required: false,
                    optional: true,
                    nullable: false,
                    read_only: false,
                    write_only: false,
                    deprecated: false,
                    constraints: vec![],
                    description: None,
                    enum_values: vec!["available".to_string(), "sold".to_string()],
                    default_value: None,
                    example: None,
                    nested_schema_name: None,
                    nested_fields: vec![],
                },
            ],
            composition: None,
            discriminator: None,
            external_docs: None,
        };

        let output = render_schema_detail(&model, false, true);
        assert!(output.contains("Schema: Pet"), "Missing header");
        assert!(output.contains("A pet in the store"), "Missing description");
        assert!(output.contains("Fields:"), "Missing fields section");
        assert!(output.contains("string/uuid"), "Missing uuid type");
        assert!(output.contains("required, read-only"), "Missing modifiers");
        assert!(output.contains("Enum:"), "Missing enum values");
    }

    #[test]
    fn test_render_schema_detail_expanded() {
        use crate::models::schema::SchemaModel;
        use crate::models::resource::Field;

        let model = SchemaModel {
            name: "Pet".to_string(),
            title: None,
            description: None,
            fields: vec![
                Field {
                    name: "name".to_string(),
                    type_display: "string".to_string(),
                    required: true,
                    optional: false,
                    nullable: false,
                    read_only: false,
                    write_only: false,
                    deprecated: false,
                    constraints: vec![],
                    description: None,
                    enum_values: vec![],
                    default_value: None,
                    example: None,
                    nested_schema_name: None,
                    nested_fields: vec![],
                },
                Field {
                    name: "owner".to_string(),
                    type_display: "Owner".to_string(),
                    required: false,
                    optional: true,
                    nullable: false,
                    read_only: false,
                    write_only: false,
                    deprecated: false,
                    constraints: vec![],
                    description: None,
                    enum_values: vec![],
                    default_value: None,
                    example: None,
                    nested_schema_name: Some("Owner".to_string()),
                    nested_fields: vec![
                        Field {
                            name: "id".to_string(),
                            type_display: "string".to_string(),
                            required: false,
                            optional: false,
                            nullable: false,
                            read_only: true,
                            write_only: false,
                            deprecated: false,
                            constraints: vec![],
                            description: None,
                            enum_values: vec![],
                            default_value: None,
                            example: None,
                            nested_schema_name: None,
                            nested_fields: vec![],
                        },
                    ],
                },
            ],
            composition: None,
            discriminator: None,
            external_docs: None,
        };

        let output = render_schema_detail(&model, true, true);
        assert!(output.contains("Schema: Pet (expanded)"), "Missing expanded label");
        assert!(output.contains("Owner:"), "Missing nested schema colon");
        assert!(!output.contains("Related schemas"), "Related schemas should be hidden when expanded");
    }

    #[test]
    fn test_render_schema_detail_oneof() {
        use crate::models::schema::{SchemaModel, Composition};

        let model = SchemaModel {
            name: "PetOrOwner".to_string(),
            title: None,
            description: None,
            fields: vec![],
            composition: Some(Composition::OneOf(vec!["Pet".to_string(), "Owner".to_string()])),
            discriminator: None,
            external_docs: None,
        };

        let output = render_schema_detail(&model, false, true);
        assert!(output.contains("oneOf"), "Missing oneOf");
        assert!(output.contains("phyllotaxis schemas Pet"), "Missing Pet variant");
        assert!(output.contains("phyllotaxis schemas Owner"), "Missing Owner variant");
    }

    #[test]
    fn test_render_endpoint_detail_post_pets() {
        use crate::models::resource::*;

        let endpoint = Endpoint {
            method: "POST".to_string(),
            path: "/pets".to_string(),
            summary: Some("Create a pet".to_string()),
            description: None,
            is_deprecated: false,
            is_alpha: false,
            external_docs: None,
            parameters: vec![],
            request_body: Some(RequestBody {
                content_type: "application/json".to_string(),
                fields: vec![Field {
                    name: "name".to_string(),
                    type_display: "string".to_string(),
                    required: true,
                    optional: false,
                    nullable: false,
                    read_only: false,
                    write_only: false,
                    deprecated: false,
                    constraints: vec![],
                    description: Some("Pet name".to_string()),
                    enum_values: vec![],
                    default_value: None,
                    example: None,
                    nested_schema_name: None,
                    nested_fields: vec![],
                }],
                options: vec![],
                schema_ref: None,
                example: Some(serde_json::json!({"name": "Fido"})),
            }),
            responses: vec![
                Response {
                    status_code: "201".to_string(),
                    description: "Created".to_string(),
                    schema_ref: Some("Pet".to_string()),
                    example: None,
                    headers: vec![],
                    links: vec![],
                },
                Response {
                    status_code: "400".to_string(),
                    description: "Invalid input".to_string(),
                    schema_ref: None,
                    example: None,
                    headers: vec![],
                    links: vec![],
                },
            ],
            security_schemes: vec!["bearerAuth".to_string()],
            callbacks: vec![],
            links: vec![],
            drill_deeper: vec![],
        };

        let output = render_endpoint_detail(&endpoint, true);
        assert!(output.contains("POST /pets"), "Missing method/path");
        assert!(output.contains("Request Body"), "Missing request body section");
        assert!(
            output.contains("Authentication:"),
            "Missing authentication"
        );
        assert!(output.contains("Errors:"), "Missing errors section");
        assert!(output.contains("400"), "Missing 400 error");
        assert!(
            output.contains("Request Example:"),
            "Missing request example"
        );
    }

    #[test]
    fn test_render_endpoint_detail_drill_deeper_shown_on_tty() {
        use crate::models::resource::*;

        let endpoint = Endpoint {
            method: "GET".to_string(),
            path: "/pets/{id}".to_string(),
            summary: None,
            description: None,
            is_deprecated: false,
            is_alpha: false,
            external_docs: None,
            parameters: vec![],
            request_body: None,
            responses: vec![],
            security_schemes: vec![],
            callbacks: vec![],
            links: vec![],
            drill_deeper: vec!["phyllotaxis schemas Pet".to_string()],
        };

        let output = render_endpoint_detail(&endpoint, true);
        assert!(output.contains("Drill deeper:"), "Missing drill deeper header");
        assert!(
            output.contains("phyllotaxis schemas Pet"),
            "Missing drill deeper command"
        );
    }

    #[test]
    fn test_render_endpoint_detail_drill_deeper_hidden_off_tty() {
        use crate::models::resource::*;

        let endpoint = Endpoint {
            method: "GET".to_string(),
            path: "/pets/{id}".to_string(),
            summary: None,
            description: None,
            is_deprecated: false,
            is_alpha: false,
            external_docs: None,
            parameters: vec![],
            request_body: None,
            responses: vec![],
            security_schemes: vec![],
            callbacks: vec![],
            links: vec![],
            drill_deeper: vec!["phyllotaxis schemas Pet".to_string()],
        };

        let output = render_endpoint_detail(&endpoint, false);
        assert!(
            !output.contains("Drill deeper:"),
            "Drill deeper should be hidden when not a TTY"
        );
    }

    #[test]
    fn test_render_endpoint_detail_no_drill_deeper_when_empty() {
        use crate::models::resource::*;

        let endpoint = Endpoint {
            method: "GET".to_string(),
            path: "/pets".to_string(),
            summary: None,
            description: None,
            is_deprecated: false,
            is_alpha: false,
            external_docs: None,
            parameters: vec![],
            request_body: None,
            responses: vec![],
            security_schemes: vec![],
            callbacks: vec![],
            links: vec![],
            drill_deeper: vec![],
        };

        let output = render_endpoint_detail(&endpoint, true);
        assert!(
            !output.contains("Drill deeper:"),
            "Drill deeper section should not appear when empty"
        );
    }

    #[test]
    fn test_render_search_endpoint_includes_drill_command() {
        use crate::commands::search::*;

        let results = SearchResults {
            term: "pets".to_string(),
            resources: vec![],
            endpoints: vec![EndpointMatch {
                method: "GET".to_string(),
                path: "/pets/{id}".to_string(),
                summary: None,
                resource_slug: "pets".to_string(),
            }],
            schemas: vec![],
            callbacks: vec![],
        };

        let output = render_search(&results, true);
        assert!(
            output.contains("phyllotaxis resources pets GET /pets/{id}"),
            "Should include drill-down command, got:\n{}",
            output
        );
    }

    #[test]
    fn test_render_search_endpoint_no_slug_omits_drill_command() {
        use crate::commands::search::*;

        let results = SearchResults {
            term: "test".to_string(),
            resources: vec![],
            endpoints: vec![EndpointMatch {
                method: "GET".to_string(),
                path: "/test".to_string(),
                summary: None,
                resource_slug: "".to_string(),
            }],
            schemas: vec![],
            callbacks: vec![],
        };

        let output = render_search(&results, true);
        assert!(
            !output.contains("phyllotaxis resources  GET"),
            "Should not include drill command when slug is empty"
        );
    }

    #[test]
    fn test_render_search_drill_command_shown_off_tty() {
        use crate::commands::search::*;

        let results = SearchResults {
            term: "pets".to_string(),
            resources: vec![],
            endpoints: vec![EndpointMatch {
                method: "GET".to_string(),
                path: "/pets/{id}".to_string(),
                summary: None,
                resource_slug: "pets".to_string(),
            }],
            schemas: vec![],
            callbacks: vec![],
        };

        let output = render_search(&results, false);
        assert!(
            output.contains("phyllotaxis resources pets GET /pets/{id}"),
            "Drill command should appear even when piped (not TTY)"
        );
    }

    // ─── Task 15: write_only, deprecated, constraints rendering ───

    #[test]
    fn test_render_write_only_flag() {
        use crate::models::resource::Field;
        use crate::models::schema::SchemaModel;

        let model = SchemaModel {
            name: "Test".to_string(),
            title: None,
            description: None,
            fields: vec![Field {
                name: "password".to_string(),
                type_display: "string".to_string(),
                required: true,
                optional: false,
                nullable: false,
                read_only: false,
                write_only: true,
                deprecated: false,
                description: None,
                enum_values: vec![],
                constraints: vec![],
                default_value: None,
                example: None,
                nested_schema_name: None,
                nested_fields: vec![],
            }],
            composition: None,
            discriminator: None,
            external_docs: None,
        };
        let output = render_schema_detail(&model, false, false);
        assert!(output.contains("write-only"), "Missing write-only flag, got:\n{}", output);
    }

    #[test]
    fn test_render_deprecated_field_flag() {
        use crate::models::resource::Field;
        use crate::models::schema::SchemaModel;

        let model = SchemaModel {
            name: "Test".to_string(),
            title: None,
            description: None,
            fields: vec![Field {
                name: "legacy_code".to_string(),
                type_display: "string".to_string(),
                required: false,
                optional: true,
                nullable: false,
                read_only: false,
                write_only: false,
                deprecated: true,
                description: None,
                enum_values: vec![],
                constraints: vec![],
                default_value: None,
                example: None,
                nested_schema_name: None,
                nested_fields: vec![],
            }],
            composition: None,
            discriminator: None,
            external_docs: None,
        };
        let output = render_schema_detail(&model, false, false);
        assert!(output.contains("DEPRECATED"), "Missing DEPRECATED flag, got:\n{}", output);
    }

    #[test]
    fn test_render_constraints_inline() {
        use crate::models::resource::Field;
        use crate::models::schema::SchemaModel;

        let model = SchemaModel {
            name: "Test".to_string(),
            title: None,
            description: None,
            fields: vec![Field {
                name: "username".to_string(),
                type_display: "string".to_string(),
                required: true,
                optional: false,
                nullable: false,
                read_only: false,
                write_only: false,
                deprecated: false,
                description: Some("Unique username".to_string()),
                enum_values: vec![],
                constraints: vec!["min:3".to_string(), "max:32".to_string(), "pattern:^[a-zA-Z0-9_-]+$".to_string()],
                default_value: None,
                example: None,
                nested_schema_name: None,
                nested_fields: vec![],
            }],
            composition: None,
            discriminator: None,
            external_docs: None,
        };
        let output = render_schema_detail(&model, false, false);
        assert!(output.contains("min:3"), "Missing min:3, got:\n{}", output);
        assert!(output.contains("max:32"), "Missing max:32, got:\n{}", output);
        assert!(output.contains("pattern:"), "Missing pattern, got:\n{}", output);
    }

    // ─── Task 16: Response headers rendering ───

    #[test]
    fn test_render_response_headers() {
        use crate::models::resource::*;

        let endpoint = Endpoint {
            method: "GET".to_string(),
            path: "/users".to_string(),
            summary: None,
            description: None,
            is_deprecated: false,
            is_alpha: false,
            external_docs: None,
            parameters: vec![],
            request_body: None,
            responses: vec![Response {
                status_code: "200".to_string(),
                description: "OK".to_string(),
                schema_ref: None,
                example: None,
                headers: vec![
                    ResponseHeader {
                        name: "X-Total-Count".to_string(),
                        type_display: "integer".to_string(),
                        description: Some("Total count".to_string()),
                    },
                ],
                links: vec![],
            }],
            security_schemes: vec![],
            callbacks: vec![],
            links: vec![],
            drill_deeper: vec![],
        };

        let output = render_endpoint_detail(&endpoint, false);
        assert!(output.contains("X-Total-Count"), "Missing header name, got:\n{}", output);
        assert!(output.contains("integer"), "Missing header type, got:\n{}", output);
    }

    // ─── Task 17: Links rendering ───

    #[test]
    fn test_render_links_section() {
        use crate::models::resource::*;

        let endpoint = Endpoint {
            method: "POST".to_string(),
            path: "/users".to_string(),
            summary: None,
            description: None,
            is_deprecated: false,
            is_alpha: false,
            external_docs: None,
            parameters: vec![],
            request_body: None,
            responses: vec![Response {
                status_code: "201".to_string(),
                description: "Created".to_string(),
                schema_ref: None,
                example: None,
                headers: vec![],
                links: vec![ResponseLink {
                    name: "GetCreatedUser".to_string(),
                    operation_id: "getUser".to_string(),
                    parameters: vec!["userId = $response.body#/id".to_string()],
                    description: None,
                    drill_command: Some("phyllotaxis resources users GET /users/{userId}".to_string()),
                }],
            }],
            security_schemes: vec![],
            callbacks: vec![],
            links: vec![ResponseLink {
                name: "GetCreatedUser".to_string(),
                operation_id: "getUser".to_string(),
                parameters: vec!["userId = $response.body#/id".to_string()],
                description: None,
                drill_command: Some("phyllotaxis resources users GET /users/{userId}".to_string()),
            }],
            drill_deeper: vec![],
        };

        let output = render_endpoint_detail(&endpoint, false);
        assert!(output.contains("Links:"), "Missing Links section, got:\n{}", output);
        assert!(output.contains("GetCreatedUser"), "Missing link name, got:\n{}", output);
        assert!(output.contains("getUser"), "Missing operationId, got:\n{}", output);
        assert!(output.contains("userId = $response.body#/id"), "Missing parameter mapping, got:\n{}", output);
    }

    // ─── Task 18: Callbacks inline rendering ───

    #[test]
    fn test_render_callbacks_inline() {
        use crate::models::resource::*;

        let endpoint = Endpoint {
            method: "POST".to_string(),
            path: "/notifications/subscribe".to_string(),
            summary: None,
            description: None,
            is_deprecated: false,
            is_alpha: false,
            external_docs: None,
            parameters: vec![],
            request_body: None,
            responses: vec![],
            security_schemes: vec![],
            callbacks: vec![CallbackEntry {
                name: "onEvent".to_string(),
                defined_on_operation_id: Some("subscribeNotifications".to_string()),
                defined_on_method: "POST".to_string(),
                defined_on_path: "/notifications/subscribe".to_string(),
                operations: vec![CallbackOperation {
                    method: "POST".to_string(),
                    url_expression: "{$request.query.callbackUrl}/events".to_string(),
                    summary: Some("Event notification callback".to_string()),
                    body_schema: Some("EventPayload".to_string()),
                    responses: vec![
                        CallbackResponse { status_code: "200".to_string(), description: "Acknowledged".to_string() },
                    ],
                }],
            }],
            links: vec![],
            drill_deeper: vec![],
        };

        let output = render_endpoint_detail(&endpoint, false);
        assert!(output.contains("Callbacks:"), "Missing Callbacks section, got:\n{}", output);
        assert!(output.contains("onEvent"), "Missing callback name, got:\n{}", output);
        assert!(output.contains("EventPayload"), "Missing body schema, got:\n{}", output);
        assert!(output.contains("{$request.query.callbackUrl}/events"), "Missing URL expression, got:\n{}", output);
    }

    // ─── Task 19: Schema title rendering ───

    #[test]
    fn test_render_schema_title_shown_when_different() {
        use crate::models::schema::SchemaModel;

        let model = SchemaModel {
            name: "GeoLocation".to_string(),
            title: Some("Geographic Location".to_string()),
            description: Some("GPS coordinates".to_string()),
            fields: vec![],
            composition: None,
            discriminator: None,
            external_docs: None,
        };

        let output = render_schema_detail(&model, false, false);
        assert!(output.contains("Schema: GeoLocation"), "Missing schema name, got:\n{}", output);
        assert!(output.contains("Geographic Location"), "Missing title, got:\n{}", output);
    }

    #[test]
    fn test_render_schema_title_hidden_when_same_as_name() {
        use crate::models::schema::SchemaModel;

        let model = SchemaModel {
            name: "User".to_string(),
            title: Some("User".to_string()),
            description: None,
            fields: vec![],
            composition: None,
            discriminator: None,
            external_docs: None,
        };

        let output = render_schema_detail(&model, false, false);
        assert!(!output.contains("Title:"), "Title should be hidden when same as name, got:\n{}", output);
    }

    // ─── Task 22: Callback list/detail rendering ───

    #[test]
    fn test_render_callback_list() {
        use crate::models::resource::*;
        let callbacks = vec![
            CallbackEntry {
                name: "onEvent".to_string(),
                defined_on_operation_id: Some("subscribeNotifications".to_string()),
                defined_on_method: "POST".to_string(),
                defined_on_path: "/notifications/subscribe".to_string(),
                operations: vec![],
            },
        ];
        let output = render_callback_list(&callbacks, true);
        assert!(output.contains("Callbacks"), "Missing header");
        assert!(output.contains("onEvent"), "Missing callback name");
        assert!(output.contains("phyllotaxis callbacks <name>"), "Missing drill hint");
    }

    #[test]
    fn test_render_callback_detail() {
        use crate::models::resource::*;
        let cb = CallbackEntry {
            name: "onEvent".to_string(),
            defined_on_operation_id: Some("subscribeNotifications".to_string()),
            defined_on_method: "POST".to_string(),
            defined_on_path: "/notifications/subscribe".to_string(),
            operations: vec![CallbackOperation {
                method: "POST".to_string(),
                url_expression: "{$request.query.callbackUrl}/events".to_string(),
                summary: None,
                body_schema: Some("EventPayload".to_string()),
                responses: vec![
                    CallbackResponse { status_code: "200".to_string(), description: "OK".to_string() },
                ],
            }],
        };
        let output = render_callback_detail(&cb, false);
        assert!(output.contains("Callback: onEvent"), "Missing callback name, got:\n{}", output);
        assert!(output.contains("POST /notifications/subscribe"), "Missing defined-on line, got:\n{}", output);
        assert!(output.contains("EventPayload"), "Missing body schema, got:\n{}", output);
        assert!(output.contains("200"), "Missing response code, got:\n{}", output);
    }
}
