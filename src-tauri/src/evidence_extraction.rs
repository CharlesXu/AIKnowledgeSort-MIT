use crate::identity::ContentIdentity;
use quick_xml::events::Event;
use quick_xml::Reader;
use serde::{Deserialize, Serialize};
use std::io::{Cursor, Read};

pub(crate) const MAX_EVIDENCE_SOURCE_BYTES: usize = 16 * 1024 * 1024;
const MAX_DOCX_XML_BYTES: usize = 4 * 1024 * 1024;
const MAX_EXCERPTS: usize = 16;
const MAX_EXCERPT_CHARS: usize = 4_096;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum EvidenceFormat {
    Text,
    Docx,
    Pdf,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct FileEvidenceExcerpt {
    pub evidence_id: String,
    pub location: String,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ExtractedFileEvidence {
    pub source_identity: ContentIdentity,
    pub format: EvidenceFormat,
    pub excerpts: Vec<FileEvidenceExcerpt>,
    pub truncated: bool,
}

pub(crate) fn extract_file_evidence(
    source_basename: &str,
    source_bytes: &[u8],
    expected_identity: &ContentIdentity,
) -> Result<ExtractedFileEvidence, String> {
    expected_identity.validate()?;
    if source_bytes.is_empty() || source_bytes.len() > MAX_EVIDENCE_SOURCE_BYTES {
        return Err("Evidence source is empty or exceeds 16 MiB".to_owned());
    }
    let actual_identity = ContentIdentity::from_reader(Cursor::new(source_bytes))
        .map_err(|error| format!("Evidence source cannot be hashed: {error}"))?;
    if &actual_identity != expected_identity {
        return Err("Evidence source identity changed after discovery".to_owned());
    }
    let extension = source_extension(source_basename)?;
    let (format, sections) = match extension.as_str() {
        "docx" => (EvidenceFormat::Docx, extract_docx(source_bytes)?),
        "pdf" => (EvidenceFormat::Pdf, extract_pdf(source_bytes)?),
        extension if is_text_extension(extension) => {
            (EvidenceFormat::Text, extract_utf8_text(source_bytes)?)
        }
        _ => {
            return Err(
                "File format is not supported for semantic extraction; provide OCR or reviewed evidence"
                    .to_owned(),
            )
        }
    };
    let (excerpts, truncated) = bounded_excerpts(sections)?;
    Ok(ExtractedFileEvidence {
        source_identity: actual_identity,
        format,
        excerpts,
        truncated,
    })
}

fn source_extension(source_basename: &str) -> Result<String, String> {
    if source_basename.is_empty()
        || source_basename.len() > 255
        || source_basename.contains(['/', '\\'])
        || source_basename.chars().any(char::is_control)
    {
        return Err("Evidence source filename is invalid".to_owned());
    }
    source_basename
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .filter(|extension| !extension.is_empty())
        .ok_or_else(|| "Evidence source filename has no supported extension".to_owned())
}

fn is_text_extension(extension: &str) -> bool {
    matches!(
        extension,
        "txt"
            | "md"
            | "markdown"
            | "json"
            | "html"
            | "htm"
            | "csv"
            | "tsv"
            | "yaml"
            | "yml"
            | "xml"
            | "toml"
            | "ini"
            | "log"
            | "rs"
            | "js"
            | "jsx"
            | "ts"
            | "tsx"
            | "py"
            | "go"
            | "java"
            | "kt"
            | "c"
            | "cc"
            | "cpp"
            | "h"
            | "hpp"
    )
}

fn extract_utf8_text(source_bytes: &[u8]) -> Result<Vec<(String, String)>, String> {
    let text = std::str::from_utf8(source_bytes)
        .map_err(|_| "Text evidence source is not valid UTF-8".to_owned())?;
    Ok(vec![("text".to_owned(), text.to_owned())])
}

fn extract_docx(source_bytes: &[u8]) -> Result<Vec<(String, String)>, String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(source_bytes))
        .map_err(|_| "DOCX evidence source is not a valid ZIP package".to_owned())?;
    let mut document = archive
        .by_name("word/document.xml")
        .map_err(|_| "DOCX evidence source has no word/document.xml".to_owned())?;
    if document.size() == 0 || document.size() > MAX_DOCX_XML_BYTES as u64 {
        return Err("DOCX document XML is empty or exceeds 4 MiB".to_owned());
    }
    let mut xml = Vec::with_capacity(document.size() as usize);
    document
        .by_ref()
        .take(MAX_DOCX_XML_BYTES as u64 + 1)
        .read_to_end(&mut xml)
        .map_err(|_| "DOCX document XML cannot be read".to_owned())?;
    if xml.len() > MAX_DOCX_XML_BYTES {
        return Err("DOCX document XML exceeds 4 MiB".to_owned());
    }
    docx_paragraphs(&xml)
}

fn docx_paragraphs(xml: &[u8]) -> Result<Vec<(String, String)>, String> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);
    let mut paragraphs = Vec::new();
    let mut current = String::new();
    let mut paragraph_number = 0usize;
    loop {
        match reader.read_event() {
            Ok(Event::Text(text)) => {
                let unescaped = text
                    .unescape()
                    .map_err(|_| "DOCX text encoding or escaping is invalid".to_owned())?;
                current.push_str(&unescaped);
            }
            Ok(Event::Empty(element)) => match element.name().as_ref() {
                b"w:tab" => current.push('\t'),
                b"w:br" | b"w:cr" => current.push('\n'),
                _ => {}
            },
            Ok(Event::End(element)) if element.name().as_ref() == b"w:p" => {
                paragraph_number += 1;
                let paragraph = current.trim();
                if !paragraph.is_empty() {
                    paragraphs.push((
                        format!("paragraph:{paragraph_number}"),
                        paragraph.to_owned(),
                    ));
                }
                current.clear();
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(_) => return Err("DOCX document XML is invalid".to_owned()),
        }
    }
    if !current.trim().is_empty() {
        paragraph_number += 1;
        paragraphs.push((
            format!("paragraph:{paragraph_number}"),
            current.trim().to_owned(),
        ));
    }
    Ok(paragraphs)
}

fn extract_pdf(source_bytes: &[u8]) -> Result<Vec<(String, String)>, String> {
    let pages = pdf_extract::extract_text_from_mem_by_pages(source_bytes).map_err(|_| {
        "PDF text extraction failed; OCR or reviewed evidence may be required".to_owned()
    })?;
    Ok(pages
        .into_iter()
        .enumerate()
        .filter_map(|(index, text)| {
            let text = text.trim();
            (!text.is_empty()).then(|| (format!("page:{}", index + 1), text.to_owned()))
        })
        .collect())
}

fn bounded_excerpts(
    sections: Vec<(String, String)>,
) -> Result<(Vec<FileEvidenceExcerpt>, bool), String> {
    let mut excerpts = Vec::new();
    let mut truncated = false;
    for (location, text) in sections {
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        let mut remaining = normalized.trim();
        let mut part = 1usize;
        while !remaining.is_empty() {
            if excerpts.len() == MAX_EXCERPTS {
                truncated = true;
                break;
            }
            let split_at = remaining
                .char_indices()
                .nth(MAX_EXCERPT_CHARS)
                .map(|(index, _)| index)
                .unwrap_or(remaining.len());
            let (chunk, rest) = remaining.split_at(split_at);
            let chunk = chunk.trim();
            if !chunk.is_empty() {
                let excerpt_location = if rest.is_empty() && part == 1 {
                    location.clone()
                } else {
                    format!("{location}:part:{part}")
                };
                let binding = format!("{excerpt_location}\n{chunk}");
                let identity = ContentIdentity::from_reader(Cursor::new(binding.as_bytes()))
                    .map_err(|error| format!("Evidence excerpt cannot be hashed: {error}"))?;
                excerpts.push(FileEvidenceExcerpt {
                    evidence_id: format!("evidence-{}", &identity.digest[..16]),
                    location: excerpt_location,
                    text: chunk.to_owned(),
                });
            }
            remaining = rest.trim();
            part += 1;
        }
        if truncated {
            break;
        }
    }
    if excerpts.is_empty() {
        return Err(
            "No semantic text could be extracted; OCR or reviewed evidence is required".to_owned(),
        );
    }
    Ok((excerpts, truncated))
}

#[cfg(test)]
mod tests {
    use super::{extract_file_evidence, EvidenceFormat, MAX_EXCERPTS};
    use crate::identity::ContentIdentity;
    use std::io::{Cursor, Write};

    fn identity(bytes: &[u8]) -> ContentIdentity {
        ContentIdentity::from_reader(Cursor::new(bytes)).expect("hash evidence fixture")
    }

    fn docx_bytes(xml: &str) -> Vec<u8> {
        let cursor = Cursor::new(Vec::new());
        let mut archive = zip::ZipWriter::new(cursor);
        archive
            .start_file(
                "word/document.xml",
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated),
            )
            .expect("start DOCX document entry");
        archive
            .write_all(xml.as_bytes())
            .expect("write DOCX document XML");
        archive.finish().expect("finish DOCX fixture").into_inner()
    }

    fn text_pdf_bytes() -> Vec<u8> {
        let content = b"BT /F1 12 Tf 72 720 Td (Project Atlas PDF evidence) Tj ET";
        let objects = [
            b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
            b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
            b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>".to_vec(),
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec(),
            [
                format!("<< /Length {} >>\nstream\n", content.len()).as_bytes(),
                content,
                b"\nendstream",
            ]
            .concat(),
        ];
        let mut pdf = b"%PDF-1.4\n".to_vec();
        let mut offsets = Vec::with_capacity(objects.len());
        for (index, object) in objects.iter().enumerate() {
            offsets.push(pdf.len());
            writeln!(&mut pdf, "{} 0 obj", index + 1).expect("write PDF object header");
            pdf.extend_from_slice(object);
            pdf.extend_from_slice(b"\nendobj\n");
        }
        let xref_offset = pdf.len();
        writeln!(&mut pdf, "xref\n0 {}", objects.len() + 1).expect("write PDF xref header");
        pdf.extend_from_slice(b"0000000000 65535 f \n");
        for offset in offsets {
            writeln!(&mut pdf, "{offset:010} 00000 n ").expect("write PDF xref entry");
        }
        writeln!(
            &mut pdf,
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF",
            objects.len() + 1,
        )
        .expect("write PDF trailer");
        pdf
    }

    #[test]
    fn extracts_bounded_utf8_text_with_stable_evidence_identity() {
        let bytes = b"Project Atlas reset reliability report\nVersion V2.1\n";
        let first = extract_file_evidence("report.md", bytes, &identity(bytes))
            .expect("extract text evidence");
        let repeated = extract_file_evidence("report.md", bytes, &identity(bytes))
            .expect("repeat text evidence");

        assert_eq!(first, repeated);
        assert_eq!(first.format, EvidenceFormat::Text);
        assert_eq!(first.excerpts.len(), 1);
        assert_eq!(first.excerpts[0].location, "text");
        assert!(first.excerpts[0].evidence_id.starts_with("evidence-"));
        assert!(!first.truncated);
    }

    #[test]
    fn extracts_docx_paragraphs_without_running_embedded_content() {
        let bytes = docx_bytes(
            r#"<w:document xmlns:w="urn:w"><w:body>
            <w:p><w:r><w:t>Project Atlas &amp; MCU</w:t></w:r></w:p>
            <w:p><w:r><w:t>Reset</w:t></w:r><w:tab/><w:r><w:t>V2.1</w:t></w:r></w:p>
            </w:body></w:document>"#,
        );
        let extracted = extract_file_evidence("notice.docx", &bytes, &identity(&bytes))
            .expect("extract DOCX evidence");

        assert_eq!(extracted.format, EvidenceFormat::Docx);
        assert_eq!(extracted.excerpts[0].location, "paragraph:1");
        assert_eq!(extracted.excerpts[0].text, "Project Atlas & MCU");
        assert_eq!(extracted.excerpts[1].text, "Reset\tV2.1");
    }

    #[test]
    fn extracts_page_scoped_text_from_a_text_pdf() {
        let bytes = text_pdf_bytes();
        let extracted = extract_file_evidence("report.pdf", &bytes, &identity(&bytes))
            .expect("extract PDF evidence");

        assert_eq!(extracted.format, EvidenceFormat::Pdf);
        assert_eq!(extracted.excerpts[0].location, "page:1");
        assert!(extracted.excerpts[0]
            .text
            .contains("Project Atlas PDF evidence"));
    }

    #[test]
    fn rejects_changed_binary_and_scanned_or_unsupported_sources_without_guessing() {
        let expected = identity(b"before");
        assert!(extract_file_evidence("report.txt", b"after", &expected).is_err());
        let empty_pdf = b"%PDF-1.4\n%%EOF\n";
        assert!(extract_file_evidence("scan.pdf", empty_pdf, &identity(empty_pdf)).is_err());
        let binary = b"\x89PNG\r\n\x1a\n";
        assert!(extract_file_evidence("image.png", binary, &identity(binary)).is_err());
    }

    #[test]
    fn truncates_large_text_at_the_declared_excerpt_budget() {
        let bytes = "a".repeat((MAX_EXCERPTS + 2) * 4_096).into_bytes();
        let extracted = extract_file_evidence("large.txt", &bytes, &identity(&bytes))
            .expect("extract bounded evidence");

        assert_eq!(extracted.excerpts.len(), MAX_EXCERPTS);
        assert!(extracted.truncated);
        assert!(extracted
            .excerpts
            .iter()
            .all(|excerpt| excerpt.text.chars().count() <= 4_096));
    }
}
