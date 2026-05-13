use reqwest::{
    header::{self, HeaderMap, HeaderValue},
    Client, Url,
};
use scraper::{Html, Selector};
use std::net::IpAddr;
use std::time::Duration;
use thiserror::Error;
use tracing::debug;

use super::upload::{save_image, UploadError};
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum ScrapeError {
    #[error("URL invalide ou schéma non supporté")]
    InvalidUrl,
    #[error("URL bloquée : adresse locale ou privée non autorisée")]
    BlockedUrl,
    #[error("Erreur réseau : {0}")]
    Http(#[from] reqwest::Error),
    #[error("Impossible de récupérer automatiquement les informations, remplissez manuellement")]
    NoData,
    #[error("Erreur image : {0}")]
    Upload(#[from] UploadError),
}

#[derive(Debug)]
pub struct ScrapeResult {
    pub name: Option<String>,
    pub description: Option<String>,
    pub image_url: Option<String>,
}

const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:128.0) Gecko/20100101 Firefox/128.0";

fn browser_headers() -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(header::ACCEPT, HeaderValue::from_static(
        "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8",
    ));
    h.insert(header::ACCEPT_LANGUAGE, HeaderValue::from_static("fr-FR,fr;q=0.9,en-US;q=0.8,en;q=0.7"));
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    h.insert("DNT", HeaderValue::from_static("1"));
    h
}

/// Scrape les métadonnées Open Graph d'une URL produit.
pub async fn scrape_product(url: &str) -> Result<ScrapeResult, ScrapeError> {
    let parsed = Url::parse(url).map_err(|_| ScrapeError::InvalidUrl)?;

    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(ScrapeError::InvalidUrl);
    }

    let host = parsed.host_str().ok_or(ScrapeError::InvalidUrl)?;
    check_ssrf(host).await?;

    let client = Client::builder()
        .user_agent(USER_AGENT)
        .default_headers(browser_headers())
        .cookie_store(true)
        .timeout(Duration::from_secs(10))
        .build()?;

    let html = client.get(url).send().await?.text().await?;
    let doc = Html::parse_document(&html);

    let name = og(&doc, "og:title").or_else(|| page_title(&doc));
    let description = og(&doc, "og:description").or_else(|| meta_desc(&doc));
    let image_url = og(&doc, "og:image");

    debug!(
        "Scrape {} → name={:?} desc={:?} img={:?}",
        url,
        name.as_deref(),
        description.as_deref(),
        image_url.as_deref()
    );

    if name.is_none() && description.is_none() && image_url.is_none() {
        return Err(ScrapeError::NoData);
    }

    Ok(ScrapeResult { name, description, image_url })
}

/// Télécharge l'image OG et la stocke dans UPLOAD_DIR.
/// Retourne le chemin relatif. Erreurs non-fatales : l'appelant peut ignorer.
pub async fn download_og_image(
    image_url: &str,
    upload_dir: &str,
    user_id: Uuid,
    equipment_id: Uuid,
) -> Result<String, ScrapeError> {
    let parsed = Url::parse(image_url).map_err(|_| ScrapeError::InvalidUrl)?;
    let host = parsed.host_str().ok_or(ScrapeError::InvalidUrl)?;
    check_ssrf(host).await?;

    let client = Client::builder().timeout(Duration::from_secs(15)).build()?;
    let resp = client.get(image_url).send().await?;

    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/jpeg")
        // Certains CDN retournent "image/jpeg; charset=..." — on prend juste le type
        .split(';')
        .next()
        .unwrap_or("image/jpeg")
        .trim()
        .to_string();

    let data = resp.bytes().await?.to_vec();
    let path = save_image(data, &content_type, upload_dir, user_id, equipment_id).await?;
    Ok(path)
}

// -- Anti-SSRF ---------------------------------------------------------------

async fn check_ssrf(host: &str) -> Result<(), ScrapeError> {
    // Refuser les IP littérales privées
    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_private(&ip) {
            return Err(ScrapeError::BlockedUrl);
        }
    }

    // Refuser les noms d'hôtes manifestement locaux
    let h = host.to_lowercase();
    if h == "localhost"
        || h.ends_with(".local")
        || h.ends_with(".internal")
        || h.ends_with(".lan")
    {
        return Err(ScrapeError::BlockedUrl);
    }

    // Résolution DNS + vérification des IPs résolues
    if let Ok(addrs) = tokio::net::lookup_host(format!("{}:80", host)).await {
        for addr in addrs {
            if is_private(&addr.ip()) {
                return Err(ScrapeError::BlockedUrl);
            }
        }
    }

    Ok(())
}

fn is_private(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            matches!(
                (a, b),
                (10, _)           // 10.0.0.0/8
                | (172, 16..=31)  // 172.16.0.0/12
                | (192, 168)      // 192.168.0.0/16
                | (127, _)        // 127.0.0.0/8 loopback
                | (169, 254)      // 169.254.0.0/16 link-local
                | (0, _)          // 0.0.0.0/8
                | (100, 64..=127) // 100.64.0.0/10 CGNAT
                | (198, 18..=19)  // 198.18.0.0/15 benchmarking
            )
        }
        IpAddr::V6(v6) => v6.is_loopback() || v6.is_unspecified(),
    }
}

// -- Helpers HTML (pub(crate) pour les tests) ---------------------------------

pub(crate) fn og(doc: &Html, property: &str) -> Option<String> {
    let sel = Selector::parse(&format!("meta[property='{property}']")).ok()?;
    doc.select(&sel)
        .next()?
        .attr("content")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub(crate) fn page_title(doc: &Html) -> Option<String> {
    let sel = Selector::parse("title").ok()?;
    let text = doc.select(&sel).next()?.text().collect::<String>();
    let t = text.trim().to_string();
    if t.is_empty() { None } else { Some(t) }
}

pub(crate) fn meta_desc(doc: &Html) -> Option<String> {
    let sel = Selector::parse("meta[name='description']").ok()?;
    doc.select(&sel)
        .next()?
        .attr("content")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(html: &str) -> Html {
        Html::parse_document(html)
    }

    #[test]
    fn lit_balises_og() {
        let doc = parse(r#"<html><head>
            <meta property="og:title" content="Super Produit" />
            <meta property="og:description" content="Une belle description." />
            <meta property="og:image" content="https://example.com/img.jpg" />
        </head></html>"#);
        assert_eq!(og(&doc, "og:title"),       Some("Super Produit".into()));
        assert_eq!(og(&doc, "og:description"), Some("Une belle description.".into()));
        assert_eq!(og(&doc, "og:image"),       Some("https://example.com/img.jpg".into()));
    }

    #[test]
    fn retourne_none_si_balise_absente() {
        let doc = parse("<html><head></head></html>");
        assert_eq!(og(&doc, "og:title"), None);
        assert_eq!(page_title(&doc),     None);
        assert_eq!(meta_desc(&doc),      None);
    }

    #[test]
    fn fallback_sur_title_et_meta_description() {
        let doc = parse(r#"<html><head>
            <title>Titre de la page</title>
            <meta name="description" content="Description classique." />
        </head></html>"#);
        assert_eq!(page_title(&doc), Some("Titre de la page".into()));
        assert_eq!(meta_desc(&doc),  Some("Description classique.".into()));
        assert_eq!(og(&doc, "og:title"), None);
    }

    #[test]
    fn ignore_balise_og_vide() {
        let doc = parse(r#"<html><head>
            <meta property="og:title" content="  " />
        </head></html>"#);
        assert_eq!(og(&doc, "og:title"), None);
    }
}
