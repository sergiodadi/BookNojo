//! Online book-metadata lookup ("Recherche en ligne" feature) — three
//! independent, free, keyless sources: Open Library, Google Books, and the
//! BnF's SRU Catalogue général. Each source's function is self-contained
//! and returns `Err` only for a genuine transport/parse failure; a source
//! simply having zero matches returns `Ok(vec![])`, never an error — the
//! caller (a future Tauri command) is expected to run all three and show
//! whatever comes back, without one failing source blocking the others.

use serde::Serialize;

/// One candidate book, normalized from whichever source returned it.
/// `source` is a short label ("Open Library"/"Google Books"/"BnF"), shown
/// to the user so they know which result they're picking — results are
/// deliberately never merged across sources.
#[derive(Debug, Clone, Serialize)]
pub struct BookSearchResult {
    pub source: &'static str,
    pub title: String,
    pub authors: Vec<String>,
    pub publisher: Option<String>,
    pub publication_year: Option<String>,
    pub isbn: Option<String>,
    pub cover_url: Option<String>,
}

/// Strips everything but letters/digits, so "978-0-14-032872-1" and
/// "978 0 14 032872 1" compare the same as "9780140328721".
fn normalize_isbn(query: &str) -> String {
    query.chars().filter(|c| c.is_ascii_alphanumeric()).collect()
}

/// True for a 10- or 13-digit ISBN (the final character of a 10-digit ISBN
/// may be the checksum letter 'X'), once hyphens/spaces are stripped — used
/// to decide whether a search goes through each source's ISBN-specific
/// lookup or its free-text title/author search.
pub fn looks_like_isbn(query: &str) -> bool {
    let cleaned = normalize_isbn(query);
    let len = cleaned.len();
    if len != 10 && len != 13 {
        return false;
    }
    cleaned
        .chars()
        .enumerate()
        .all(|(i, c)| c.is_ascii_digit() || (len == 10 && i == 9 && (c == 'X' || c == 'x')))
}

fn http_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        // BnF and Open Library both ask API clients to identify themselves;
        // Google Books doesn't require it but it costs nothing to send
        // everywhere.
        .user_agent("BookNojo (https://github.com/sergiodadi/BookNojo)")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())
}

/// Pulls the first 4-digit run out of a free-text date string — Open
/// Library's `publish_date` ("October 1, 1988") and Google Books'
/// `publishedDate` ("1988" or "1988-10-01") are both free text, not a
/// clean year.
fn extract_year(text: &str) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() < 4 {
        return None;
    }
    for start in 0..=(chars.len() - 4) {
        let candidate: String = chars[start..start + 4].iter().collect();
        if candidate.chars().all(|c| c.is_ascii_digit()) {
            return Some(candidate);
        }
    }
    None
}

// ---------------------------------------------------------------------
// Open Library
// ---------------------------------------------------------------------

/// `openlibrary.org` — keyless, no published hard rate limit (the
/// maintainers ask for a `User-Agent` and reasonable use instead). ISBN
/// lookups use the dedicated `api/books` endpoint (one exact match);
/// free-text queries use `search.json` (title/author, several candidates).
pub fn search_open_library(query: &str) -> Result<Vec<BookSearchResult>, String> {
    let client = http_client()?;
    if looks_like_isbn(query) {
        let isbn = normalize_isbn(query);
        let bibkey = format!("ISBN:{isbn}");
        let json: serde_json::Value = client
            .get("https://openlibrary.org/api/books")
            .query(&[("bibkeys", bibkey.as_str()), ("format", "json"), ("jscmd", "data")])
            .send()
            .map_err(|e| e.to_string())?
            .json()
            .map_err(|e| e.to_string())?;
        Ok(json
            .get(&bibkey)
            .map(|entry| open_library_result_from_data(entry, Some(isbn)))
            .into_iter()
            .collect())
    } else {
        let json: serde_json::Value = client
            .get("https://openlibrary.org/search.json")
            .query(&[("q", query), ("limit", "10")])
            .send()
            .map_err(|e| e.to_string())?
            .json()
            .map_err(|e| e.to_string())?;
        let docs = json.get("docs").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        Ok(docs.iter().map(open_library_result_from_search_doc).collect())
    }
}

fn open_library_result_from_data(entry: &serde_json::Value, isbn: Option<String>) -> BookSearchResult {
    let authors = entry
        .get("authors")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|a| a.get("name").and_then(|n| n.as_str()).map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let publisher = entry
        .get("publishers")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .map(str::to_string);
    let publication_year = entry
        .get("publish_date")
        .and_then(|v| v.as_str())
        .and_then(extract_year);
    let cover_url = entry
        .get("cover")
        .and_then(|c| c.get("medium"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    BookSearchResult {
        source: "Open Library",
        title: entry.get("title").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
        authors,
        publisher,
        publication_year,
        isbn,
        cover_url,
    }
}

fn open_library_result_from_search_doc(doc: &serde_json::Value) -> BookSearchResult {
    let authors = doc
        .get("author_name")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|a| a.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    let publisher = doc
        .get("publisher")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let isbn = doc
        .get("isbn")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.iter().find_map(|v| v.as_str()))
        .map(str::to_string);
    let cover_url = doc
        .get("cover_i")
        .and_then(|v| v.as_i64())
        .map(|id| format!("https://covers.openlibrary.org/b/id/{id}-M.jpg"));
    BookSearchResult {
        source: "Open Library",
        title: doc.get("title").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
        authors,
        publisher,
        publication_year: doc
            .get("first_publish_year")
            .and_then(|v| v.as_i64())
            .map(|y| y.to_string()),
        isbn,
        cover_url,
    }
}

// ---------------------------------------------------------------------
// Google Books
// ---------------------------------------------------------------------

/// `googleapis.com/books/v1` — keyless access shares one global quota
/// across every caller with no API key, so a `429` is expected and
/// transient, not a bug. Treated as "zero results" rather than a hard
/// error, so the caller doesn't need special-case handling for it.
pub fn search_google_books(query: &str) -> Result<Vec<BookSearchResult>, String> {
    let client = http_client()?;
    let q = if looks_like_isbn(query) {
        format!("isbn:{}", normalize_isbn(query))
    } else {
        query.to_string()
    };
    let resp = client
        .get("https://www.googleapis.com/books/v1/volumes")
        .query(&[("q", q.as_str())])
        .send()
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Ok(vec![]);
    }
    let json: serde_json::Value = resp.json().map_err(|e| e.to_string())?;
    let items = json.get("items").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    Ok(items.iter().filter_map(google_books_result_from_item).take(10).collect())
}

fn google_books_result_from_item(item: &serde_json::Value) -> Option<BookSearchResult> {
    let info = item.get("volumeInfo")?;
    let title = info.get("title").and_then(|v| v.as_str())?.to_string();
    let authors = info
        .get("authors")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|a| a.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    let publisher = info.get("publisher").and_then(|v| v.as_str()).map(str::to_string);
    let publication_year = info
        .get("publishedDate")
        .and_then(|v| v.as_str())
        .and_then(extract_year);
    let isbn = info
        .get("industryIdentifiers")
        .and_then(|v| v.as_array())
        .and_then(|arr| {
            arr.iter()
                .find(|id| id.get("type").and_then(|t| t.as_str()) == Some("ISBN_13"))
                .or_else(|| arr.iter().find(|id| id.get("type").and_then(|t| t.as_str()) == Some("ISBN_10")))
        })
        .and_then(|id| id.get("identifier"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let cover_url = info
        .get("imageLinks")
        .and_then(|v| v.get("thumbnail"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    Some(BookSearchResult {
        source: "Google Books",
        title,
        authors,
        publisher,
        publication_year,
        isbn,
        cover_url,
    })
}

// ---------------------------------------------------------------------
// BnF — Catalogue général (SRU)
// ---------------------------------------------------------------------

/// `catalogue.bnf.fr/api/SRU` — no authentication, no published rate
/// limit, but XML (UNIMARC/MarcXchange) only, never JSON. The only one of
/// the three sources likely to know French-only editions.
///
/// **Connectivity not verified from this development environment**: the
/// TLS handshake to `catalogue.bnf.fr` times out from here (confirmed via
/// `curl -v`, the TCP connection itself succeeds), for reasons unrelated
/// to this code — possibly a restriction specific to this sandboxed dev
/// machine's network egress, not necessarily present on the end user's own
/// Mac. The CQL query syntax and the UNIMARC field numbers used below
/// (010$a ISBN, 200$a title / $f author statement, 210$c publisher / $d
/// date) are per the BnF's own published SRU manual; the XML parser is
/// covered by a test against a hand-written fixture, but a real
/// end-to-end call should be verified once this is wired into the app.
pub fn search_bnf(query: &str) -> Result<Vec<BookSearchResult>, String> {
    let client = http_client()?;
    let cql = if looks_like_isbn(query) {
        format!("bib.fuzzyIsbn adj \"{}\"", normalize_isbn(query))
    } else {
        format!("bib.title adj \"{}\"", query.replace('"', "'"))
    };
    let body = client
        .get("https://catalogue.bnf.fr/api/SRU")
        .query(&[
            ("version", "1.2"),
            ("operation", "searchRetrieve"),
            ("query", cql.as_str()),
            ("maximumRecords", "10"),
        ])
        .send()
        .map_err(|e| e.to_string())?
        .text()
        .map_err(|e| e.to_string())?;
    parse_bnf_unimarc_xml(&body)
}

fn subfield(datafield: roxmltree::Node, code: &str) -> Option<String> {
    datafield
        .children()
        .find(|n| n.tag_name().name() == "subfield" && n.attribute("code") == Some(code))
        .and_then(|n| n.text())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn datafield<'a, 'input>(record: roxmltree::Node<'a, 'input>, tag: &str) -> Option<roxmltree::Node<'a, 'input>> {
    record
        .descendants()
        .find(|n| n.tag_name().name() == "datafield" && n.attribute("tag") == Some(tag))
}

fn parse_bnf_unimarc_xml(xml: &str) -> Result<Vec<BookSearchResult>, String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
    // Filters on `recordData` (one per hit), not `record`: the SRW
    // envelope's own per-hit wrapper (`<srw:record>`) and the MarcXchange
    // record it contains (`<record xmlns="info:lc/xmlns/marcxchange-v1">`)
    // share the exact same *local* name "record" once the namespace
    // prefix is dropped — matching on "record" directly double-counts
    // every real hit (caught by a fixture test expecting 1 result and
    // getting 2). `recordData` only exists once per hit, at the SRW
    // level, so it can't collide the same way.
    Ok(doc
        .descendants()
        .filter(|n| n.tag_name().name() == "recordData")
        .filter_map(bnf_result_from_record)
        .take(10)
        .collect())
}

fn bnf_result_from_record(record: roxmltree::Node) -> Option<BookSearchResult> {
    let title_field = datafield(record, "200")?;
    let title = subfield(title_field, "a")?;
    let authors = subfield(title_field, "f").map(|statement| vec![statement]).unwrap_or_default();
    let (publisher, publication_year) = match datafield(record, "210") {
        Some(f) => (subfield(f, "c"), subfield(f, "d").and_then(|d| extract_year(&d))),
        None => (None, None),
    };
    let isbn = datafield(record, "010").and_then(|f| subfield(f, "a"));
    Some(BookSearchResult {
        source: "BnF",
        title,
        authors,
        publisher,
        publication_year,
        isbn,
        cover_url: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_like_isbn_accepts_13_and_10_digit_forms_with_separators() {
        assert!(looks_like_isbn("9780140328721"));
        assert!(looks_like_isbn("978-0-14-032872-1"));
        assert!(looks_like_isbn("0140328726"));
        assert!(looks_like_isbn("014032872X"));
        assert!(!looks_like_isbn("Fantastic Mr Fox"));
        assert!(!looks_like_isbn("123"));
    }

    #[test]
    fn extract_year_finds_a_4_digit_run_in_free_text_dates() {
        assert_eq!(extract_year("October 1, 1988"), Some("1988".to_string()));
        assert_eq!(extract_year("1988-10-01"), Some("1988".to_string()));
        assert_eq!(extract_year("1988"), Some("1988".to_string()));
        assert_eq!(extract_year("unknown"), None);
    }

    #[test]
    fn open_library_result_from_data_parses_a_real_shaped_response() {
        let entry: serde_json::Value = serde_json::from_str(
            r#"{
                "title": "Fantastic Mr. Fox",
                "authors": [{"name": "Roald Dahl"}],
                "publishers": [{"name": "Puffin"}],
                "publish_date": "October 1, 1988",
                "cover": {"medium": "https://covers.openlibrary.org/b/id/15152634-M.jpg"}
            }"#,
        )
        .unwrap();
        let result = open_library_result_from_data(&entry, Some("9780140328721".to_string()));
        assert_eq!(result.title, "Fantastic Mr. Fox");
        assert_eq!(result.authors, vec!["Roald Dahl".to_string()]);
        assert_eq!(result.publisher, Some("Puffin".to_string()));
        assert_eq!(result.publication_year, Some("1988".to_string()));
        assert_eq!(result.isbn, Some("9780140328721".to_string()));
        assert!(result.cover_url.unwrap().contains("15152634"));
    }

    #[test]
    fn google_books_result_from_item_prefers_isbn_13_over_isbn_10() {
        let item: serde_json::Value = serde_json::from_str(
            r#"{
                "volumeInfo": {
                    "title": "Fantastic Mr. Fox",
                    "authors": ["Roald Dahl"],
                    "publisher": "Puffin",
                    "publishedDate": "1988-10-01",
                    "industryIdentifiers": [
                        {"type": "ISBN_10", "identifier": "0140328726"},
                        {"type": "ISBN_13", "identifier": "9780140328721"}
                    ],
                    "imageLinks": {"thumbnail": "https://example.com/cover.jpg"}
                }
            }"#,
        )
        .unwrap();
        let result = google_books_result_from_item(&item).unwrap();
        assert_eq!(result.title, "Fantastic Mr. Fox");
        assert_eq!(result.isbn, Some("9780140328721".to_string()));
        assert_eq!(result.publication_year, Some("1988".to_string()));
    }

    #[test]
    fn google_books_result_from_item_is_none_without_a_title() {
        let item: serde_json::Value = serde_json::from_str(r#"{"volumeInfo": {}}"#).unwrap();
        assert!(google_books_result_from_item(&item).is_none());
    }

    /// A hand-written fixture matching the BnF SRU manual's documented
    /// UNIMARC/MarcXchange shape (field 010/200/210) — not a real response,
    /// since this environment cannot reach `catalogue.bnf.fr` (see this
    /// module's doc comment on `search_bnf`). Exercises the parser's field
    /// extraction logic independent of live network access.
    const BNF_FIXTURE_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
    <srw:searchRetrieveResponse xmlns:srw="http://www.loc.gov/zing/srw/">
        <srw:records>
            <srw:record>
                <srw:recordData>
                    <record xmlns="info:lc/xmlns/marcxchange-v1">
                        <datafield tag="010" ind1=" " ind2=" ">
                            <subfield code="a">9780140328721</subfield>
                        </datafield>
                        <datafield tag="200" ind1="1" ind2=" ">
                            <subfield code="a">Fantastic Mr. Fox</subfield>
                            <subfield code="f">Roald Dahl</subfield>
                        </datafield>
                        <datafield tag="210" ind1=" " ind2=" ">
                            <subfield code="c">Puffin</subfield>
                            <subfield code="d">1988</subfield>
                        </datafield>
                    </record>
                </srw:recordData>
            </srw:record>
        </srw:records>
    </srw:searchRetrieveResponse>"#;

    #[test]
    fn parse_bnf_unimarc_xml_extracts_title_author_publisher_year_isbn() {
        let results = parse_bnf_unimarc_xml(BNF_FIXTURE_XML).unwrap();
        assert_eq!(results.len(), 1);
        let r = &results[0];
        assert_eq!(r.source, "BnF");
        assert_eq!(r.title, "Fantastic Mr. Fox");
        assert_eq!(r.authors, vec!["Roald Dahl".to_string()]);
        assert_eq!(r.publisher, Some("Puffin".to_string()));
        assert_eq!(r.publication_year, Some("1988".to_string()));
        assert_eq!(r.isbn, Some("9780140328721".to_string()));
    }

    #[test]
    fn parse_bnf_unimarc_xml_skips_a_record_with_no_title_field() {
        let xml = r#"<recordData>
            <record xmlns="info:lc/xmlns/marcxchange-v1">
                <datafield tag="010"><subfield code="a">9780140328721</subfield></datafield>
            </record>
        </recordData>"#;
        let results = parse_bnf_unimarc_xml(xml).unwrap();
        assert!(results.is_empty());
    }

    // Live network tests — real calls to the actual services, not mocks.
    // Only Open Library is asserted on: it's confirmed reachable and
    // deterministic from this environment. Google Books' keyless quota is
    // shared globally and can 429 at any time (handled as "zero results",
    // not a panic, so nothing to assert beyond "doesn't crash"). BnF is
    // unreachable from this dev environment entirely (see `search_bnf`'s
    // doc comment) — not exercised here.

    #[test]
    fn search_open_library_finds_a_known_isbn() {
        let results = search_open_library("978-0-14-032872-1").expect("network call failed");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Fantastic Mr. Fox");
        assert!(results[0].authors.contains(&"Roald Dahl".to_string()));
    }

    #[test]
    fn search_google_books_does_not_panic_on_a_known_isbn() {
        let _ = search_google_books("978-0-14-032872-1");
    }
}
