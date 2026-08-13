use quick_xml::events::Event;
use quick_xml::Reader;

/// Parse un sitemap XML standard (<urlset><url><loc>...</loc></url></urlset>)
/// et retourne la liste des URLs trouvees. Les entrees vides sont ignorees.
pub fn parse_sitemap_xml(xml: &str) -> Result<Vec<String>, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut urls = Vec::new();
    let mut in_loc = false;
    let mut buf = Vec::new();
    let mut current = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"loc" => {
                in_loc = true;
                current.clear();
            }
            Ok(Event::End(ref e)) if e.name().as_ref() == b"loc" => {
                if !current.trim().is_empty() {
                    urls.push(current.trim().to_string());
                }
                in_loc = false;
                current.clear();
            }
            Ok(Event::Text(e)) if in_loc => {
                let text = e.unescape().map_err(|e| format!("xml text: {}", e))?;
                current.push_str(&text);
            }
            Ok(Event::CData(e)) if in_loc => {
                let text =
                    std::str::from_utf8(e.as_ref()).map_err(|e| format!("xml cdata: {}", e))?;
                current.push_str(text);
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("xml parse error: {}", e)),
            _ => {}
        }
        buf.clear();
    }

    Ok(urls)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_sitemap() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url><loc>https://example.com/page1</loc></url>
  <url><loc>https://example.com/page2</loc></url>
</urlset>"#;
        let urls = parse_sitemap_xml(xml).unwrap();
        assert_eq!(urls, vec!["https://example.com/page1", "https://example.com/page2"]);
    }

    #[test]
    fn test_parse_with_cdata() {
        let xml = r#"<urlset><url><loc><![CDATA[https://example.com/a?x=1&y=2]]></loc></url></urlset>"#;
        let urls = parse_sitemap_xml(xml).unwrap();
        assert_eq!(urls, vec!["https://example.com/a?x=1&y=2"]);
    }

    #[test]
    fn test_parse_ignores_empty_loc() {
        let xml = r#"<urlset><url><loc></loc></url><url><loc>https://example.com/ok</loc></url></urlset>"#;
        let urls = parse_sitemap_xml(xml).unwrap();
        assert_eq!(urls, vec!["https://example.com/ok"]);
    }

    #[test]
    fn test_parse_truncated_xml_is_tolerated() {
        // quick-xml tolere les tags non fermes a EOF : on ne doit pas paniquer.
        let xml = "<urlset><url><loc>not closed";
        let res = parse_sitemap_xml(xml);
        assert!(res.is_ok());
        assert!(res.unwrap().is_empty());
    }

    #[test]
    fn test_parse_multi_url_with_other_elements() {
        let xml = r#"<urlset>
          <url><loc>https://a.com/p1</loc><lastmod>2026-01-01</lastmod></url>
          <url><loc>https://a.com/p2</loc><priority>0.8</priority></url>
        </urlset>"#;
        let urls = parse_sitemap_xml(xml).unwrap();
        assert_eq!(urls, vec!["https://a.com/p1", "https://a.com/p2"]);
    }
}
