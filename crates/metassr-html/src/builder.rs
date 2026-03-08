use super::{html_props::HtmlProps, template::HtmlTemplate};
use anyhow::Result;
use std::{fmt::Display, fs::File, io::Write, path::PathBuf};

const LANG_TAG: &str = "%LANG%";
const HEAD_TAG: &str = "%HEAD%";
const BODY_TAG: &str = "%BODY%";
const SCRIPTS_TAG: &str = "%SCRIPTS%";
const STYLES_TAG: &str = "%STYLES%";

#[derive(Debug, Clone)]
pub struct HtmlOutput(String);

impl HtmlOutput {
    pub fn from(html: &str) -> Self {
        Self(html.to_string())
    }

    pub fn write(&self, path: PathBuf) -> Result<()> {
        let mut file = File::create(path)?;
        file.write_all(self.0.as_bytes())?;
        Ok(())
    }
}

impl Display for HtmlOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self.0))
    }
}

pub struct HtmlBuilder {
    template: HtmlTemplate,
    props: HtmlProps,
}

impl HtmlBuilder {
    pub fn new(template: HtmlTemplate, props: HtmlProps) -> Self {
        Self { template, props }
    }

    pub fn generate(&self) -> HtmlOutput {
        let scripts = self
            .props
            .scripts
            .iter()
            .map(|path| format!("<script src=\"{}\"></script>", path.display()))
            .collect::<Vec<String>>()
            .join("");

        let styles = self
            .props
            .styles
            .iter()
            .map(|path| format!("<link rel=\"stylesheet\" href=\"{}\">", path.display()))
            .collect::<Vec<String>>()
            .join("");

        HtmlOutput::from(
            &self
                .template
                .to_string()
                .replace(LANG_TAG, &self.props.lang)
                .replace(HEAD_TAG, &self.props.head)
                .replace(BODY_TAG, &self.props.body)
                .replace(SCRIPTS_TAG, &scripts)
                .replace(STYLES_TAG, &styles),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{HtmlBuilder};
    use crate::{html_props::HtmlPropsBuilder, template::HtmlTemplate};
    use scraper::{Html, Selector};

    #[test]
    fn generating_html_structure() {
        let props = HtmlPropsBuilder::new()
            .lang("en")
            .body("<div id=\"root\"><h1>Hello</h1></div>")
            .head("<meta charset=\"utf-8\" /><title>Test Page</title>")
            .scripts(vec!["main.js".into(), "react.js".into()])
            .styles(vec!["style.css".into()])
            .build();

        let html_output = HtmlBuilder::new(HtmlTemplate::default(), props).generate();
        let doc = Html::parse_document(&html_output.to_string());

        // Test lang attribute
        let html_sel = Selector::parse("html").unwrap();
        let html_el = doc.select(&html_sel).next().unwrap();
        assert_eq!(html_el.value().attr("lang"), Some("en"));

        // Test title
        let title_sel = Selector::parse("title").unwrap();
        let title = doc.select(&title_sel).next().unwrap();
        assert_eq!(title.text().collect::<String>().trim(), "Test Page");

        // Test script tags (order preserved)
        let script_sel = Selector::parse("script[src]").unwrap();
        let scripts: Vec<_> = doc.select(&script_sel).map(|s| s.value().attr("src").unwrap()).collect();
        assert_eq!(scripts, vec!["main.js", "react.js"]);

        // Test styles
        let style_sel = Selector::parse("link[rel=\"stylesheet\"]").unwrap();
        let styles: Vec<_> = doc.select(&style_sel).map(|s| s.value().attr("href").unwrap()).collect();
        assert_eq!(styles, vec!["style.css"]);

        // Test body content structure
        let root_sel = Selector::parse("#root h1").unwrap();
        assert!(doc.select(&root_sel).next().is_some());
    }

    #[test]
    fn no_scripts_or_styles_generates_clean_html() {
        let props = HtmlPropsBuilder::new()
            .lang("en")
            .body("<div id=\"root\"></div>")
            .head("<title>Minimal</title>")
            .build();

        let html_str = HtmlBuilder::new(HtmlTemplate::default(), props)
            .generate()
            .to_string();
        let doc = Html::parse_document(&html_str);

        // No scripts
        let script_sel = Selector::parse("script").unwrap();
        assert!(doc.select(&script_sel).next().is_none());

        // No styles
        let style_sel = Selector::parse("link[rel=\"stylesheet\"]").unwrap();
        assert!(doc.select(&style_sel).next().is_none());

        // Required structure intact
        assert!(doc.select(&Selector::parse("html").unwrap()).next().is_some());
        assert!(doc.select(&Selector::parse("head").unwrap()).next().is_some());
        assert!(doc.select(&Selector::parse("body").unwrap()).next().is_some());
    }
}

