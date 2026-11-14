use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use crate::dependencies::Count;
use crate::frontmatter::{self, Front};
use crate::idea::IdeaKind;
use crate::okf::{self, GENERATED_BY, LENGUA_CHECK};
use crate::text::is_fence;
use crate::types::render;
use crate::web::Route;
use crate::{Finding, Label, Library, Note, Shape, Sink, Trust};

pub(crate) struct Parts<'a> {
    pub title: &'a str,
    pub shape: Shape,
    pub libraries: &'a [Library],
    pub components: &'a [(String, String, usize)],
    pub crates: Count,
    pub vision: bool,
    pub interface: &'a Interface<'a>,
}

pub(crate) struct Interface<'a> {
    pub routes: &'a [Route],
    pub calls: &'a [(String, Vec<String>)],
    pub client_hash: Option<String>,
    pub packages: &'a [(String, String)],
    pub large: &'a [(String, u64)],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Stamp {
    pub key: &'static str,
    pub file: &'static str,
    pub hash: String,
}

#[derive(Clone, Debug)]
pub(crate) struct Page {
    pub file: String,
    pub kind: IdeaKind,
    pub name: String,
    pub title: String,
    pub description: String,
    pub uses: Vec<String>,
    pub front: Front,
    pub idea_hash: String,
    pub source: String,
    pub source_hash: Option<String>,
    pub generated: Vec<Stamp>,
}

impl Page {
    pub(crate) fn id(&self) -> &str {
        self.file.strip_suffix(".md").unwrap_or(&self.file)
    }

    fn previous_idea(&self) -> Option<&str> {
        self.front.value("idea_sha256")
    }

    fn previous_source(&self) -> Option<&str> {
        self.front.value("source_sha256").filter(|v| *v != "none")
    }
}

pub(crate) fn without_links(body: &str) -> String {
    let lines: Vec<&str> = body.split_inclusive('\n').collect();
    let mut fence = false;
    let mut last: Option<(usize, bool)> = None;
    for (i, line) in lines.iter().enumerate() {
        if is_fence(line) {
            fence = !fence;
        } else if !fence && line.starts_with("# ") {
            last = Some((i, line.trim_end() == "# Links"));
        }
    }
    let kept = if let Some((i, true)) = last {
        lines[..i].concat()
    } else {
        body.to_string()
    };
    format!("{}\n", kept.trim_end_matches('\n'))
}

pub(crate) fn date(now: u64) -> (u64, u64, u64) {
    let z = now / 86_400 + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + u64::from(month <= 2);
    (year, month, day)
}

fn day_text(now: u64) -> String {
    let (year, month, day) = date(now);
    format!("{year:04}-{month:02}-{day:02}")
}

fn time_text(now: u64) -> String {
    let seconds = now % 86_400;
    format!(
        "{:02}:{:02}:{:02}Z",
        seconds / 3600,
        seconds % 3600 / 60,
        seconds % 60
    )
}

pub(crate) fn iso(now: u64) -> String {
    format!("{}T{}", day_text(now), time_text(now))
}

pub(crate) fn quoted(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' | '\r' | '\t' => out.push(' '),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

pub(crate) fn scalar(text: &str) -> String {
    let special = text.is_empty()
        || text.contains(": ")
        || text.starts_with(['-', '?', '!', '&', '*', '|', '>', '%', '@', '`', '\'', ' '])
        || text.ends_with(' ')
        || text.contains(['"', ',', '{', '}', '[', ']', '#']);
    if special {
        quoted(text)
    } else {
        text.to_string()
    }
}

fn flow(list: &[String]) -> String {
    format!("[{}]", list.join(", "))
}

fn type_name(kind: IdeaKind, shape: Shape) -> &'static str {
    match kind {
        IdeaKind::Program => match shape {
            Shape::Command => "lengua Program",
            Shape::Server => "lengua Server",
            Shape::Library => "lengua Library",
        },
        IdeaKind::Module => "lengua Module",
        IdeaKind::Screen => "lengua Screen",
    }
}

fn previous_generated(front: &Front) -> Option<String> {
    let maps = okf::mappings(front.entry("generated")?)?;
    maps.first()
        .and_then(|m| okf::get(m, "at"))
        .map(str::to_string)
}

fn previous_check(front: &Front) -> Option<String> {
    let maps = okf::mappings(front.entry("verified")?)?;
    maps.iter()
        .find(|m| okf::get(m, "by") == Some(LENGUA_CHECK))
        .and_then(|m| okf::get(m, "at"))
        .map(str::to_string)
}

fn verified_value(front: &Front, check: Option<&str>) -> Option<String> {
    let others = front
        .entry("verified")
        .and_then(okf::mappings)
        .unwrap_or_default();
    let mut lines: Vec<String> = others
        .iter()
        .filter(|m| okf::get(m, "by") != Some(LENGUA_CHECK))
        .map(|m| {
            let pairs: Vec<String> = m
                .iter()
                .map(|(k, v)| format!("{k}: {}", scalar(v)))
                .collect();
            format!("  - {{ {} }}", pairs.join(", "))
        })
        .collect();
    if let Some(at) = check {
        lines.push(format!("  - {{ by: {LENGUA_CHECK}, at: {at} }}"));
    }
    (!lines.is_empty()).then(|| format!("\n{}", lines.join("\n")))
}

struct Time<'a> {
    now: &'a str,
    keep: bool,
}

impl Time<'_> {
    fn at(&self, previous: Option<String>) -> String {
        previous
            .filter(|_| self.keep)
            .unwrap_or_else(|| self.now.to_string())
    }
}

fn generated_value(front: &Front, time: &Time) -> String {
    format!(
        "{{ by: {GENERATED_BY}, at: {} }}",
        time.at(previous_generated(front))
    )
}

pub(crate) fn component_page(file: &str) -> String {
    let stem = Path::new(file)
        .file_stem()
        .map_or_else(|| file.to_string(), |s| s.to_string_lossy().to_string());
    format!("components/{stem}.md")
}

fn component_name(file: &str) -> String {
    component_page(file)
        .trim_start_matches("components/")
        .trim_end_matches(".md")
        .to_string()
}

fn title_of(pages: &[Page], parts: &Parts, id: &str) -> String {
    if let Some(page) = pages.iter().find(|p| p.id() == id) {
        return page.title.clone();
    }
    parts
        .libraries
        .iter()
        .find(|l| format!("{}/idea", l.path) == id)
        .map_or_else(|| id.to_string(), |l| l.name.clone())
}

fn links_text(from: &str, parts: &Parts, groups: &[(&str, Vec<String>)]) -> String {
    let mut lines = Vec::new();
    for (what, links) in groups {
        if !links.is_empty() {
            lines.push(format!("* {what}: {}.\n", links.join(", ")));
        }
    }
    if parts.vision {
        lines.push(format!(
            "* Vision: [vision]({}).\n",
            okf::relative(from, "vision.md")
        ));
    }
    lines.push(format!(
        "* Index: [{}]({}).\n",
        parts.title,
        okf::relative(from, "index.md")
    ));
    format!("# Links\n\n{}", lines.concat())
}

fn idea_link(from: &str, id: &str, pages: &[Page], parts: &Parts) -> String {
    format!(
        "[{}]({})",
        title_of(pages, parts, id),
        okf::relative(from, &format!("{id}.md"))
    )
}

fn calls_route(interface: &Interface, page: &Page, route: &Route) -> bool {
    interface
        .calls
        .iter()
        .any(|(source, called)| *source == page.source && called.contains(&route.function))
}

fn interface_groups(
    page: &Page,
    pages: &[Page],
    parts: &Parts,
) -> Vec<(&'static str, Vec<String>)> {
    let interface = parts.interface;
    let api = okf::relative(&page.file, "api.md");
    let route_link = |r: &Route| format!("[`{} {}`]({api})", r.method, r.path);
    let calls: Vec<String> = interface
        .routes
        .iter()
        .filter(|r| calls_route(interface, page, r))
        .map(route_link)
        .collect();
    let served: Vec<&Route> = interface
        .routes
        .iter()
        .filter(|r| r.served_by.contains(&page.file))
        .collect();
    let mut callers: Vec<String> = Vec::new();
    for other in pages {
        let link = idea_link(&page.file, other.id(), pages, parts);
        if served.iter().any(|r| calls_route(interface, other, r)) && !callers.contains(&link) {
            callers.push(link);
        }
    }
    vec![
        ("Calls", calls),
        ("Serves", served.iter().map(|r| route_link(r)).collect()),
        ("Called by", callers),
    ]
}

pub(crate) const API_DESCRIPTION: &str = "The routes of the back end that the front end calls, with the shapes of their requests and answers.";

fn file_link(pages: &[Page], file: &str) -> String {
    let title = pages
        .iter()
        .find(|p| p.file == file)
        .map_or(file, |p| p.title.as_str());
    format!("[{title}]({file})")
}

pub(crate) fn api_text(pages: &[Page], label: &str, interface: &Interface) -> String {
    let mut out = format!(
        "---\ntype: lengua Interface\ndescription: {}\nlabel: {label}\nclient_sha256: {}\n---\n\n# Routes\n",
        quoted(API_DESCRIPTION),
        interface.client_hash.as_deref().unwrap_or("none")
    );
    for route in interface.routes {
        let (answer, failure) = if route.json {
            (render(&route.answer), render(&route.failure))
        } else {
            ("string".to_string(), "string".to_string())
        };
        let body = route
            .body
            .as_ref()
            .map_or("none".to_string(), |b| format!("`{}`", render(b)));
        let served: Vec<String> = route
            .served_by
            .iter()
            .map(|f| file_link(pages, f))
            .collect();
        let called: Vec<String> = pages
            .iter()
            .filter(|p| calls_route(interface, p, route))
            .map(|p| format!("[{}]({})", p.title, p.file))
            .collect();
        let called = if called.is_empty() {
            "no screen".to_string()
        } else {
            called.join(", ")
        };
        let _ = write!(
            out,
            "\n## `{} {}`\n\n* Function: `{}` in `web/lib/lengua.ts`.\n* Request body: {body}.\n* Answer: `{answer}`.\n* Failure: `{failure}`.\n* Served by: {}.\n* Called by: {called}.\n",
            route.method,
            route.path,
            route.function,
            served.join(", ")
        );
    }
    out.push_str("\n# Links\n\n* Index: [index](index.md).\n");
    out
}

fn page_text(
    page: &Page,
    used_by: &[String],
    label: &str,
    (pages, parts): (&[Page], &Parts),
    time: &Time,
) -> String {
    let source_hash = page
        .source_hash
        .clone()
        .unwrap_or_else(|| "none".to_string());
    let mut keys = vec![
        ("type", type_name(page.kind, parts.shape).to_string()),
        ("description", quoted(&page.description)),
        ("module", page.name.clone()),
        ("source", page.source.clone()),
        ("uses", flow(&page.uses)),
        ("used_by", flow(used_by)),
        ("label", label.to_string()),
        ("idea_sha256", page.idea_hash.clone()),
        ("source_sha256", source_hash),
    ];
    keys.extend(page.generated.iter().map(|s| (s.key, s.hash.clone())));
    keys.push(("generated", generated_value(&page.front, time)));
    let confirmed = label != Label::Unchecked.name();
    let check = confirmed.then(|| time.at(previous_check(&page.front)));
    if let Some(value) = verified_value(&page.front, check.as_deref()) {
        keys.push(("verified", value));
    }
    let absent = [
        ("title", scalar(&page.title)),
        ("status", "draft".to_string()),
    ];
    let link = |id: &String| idea_link(&page.file, id, pages, parts);
    let mut groups = vec![
        ("Uses", page.uses.iter().map(link).collect::<Vec<String>>()),
        ("Used by", used_by.iter().map(link).collect()),
    ];
    if page.kind == IdeaKind::Program {
        let components = parts
            .components
            .iter()
            .map(|(file, _, _)| {
                format!(
                    "[{}]({})",
                    component_name(file),
                    okf::relative(&page.file, &component_page(file))
                )
            })
            .collect();
        groups.push(("Components", components));
    }
    groups.extend(interface_groups(page, pages, parts));
    format!(
        "{}\n{}",
        page.front.render(&keys, &absent),
        links_text(&page.file, parts, &groups)
    )
}

fn render_without(front: &Front, keys: &[(&str, String)], absent: &[(&str, String)]) -> String {
    let mut kept = front.clone();
    kept.entries
        .retain(|e| !keys.iter().any(|(key, _)| *key == e.key));
    kept.render(keys, absent)
}

fn component_text(
    folder: &Path,
    (file, built): (&str, &str),
    pages: &[Page],
    parts: &Parts,
    time: &Time,
) -> String {
    let page = component_page(file);
    let old = fs::read_to_string(folder.join(&page))
        .unwrap_or_default()
        .replace("\r\n", "\n");
    let mut sink = Sink::new(&page);
    let mut front = frontmatter::split(&old, &mut sink);
    front.body = format!(
        "# Component\n\nThe program uses the component `{file}`, built from {built}. lengua did not build it and did not check its code, so its label is `trusted`, and it runs under the grants of the program.\n"
    );
    let keys = [
        ("type", "lengua Component".to_string()),
        (
            "description",
            quoted(&format!("A component built from {built}.")),
        ),
        ("resource", format!("/{file}")),
        ("label", "trusted".to_string()),
        ("generated", generated_value(&front, time)),
    ];
    let absent = [
        ("title", scalar(&component_name(file))),
        ("status", "draft".to_string()),
    ];
    let user = pages
        .first()
        .map_or_default(|top| vec![idea_link(&page, top.id(), pages, parts)]);
    format!(
        "{}\n{}",
        render_without(&front, &keys, &absent),
        links_text(&page, parts, &[("Used by", user)])
    )
}

fn index_line(title: &str, link: &str, description: &str, label: &str) -> String {
    format!("* [{title}]({link}) - {description} Label: `{label}`.\n")
}

pub(crate) fn index_text(pages: &[Page], label: &str, parts: &Parts) -> String {
    let mut out = String::from("---\nokf_version: \"0.2\"\n---\n");
    let of = |kind: IdeaKind| -> Vec<String> {
        pages
            .iter()
            .filter(|p| p.kind == kind)
            .map(|p| index_line(&p.title, &p.file, &p.description, label))
            .collect()
    };
    section(&mut out, "# Back end: program", &of(IdeaKind::Program));
    section(&mut out, "# Back end: modules", &of(IdeaKind::Module));
    let libraries: Vec<String> = parts
        .libraries
        .iter()
        .map(|l| {
            index_line(
                &l.name,
                &format!("{}/idea.md", l.path),
                &l.description,
                &l.label,
            )
        })
        .collect();
    let front = pages.iter().any(|p| p.kind == IdeaKind::Screen);
    let libraries: Vec<String> = libraries
        .into_iter()
        .zip(parts.libraries)
        .map(|(line, l)| {
            if front {
                format!(
                    "{line}* In the browser, `{}` is `@/lib/wasm/{}`.\n",
                    l.name, l.name
                )
            } else {
                line
            }
        })
        .collect();
    section(&mut out, "# Back end: libraries", &libraries);
    let components: Vec<String> = parts
        .components
        .iter()
        .map(|(file, built, _)| {
            index_line(
                &component_name(file),
                &component_page(file),
                &format!("A component built from {built}."),
                "trusted",
            )
        })
        .collect();
    section(&mut out, "# Back end: components", &components);
    section(&mut out, "# Back end: crates", &parts.crates.lines());
    let screens = of(IdeaKind::Screen);
    if !screens.is_empty() {
        section(&mut out, "# Front end: screens", &screens);
        section(
            &mut out,
            "# Front end: npm packages",
            &package_lines(parts.interface),
        );
    }
    if !parts.interface.routes.is_empty() {
        section(
            &mut out,
            "# Interface",
            &[index_line("api", "api.md", API_DESCRIPTION, label)],
        );
    }
    out
}

fn package_lines(interface: &Interface) -> Vec<String> {
    let mut lines = Vec::new();
    let packages = interface.packages;
    if !packages.is_empty() {
        let names: Vec<String> = packages.iter().map(|(n, v)| format!("`{n}` {v}")).collect();
        lines.push(format!(
            "* {} in `web/package.json` ({}), with the packages every front end has for its build and tests, all pinned in `web/pnpm-lock.yaml`.\n",
            packages.len(),
            names.join(", ")
        ));
    }
    if !interface.large.is_empty() {
        let names: Vec<String> = interface
            .large
            .iter()
            .map(|(n, size)| format!("`{n}` ({} KiB)", size / 1024))
            .collect();
        lines.push(format!(
            "* Large, over 1 MiB in pnpm's copy: {}.\n",
            names.join(", ")
        ));
    }
    lines
}

fn section(out: &mut String, heading: &str, lines: &[String]) {
    if !lines.is_empty() {
        out.push('\n');
        out.push_str(heading);
        out.push_str("\n\n");
        out.push_str(&lines.concat());
    }
}

fn change(previous: Option<&str>, now: Option<&str>) -> Option<&'static str> {
    match (previous, now) {
        (None, Some(_)) => Some("new"),
        (Some(_), None) => Some("removed"),
        (Some(a), Some(b)) if a != b => Some("changed"),
        _ => None,
    }
}

pub(crate) fn log_entry(pages: &[Page], now: u64, trust: &Trust) -> (String, Vec<Note>) {
    let mut lines = Vec::new();
    let mut notes = Vec::new();
    for page in pages {
        let idea = change(page.previous_idea(), Some(page.idea_hash.as_str()));
        let code = change(page.previous_source(), page.source_hash.as_deref());
        if let Some(what) = idea {
            lines.push(format!(
                "  * Idea [{}]({}), {what}: `{}`.\n",
                page.title, page.file, page.idea_hash
            ));
        }
        if let Some(what) = code {
            let hash = page.source_hash.as_deref().unwrap_or("none");
            lines.push(format!("  * Code `{}`, {what}: `{hash}`.\n", page.source));
        }
        let drift = match (idea, code) {
            (Some("changed"), None) if page.source_hash.is_some() => Some(format!(
                "the idea `{}` changed and its code `{}` did not",
                page.file, page.source
            )),
            (None, Some("changed")) => Some(format!(
                "the code `{}` changed and its idea `{}` did not",
                page.source, page.file
            )),
            _ => None,
        };
        if let Some(message) = drift {
            lines.push(format!("  * Note: {message}.\n"));
            notes.push(Note {
                file: page.file.clone(),
                message,
            });
        }
        for stamp in &page.generated {
            if let Some(what) = change(page.front.value(stamp.key), Some(stamp.hash.as_str())) {
                lines.push(format!(
                    "  * Generated `{}`, {what}: `{}`.\n",
                    stamp.file, stamp.hash
                ));
            }
        }
    }
    let head = format!(
        "* **Check** at {}: no problems; {}.\n",
        time_text(now),
        trust.text()
    );
    (format!("{head}{}", lines.concat()), notes)
}

pub(crate) fn log_text(previous: &str, entry: &str, now: u64) -> String {
    let day = format!("## {}", day_text(now));
    if previous.trim().is_empty() {
        return format!("# Log\n\n{day}\n\n{entry}");
    }
    let lines: Vec<&str> = previous.split_inclusive('\n').collect();
    let first = lines.iter().position(|l| l.starts_with("## "));
    match first {
        Some(i) if lines[i].trim_end() == day => {
            let mut at = i + 1;
            while lines.get(at).is_some_and(|l| l.trim().is_empty()) {
                at += 1;
            }
            format!("{}\n{entry}{}", lines[..=i].concat(), lines[at..].concat())
        }
        Some(i) => format!(
            "{}{day}\n\n{entry}\n{}",
            lines[..i].concat(),
            lines[i..].concat()
        ),
        None => {
            let base = previous.trim_end_matches('\n');
            format!("{base}\n\n{day}\n\n{entry}")
        }
    }
}

fn differs(folder: &Path, file: &str, text: &str) -> bool {
    !fs::read_to_string(folder.join(file)).is_ok_and(|old| old == text)
}

fn write_file(folder: &Path, file: &str, text: &str) -> Result<(), Finding> {
    let path = folder.join(file);
    let parent = path.parent().map_or(Ok(()), fs::create_dir_all);
    parent
        .and_then(|()| fs::write(&path, text))
        .map_err(|e| Finding {
            file: file.to_string(),
            line: 1,
            code: "wiki-write",
            message: format!("lengua could not write the file: {e}"),
        })
}

pub(crate) struct Written {
    pub files: Vec<String>,
    pub notes: Vec<Note>,
    pub findings: Vec<Finding>,
}

fn steady(
    folder: &Path,
    file: String,
    now: &str,
    make: impl Fn(&Time) -> String,
) -> (String, String) {
    let kept = make(&Time { now, keep: true });
    if differs(folder, &file, &kept) {
        let fresh = make(&Time { now, keep: false });
        (file, fresh)
    } else {
        (file, kept)
    }
}

fn texts(
    folder: &Path,
    pages: &[Page],
    generated: &[(String, String)],
    label: &str,
    parts: &Parts,
    now: &str,
) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for page in pages {
        let used_by: Vec<String> = pages
            .iter()
            .filter(|other| other.uses.iter().any(|u| u == page.id()))
            .map(|other| other.id().to_string())
            .collect();
        out.push(steady(folder, page.file.clone(), now, |time| {
            page_text(page, &used_by, label, (pages, parts), time)
        }));
    }
    for (file, built, _) in parts.components {
        out.push(steady(folder, component_page(file), now, |time| {
            component_text(folder, (file, built), pages, parts, time)
        }));
    }
    out.push(("index.md".to_string(), index_text(pages, label, parts)));
    if !parts.interface.routes.is_empty() {
        out.push((
            "api.md".to_string(),
            api_text(pages, label, parts.interface),
        ));
    }
    out.extend(generated.iter().cloned());
    out
}

pub(crate) fn write(
    folder: &Path,
    pages: &[Page],
    generated: &[(String, String)],
    now: u64,
    trust: Option<&Trust>,
    parts: &Parts,
) -> Written {
    let mut out = Written {
        files: Vec::new(),
        notes: Vec::new(),
        findings: Vec::new(),
    };
    let stamp = iso(now);
    let unchecked = Label::Unchecked.name();
    let known = [unchecked, Label::Tested.name(), Label::Checked.name()];
    let previous = pages
        .first()
        .and_then(|p| p.front.value("label"))
        .filter(|l| known.contains(l))
        .unwrap_or(unchecked);
    let label = trust.map_or(previous, |t| t.label.name());
    let mut planned = texts(folder, pages, generated, label, parts, &stamp);
    if !planned
        .iter()
        .any(|(file, text)| differs(folder, file, text))
    {
        return out;
    }
    if trust.is_none() && label != unchecked {
        planned = texts(folder, pages, generated, unchecked, parts, &stamp);
    }
    let shown = trust.copied().unwrap_or_default();
    let (entry, notes) = log_entry(pages, now, &shown);
    out.notes = notes;
    let previous_log = fs::read_to_string(folder.join("log.md"))
        .unwrap_or_default()
        .replace("\r\n", "\n");
    let at = planned
        .iter()
        .position(|(file, _)| file == "index.md")
        .map_or(planned.len(), |i| i + 1);
    planned.insert(
        at,
        ("log.md".to_string(), log_text(&previous_log, &entry, now)),
    );
    for (file, text) in planned {
        if !differs(folder, &file, &text) {
            continue;
        }
        match write_file(folder, &file, &text) {
            Ok(()) => out.files.push(file),
            Err(finding) => out.findings.push(finding),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{date, iso, log_text, quoted, scalar, time_text, without_links};

    const NOV_13_2026: u64 = 1_794_528_000;

    #[test]
    fn dates_come_from_the_seconds() {
        assert_eq!(date(0), (1970, 1, 1));
        assert_eq!(date(951_782_400), (2000, 2, 29));
        assert_eq!(date(NOV_13_2026), (2026, 11, 13));
        assert_eq!(time_text(NOV_13_2026 + 3661), "01:01:01Z");
        assert_eq!(iso(NOV_13_2026 + 3661), "2026-11-13T01:01:01Z");
    }

    #[test]
    fn a_new_log_has_a_heading_and_the_entry() {
        assert_eq!(
            log_text("", "* one\n", NOV_13_2026),
            "# Log\n\n## 2026-11-13\n\n* one\n"
        );
    }

    #[test]
    fn a_later_entry_on_the_same_day_goes_first() {
        let old = "# Log\n\n## 2026-11-13\n\n* one\n";
        assert_eq!(
            log_text(old, "* two\n", NOV_13_2026),
            "# Log\n\n## 2026-11-13\n\n* two\n* one\n"
        );
    }

    #[test]
    fn a_new_day_goes_above_the_old_days() {
        let old = "# Log\n\n## 2026-11-12\n\n* one\n";
        assert_eq!(
            log_text(old, "* two\n", NOV_13_2026),
            "# Log\n\n## 2026-11-13\n\n* two\n\n## 2026-11-12\n\n* one\n"
        );
    }

    #[test]
    fn descriptions_are_quoted_for_yaml() {
        assert_eq!(quoted("a \"b\" \\ c"), "\"a \\\"b\\\" \\\\ c\"");
        assert_eq!(scalar("words"), "words");
        assert_eq!(scalar("a, b"), "\"a, b\"");
    }

    #[test]
    fn the_links_section_is_left_out_of_the_idea() {
        let body = "# Purpose\n\nText.\n\n# Structure\n\n- `main`: runs.\n\n# Links\n\n* Index: [a](index.md).\n";
        assert_eq!(
            without_links(body),
            "# Purpose\n\nText.\n\n# Structure\n\n- `main`: runs.\n"
        );
        let fenced = "# Purpose\n\n```\n# Links\n```\n";
        assert_eq!(without_links(fenced), fenced);
        assert_eq!(
            without_links("# Purpose\n\nText.\n\n\n"),
            "# Purpose\n\nText.\n"
        );
    }
}
