use std::error::Error;

use crate::config::Config;
use crate::lexer::{Token, TokenKind};
use crate::parser::{HtmlGenerate, SharedLine};
use crate::stack;

use serde::Serialize;
use tinytemplate::TinyTemplate;

pub(crate) struct Generator<'generator> {
    tt: TinyTemplate<'generator>,
    cfg: &'generator Config,
}

impl<'generator> Generator<'generator> {
    pub(crate) fn new(cfg: &'generator Config) -> Result<Self, Box<dyn Error>> {
        let mut g = Generator {
            tt: TinyTemplate::new(),
            cfg,
        };
        g.init()?;
        Ok(g)
    }

    fn init(&mut self) -> Result<(), Box<dyn Error>> {
        self.tt.add_template(URL_TEMPLATE_NAME, URL_TEMPLATE)?;
        self.tt
            .add_template(SORTED_LIST_TEMPLATE_NAME, SORTED_LIST_TEMPLATE)?;
        self.tt
            .add_template(DISORDERED_LIST_TEMPLATE_NAME, DISORDERED_LIST_TEMPLATE)?;
        self.tt.add_template(TITLE_TEMPLATE_NAME, TITLE_TEMPLATE)?;
        self.tt.add_template(QUOTE_TEMPLATE_NAME, QUOTE_TEMPLATE)?;
        self.tt.add_template(IMG_TEMPLATE_NAME, IMG_TEMPLATE)?;
        self.tt.add_template(CODE_TEMPLATE_NAME, CODE_TEMPLATE)?;
        self.tt
            .add_template(TEXT_PARAGRAPH_TEMPLATE_NAME, TEXT_PARAGRAPH_TEMPLATE)?;
        self.tt
            .add_template(HEADER_TEMPLATE_NAME, HEADER_TEMPLATE)?;
        self.tt
            .add_template(BODY_BEGIN_TEMPLATE_NAME, BODY_BEGIN_TEMPLATE)?;

        self.tt
            .set_default_formatter(&tinytemplate::format_unescaped);
        Ok(())
    }

    fn gen_line(&self, tokens: &Vec<Token>) -> String {
        let mut stack: stack::Stack<(TokenKind, &str)> = stack::Stack::new();

        let mut text = String::new();

        for t in tokens {
            match t.kind() {
                TokenKind::Text => text.push_str(t.value()),
                TokenKind::CodeMark => {
                    let matched = stack.pop_or_push((t.kind(), t.value()), |e| {
                        e.0 == t.kind() && e.1 == t.value()
                    });
                    if matched.is_some() {
                        text.push_str("</code>");
                    } else {
                        text.push_str("<code>");
                    }
                }
                TokenKind::BoldMark => {
                    let matched = stack.pop_or_push((t.kind(), t.value()), |e| {
                        e.0 == t.kind() && e.1 == t.value()
                    });
                    if matched.is_some() {
                        text.push_str("</strong>");
                    } else {
                        text.push_str("<strong>");
                    }
                }
                TokenKind::ItalicMark => {
                    let matched = stack.pop_or_push((t.kind(), t.value()), |e| {
                        e.0 == t.kind() && e.1 == t.value()
                    });
                    if matched.is_some() {
                        text.push_str("</em>");
                    } else {
                        text.push_str("<em>");
                    }
                }
                TokenKind::ItalicBoldMark => {
                    let matched = stack.pop_or_push((t.kind(), t.value()), |e| {
                        e.0 == t.kind() && e.1 == t.value()
                    });
                    if matched.is_some() {
                        text.push_str("</em></strong>");
                    } else {
                        text.push_str("<strong><em>");
                    }
                }
                TokenKind::Url => {
                    if let (Some(show_name), Some(location)) =
                        (t.as_url().get_show_name(), t.as_url().get_location())
                    {
                        let s = self.gen_url(show_name, location);
                        text.push_str(&s);
                    }
                }
                TokenKind::AutoLink => {
                    let s = self.gen_url(t.value(), t.value());
                    text.push_str(&s);
                }
                TokenKind::Image => {
                    if let (Some(alt), Some(location)) =
                        (t.as_img().get_alt_name(), t.as_img().get_location())
                    {
                        let s = self.gen_image(alt, location);
                        text.push_str(&s);
                    }
                }
                _ => (),
            }
        }
        text
    }

    fn gen_url(&self, show_name: &str, location: &str) -> String {
        self.tt
            .render(
                URL_TEMPLATE_NAME,
                &UrlContext {
                    show_name,
                    location,
                },
            )
            .unwrap()
    }

    fn gen_image(&self, alt: &str, location: &str) -> String {
        self.tt
            .render(IMG_TEMPLATE_NAME, &ImageContext { alt, location })
            .unwrap()
    }
}

impl<'generator> HtmlGenerate for Generator<'generator> {
    fn header(&self) -> String {
        if !self.cfg.custom_html_header.is_empty() {
            return self.cfg.custom_html_header.clone();
        }
        let ctx = HeaderContext {
            css_href: &self.cfg.css_href,
        };
        self.tt.render(HEADER_TEMPLATE_NAME, &ctx).unwrap()
    }

    fn body_begin(&self) -> String {
        let ctx = BodyBeginContext {
            body_class: &self.cfg.add_class_on_body,
            article_class: &self.cfg.add_class_on_article,
        };
        self.tt.render(BODY_BEGIN_TEMPLATE_NAME, &ctx).unwrap()
    }

    fn body_end(&self) -> String {
        String::from("</article></body>")
    }

    fn body_title(&self, l: &SharedLine) -> String {
        let l = l.borrow();
        let level = l.get_mark().len();
        let value = self.gen_line(l.all());

        let ctx = TitleContext {
            is_l1: level == 1,
            is_l2: level == 2,
            is_l3: level == 3,
            is_l4: level == 4,
            text: value,
        };

        self.tt.render(TITLE_TEMPLATE_NAME, &ctx).unwrap()
    }

    fn body_dividling(&self, _l: &SharedLine) -> String {
        "<hr>".to_string()
    }

    fn body_normal(&self, ls: &[SharedLine]) -> String {
        let lines: Vec<String> = ls.iter().map(|l| self.gen_line(l.borrow().all())).collect();
        self.tt
            .render(
                TEXT_PARAGRAPH_TEMPLATE_NAME,
                &TextParagraphContext { lines },
            )
            .unwrap()
    }

    fn body_blank(&self, _ls: &[SharedLine]) -> String {
        // ls.iter().map(|_| "<p></p>").collect()
        "".to_string()
    }

    fn body_sorted_list(&self, ls: &[SharedLine]) -> String {
        let list: Vec<String> = ls
            .iter()
            .map(|l| {
                let leader = self.gen_line(l.borrow().all());
                let nesting = l
                    .borrow()
                    .enter_nested_blocks(&Generator::new(self.cfg).unwrap());
                if !nesting.is_empty() {
                    leader + "\n" + nesting.as_str()
                } else {
                    leader
                }
            })
            .collect();
        self.tt
            .render(SORTED_LIST_TEMPLATE_NAME, &SortedListContext { list })
            .unwrap()
    }

    fn body_disordered_list(&self, ls: &[SharedLine]) -> String {
        let list = ls
            .iter()
            .map(|l| {
                let leader = self.gen_line(l.borrow().all());
                let nesting = l
                    .borrow()
                    .enter_nested_blocks(&Generator::new(self.cfg).unwrap());
                if !nesting.is_empty() {
                    leader + "\n" + nesting.as_str()
                } else {
                    leader
                }
            })
            .collect();
        self.tt
            .render(
                DISORDERED_LIST_TEMPLATE_NAME,
                &DisorderedListContext { list },
            )
            .unwrap()
    }

    fn body_quote(&self, ls: &[SharedLine]) -> String {
        let lines: Vec<String> = ls.iter().map(|l| self.gen_line(l.borrow().all())).collect();
        self.tt
            .render(QUOTE_TEMPLATE_NAME, &QuoteContext { lines })
            .unwrap()
    }

    fn body_code(&self, ls: &[SharedLine]) -> String {
        debug_assert!(ls.len() >= 2);

        let first = &ls[0];
        let text: String = ls[1..ls.len() - 1] // skip the first and last elements
            .iter()
            .map(|l| l.borrow().text().to_string())
            .collect();

        self.tt
            .render(
                CODE_TEMPLATE_NAME,
                &CodeBlockContext {
                    name: first.borrow().get(1).map(|t| t.value()).unwrap_or(""),
                    text: &text,
                },
            )
            .unwrap()
    }
}

// header
const HEADER_TEMPLATE_NAME: &str = "header";
const HEADER_TEMPLATE: &str = r#"
<head>
<meta charset='UTF-8'><meta name='viewport' content='width=device-width initial-scale=1'>
{{ if css_href }} <link rel="stylesheet" type="text/css" href="{ css_href }"> {{ endif }}
<title></title>
</head>
"#;

#[derive(Serialize)]
struct HeaderContext<'header_context> {
    css_href: &'header_context str,
}

const BODY_BEGIN_TEMPLATE_NAME: &str = "body_begin";
const BODY_BEGIN_TEMPLATE: &str = r#"
<body{{ if body_class }} class={body_class}{{ endif }}>
<article{{ if article_class }} class={article_class}{{ endif }}>
"#;

#[derive(Serialize)]
struct BodyBeginContext<'body_begin_context> {
    body_class: &'body_begin_context str,
    article_class: &'body_begin_context str,
}

// title
const TITLE_TEMPLATE_NAME: &str = "title";
const TITLE_TEMPLATE: &str = "\
{{ if is_l1 }}<h1>{text}</h1>{{ endif }}\
{{ if is_l2 }}<h2>{text}</h2>{{ endif }}\
{{ if is_l3 }}<h3>{text}</h3>{{ endif }}\
{{ if is_l4 }}<h4>{text}</h4>{{ endif }}";

#[derive(Serialize)]
struct TitleContext {
    is_l1: bool,
    is_l2: bool,
    is_l3: bool,
    is_l4: bool,
    text: String,
}

// sorted list
const SORTED_LIST_TEMPLATE_NAME: &str = "sorted_list";
const SORTED_LIST_TEMPLATE: &str = "\
<ol>\
{{ for item in list }}
    <li>{item}</li>\
{{ endfor }} 
</ol>";

#[derive(Serialize)]
struct SortedListContext {
    list: Vec<String>,
}

// disordered list
const DISORDERED_LIST_TEMPLATE_NAME: &str = "disordered_list";
const DISORDERED_LIST_TEMPLATE: &str = "\
<ul>\
{{ for item in list }} 
    <li>{item}</li>\
{{ endfor }} 
</ul>";

#[derive(Serialize)]
struct DisorderedListContext {
    list: Vec<String>,
}

#[derive(Serialize)]
struct QuoteContext {
    lines: Vec<String>,
}

// url
const URL_TEMPLATE_NAME: &str = "url";
const URL_TEMPLATE: &str = r#"<a href="{location}">{show_name}</a>"#;

#[derive(Serialize)]
struct UrlContext<'uc> {
    show_name: &'uc str,
    location: &'uc str,
}

// image
const IMG_TEMPLATE_NAME: &str = "img";
const IMG_TEMPLATE: &str = r#"<img src="{location}" alt="{alt}">"#;

#[derive(Serialize)]
struct ImageContext<'ic> {
    alt: &'ic str,
    location: &'ic str,
}

// code block
const CODE_TEMPLATE_NAME: &str = "code_block";
const CODE_TEMPLATE: &str = r#"<pre><code class="language-{name}">
{text}
</code></pre>"#;

#[derive(Serialize)]
struct CodeBlockContext<'cbc> {
    name: &'cbc str,
    text: &'cbc str,
}

// normal text
const TEXT_PARAGRAPH_TEMPLATE_NAME: &str = "normal_text";
const TEXT_PARAGRAPH_TEMPLATE: &str = "\
<p>\
{{ for text in lines}}\
{text}<br>\
{{ endfor }}\
</p>";

// quote block
const QUOTE_TEMPLATE_NAME: &str = "quote";
const QUOTE_TEMPLATE: &str = "\
<blockquote>
<p>{{ for text in lines }} 
    {text}<br>\
{{ endfor }}
</p>
</blockquote>";

#[derive(Serialize)]
struct TextParagraphContext {
    lines: Vec<String>,
}
