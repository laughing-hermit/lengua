use std::fmt::Write as _;
use std::fs;
use std::path::Path;

pub(crate) const TARGET: &str = "wasm32-unknown-unknown";
pub(crate) const WASM_BINDGEN: &str = "0.2.130";
pub(crate) const REQUIRED: [&str; 4] = ["gpui-pre", "gpui-pre-platform", "gpui-pre-web", "wasm-bindgen"];
pub(crate) const KIT: &str = "gpui-component";
pub(crate) const APP_CARGO: &str = "web/app/Cargo.toml";
pub(crate) const APP_LIB: &str = "web/app/lib.rs";
pub(crate) const FILES_CARGO: &str = "web/files/Cargo.toml";
pub(crate) const FILES_LIB: &str = "web/files/lib.rs";
pub(crate) const FILES_CRATE: &str = "web-files";
pub(crate) const OUT: &str = "target/lengua/web";
pub(crate) const ASSETS: &str = "assets";

pub(crate) struct Plan<'a> {
    pub package: &'a str,
    pub screens: &'a [String],
    pub start: &'a str,
    pub crates: &'a [(String, String)],
    pub libraries: &'a [(String, String)],
    pub assets: &'a [String],
}

impl Plan<'_> {
    pub(crate) fn fonts(&self) -> impl Iterator<Item = &String> {
        self.assets.iter().filter(|a| is_font(a))
    }
}

pub(crate) const LINTS: [&str; 11] = [
    "[lints.rust]",
    "warnings = \"deny\"",
    "unsafe_code = \"forbid\"",
    "unused_must_use = \"deny\"",
    "",
    "[lints.clippy]",
    "pedantic = { level = \"deny\", priority = -1 }",
    "unwrap_used = \"deny\"",
    "expect_used = \"deny\"",
    "indexing_slicing = \"deny\"",
    "",
];

#[must_use]
pub(crate) fn app_name(package: &str) -> String {
    format!("{package}-app")
}

#[must_use]
pub(crate) fn artifact(package: &str) -> String {
    format!(
        "target/{TARGET}/release/{}.wasm",
        app_name(package).replace('-', "_")
    )
}

pub(crate) fn app_cargo(plan: &Plan) -> String {
    let name = app_name(plan.package);
    let mut lines = vec![
        "[package]".to_string(),
        format!("name = \"{name}\""),
        "version = \"0.1.0\"".to_string(),
        "edition = \"2024\"".to_string(),
        "publish = false".to_string(),
        String::new(),
        "[lib]".to_string(),
        format!("name = \"{}\"", name.replace('-', "_")),
        "path = \"lib.rs\"".to_string(),
        "crate-type = [\"cdylib\"]".to_string(),
        String::new(),
        "[dependencies]".to_string(),
    ];
    let mut dependencies: Vec<(String, String)> = plan
        .crates
        .iter()
        .map(|(crate_name, version)| {
            (
                crate_name.clone(),
                format!(
                    "{crate_name} = {{ version = \"={version}\", default-features = false }}"
                ),
            )
        })
        .collect();
    dependencies.extend(plan.libraries.iter().map(|(library, path)| {
        (
            library.clone(),
            format!("{library} = {{ path = \"../../{path}\" }}"),
        )
    }));
    dependencies.sort();
    lines.extend(dependencies.into_iter().map(|(_, line)| line));
    lines.push(String::new());
    lines.extend(LINTS.iter().map(|l| (*l).to_string()));
    lines.pop();
    lines.push("missing_errors_doc = \"allow\"".to_string());
    lines.push("missing_panics_doc = \"allow\"".to_string());
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

pub(crate) fn app_lib(plan: &Plan) -> String {
    let mut text = String::new();
    for screen in plan.screens {
        let _ = write!(
            text,
            "#[path = \"../../src/ui/{screen}.rs\"]\npub mod {screen};\n"
        );
    }
    text.push_str("\n#[wasm_bindgen::prelude::wasm_bindgen(start)]\npub fn start() {\n    gpui_platform::web_init();\n    let platform = std::rc::Rc::new(gpui_web::WebPlatform::new(false));\n    let client = std::sync::Arc::new(platform.fetch_http_client());\n    gpui::Application::with_platform(platform)\n        .with_http_client(client)\n        .run(|cx: &mut gpui::App| {\n            let fonts = vec![\n");
    for font in plan.fonts() {
        let _ = writeln!(
            text,
            "                std::borrow::Cow::Borrowed(include_bytes!(\"../../{ASSETS}/{font}\").as_slice()),"
        );
    }
    let _ = write!(
        text,
        "            ];\n            let _ = cx.text_system().add_fonts(fonts);\n            {}::start(cx);\n        }});\n}}\n",
        plan.start
    );
    text
}

pub(crate) const LIBRARY_NAMES: [(&str, &str); 4] = [
    ("gpui-pre", "gpui"),
    ("gpui-pre-platform", "gpui_platform"),
    ("gpui-pre-web", "gpui_web"),
    ("gpui-pre-http-client", "http_client"),
];

pub(crate) fn is_font(asset: &str) -> bool {
    let lower = asset.to_ascii_lowercase();
    lower.starts_with("fonts/")
        && (lower.ends_with(".ttf") || lower.ends_with(".otf"))
        && !lower.contains('"')
}

pub(crate) fn files_cargo() -> String {
    format!(
        "[package]\nname = \"{FILES_CRATE}\"\nversion = \"0.1.0\"\nedition = \"2024\"\npublish = false\n\n[lib]\nname = \"{}\"\npath = \"lib.rs\"\n",
        FILES_CRATE.replace('-', "_")
    )
}

fn media_type(path: &str) -> &'static str {
    let extension = path.rsplit_once('.').map_or("", |(_, e)| e);
    match extension.to_ascii_lowercase().as_str() {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "json" => "application/json",
        "txt" => "text/plain; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

pub(crate) fn page(title: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{title}</title>\n<style>html,body{{margin:0;width:100%;height:100%;overflow:hidden;background:#fff}}canvas{{display:block;width:100%;height:100%}}</style>\n</head>\n<body data-lengua=\"loading\">\n<script type=\"module\">\nimport init from \"./app.js\";\ninit().then(() => {{ document.body.dataset.lengua = \"started\"; }}, (error) => {{ document.body.dataset.lengua = \"failed\"; console.error(error); }});\n</script>\n</body>\n</html>\n"
    )
}

pub(crate) fn files_lib(title: &str, assets: &[String]) -> String {
    let mut text = format!(
        "const PAGE: &str = r#\"{}\"#;\n\nconst SCRIPT: &[u8] = include_bytes!(\"../../{OUT}/app.js\");\n\nconst CODE: &[u8] = include_bytes!(\"../../{OUT}/app_bg.wasm\");\n",
        page(title)
    );
    for (index, asset) in assets.iter().filter(|a| !is_font(a)).enumerate() {
        let _ = write!(
            text,
            "\nconst ASSET_{index}: &[u8] = include_bytes!(\"../../{ASSETS}/{asset}\");\n"
        );
    }
    text.push_str("\n#[must_use]\npub fn asset(path: &str) -> Option<(&'static str, &'static [u8])> {\n    match path {\n        \"/\" | \"/index.html\" => Some((\"text/html; charset=utf-8\", PAGE.as_bytes())),\n        \"/app.js\" => Some((\"text/javascript; charset=utf-8\", SCRIPT)),\n        \"/app_bg.wasm\" => Some((\"application/wasm\", CODE)),\n");
    for (index, asset) in assets.iter().filter(|a| !is_font(a)).enumerate() {
        let _ = write!(
            text,
            "        \"/{ASSETS}/{asset}\" => Some((\"{}\", ASSET_{index})),\n",
            media_type(asset)
        );
    }
    text.push_str("        _ => None,\n    }\n}\n");
    text
}

pub(crate) fn assets(folder: &Path) -> Vec<String> {
    let mut found = Vec::new();
    crate::generate::collect(folder, ASSETS, &mut found);
    found
        .into_iter()
        .filter_map(|(path, _)| {
            path.strip_prefix(&format!("{ASSETS}/"))
                .map(str::to_string)
        })
        .filter(|path| {
            !path.contains(['"', '\\', '#', '\n', '\r'])
                && fs::metadata(folder.join(ASSETS).join(path)).is_ok()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Plan, app_cargo, app_lib, artifact, files_lib, media_type};

    fn plan<'a>(screens: &'a [String], crates: &'a [(String, String)]) -> Plan<'a> {
        Plan {
            package: "notes",
            screens,
            start: "list",
            crates,
            libraries: &[],
            assets: &[],
        }
    }

    #[test]
    fn the_app_is_a_cdylib_with_every_crate_and_no_default_features() {
        let crates = [
            ("gpui-pre".to_string(), "0.3.12".to_string()),
            ("wasm-bindgen".to_string(), "0.2.130".to_string()),
        ];
        let screens = ["list".to_string()];
        let mut with_library = plan(&screens, &crates);
        let libraries = [("json".to_string(), "../../libraries/json".to_string())];
        with_library.libraries = &libraries;
        let text = app_cargo(&with_library);
        assert!(text.starts_with("[package]\nname = \"notes-app\"\n"), "{text}");
        assert!(
            text.contains("[lib]\nname = \"notes_app\"\npath = \"lib.rs\"\ncrate-type = [\"cdylib\"]\n"),
            "{text}"
        );
        assert!(
            text.contains("gpui-pre = { version = \"=0.3.12\", default-features = false }\njson = { path = \"../../../../libraries/json\" }\nwasm-bindgen"),
            "{text}"
        );
        assert!(text.ends_with("indexing_slicing = \"deny\"\nmissing_errors_doc = \"allow\"\nmissing_panics_doc = \"allow\"\n"), "{text}");
        assert!(!text.contains("[profile"), "{text}");
        assert_eq!(
            artifact("my-notes"),
            "target/wasm32-unknown-unknown/release/my_notes_app.wasm"
        );
    }

    #[test]
    fn the_app_has_a_module_a_screen_and_starts_the_first() {
        let screens = ["edit".to_string(), "list".to_string()];
        let text = app_lib(&plan(&screens, &[]));
        assert!(text.starts_with(
            "#[path = \"../../src/ui/edit.rs\"]\npub mod edit;\n#[path = \"../../src/ui/list.rs\"]\npub mod list;\n"
        ));
        assert!(text.contains("            list::start(cx);\n"), "{text}");
        let fonts = ["fonts/Plex.ttf".to_string(), "logo.png".to_string()];
        let mut with_fonts = plan(&screens, &[]);
        with_fonts.assets = &fonts;
        let text = app_lib(&with_fonts);
        assert!(text.contains("include_bytes!(\"../../assets/fonts/Plex.ttf\").as_slice()"), "{text}");
        assert!(!text.contains("logo.png"), "{text}");
        assert!(text.contains("WebPlatform::new(false)"), "{text}");
    }

    #[test]
    fn the_files_serve_the_page_the_code_and_each_asset() {
        let text = files_lib("notes", &["logo.png".to_string(), "fonts/a.ttf".to_string()]);
        assert!(!text.contains("a.ttf"), "{text}");
        assert!(text.contains("<title>notes</title>"), "{text}");
        assert!(text.contains("include_bytes!(\"../../target/lengua/web/app_bg.wasm\")"));
        assert!(text.contains("const ASSET_0: &[u8] = include_bytes!(\"../../assets/logo.png\");"));
        assert!(text.contains("\"/assets/logo.png\" => Some((\"image/png\", ASSET_0)),"));
        assert!(text.contains("\"/\" | \"/index.html\" => Some((\"text/html; charset=utf-8\", PAGE.as_bytes())),"));
        assert_eq!(media_type("a/b.WOFF2"), "font/woff2");
        assert_eq!(media_type("none"), "application/octet-stream");
    }
}
