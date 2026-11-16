//! Small HTTP/1.1 request parser. One bounded request per connection, no chunked uploads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Method {
    Get,
    Post,
    Patch,
}
#[derive(Debug, PartialEq)]
pub enum Error {
    Malformed,
    TooLarge,
    UnsupportedMedia,
    Forbidden,
}
pub struct Request<'a> {
    pub method: Method,
    pub path: &'a str,
    pub body: &'a [u8],
    pub host: &'a str,
}
pub const MAX_HEADER: usize = 1024;
pub const MAX_BODY: usize = 2048;

pub fn parse(data: &[u8]) -> Result<Option<Request<'_>>, Error> {
    let Some(end) = data.windows(4).position(|w| w == b"\r\n\r\n") else {
        return if data.len() >= MAX_HEADER {
            Err(Error::TooLarge)
        } else {
            Ok(None)
        };
    };
    if end + 4 > MAX_HEADER {
        return Err(Error::TooLarge);
    }
    let header = core::str::from_utf8(&data[..end]).map_err(|_| Error::Malformed)?;
    let mut lines = header.split("\r\n");
    let mut first = lines.next().ok_or(Error::Malformed)?.split(' ');
    let method = match first.next() {
        Some("GET") => Method::Get,
        Some("POST") => Method::Post,
        Some("PATCH") => Method::Patch,
        _ => return Err(Error::Malformed),
    };
    let path = first
        .next()
        .filter(|p| p.starts_with('/') && p.bytes().all(|b| b.is_ascii_graphic()))
        .ok_or(Error::Malformed)?;
    if first.next() != Some("HTTP/1.1") || first.next().is_some() {
        return Err(Error::Malformed);
    }
    let mut length = None;
    let mut host = None;
    let mut origin = None;
    let mut json = false;
    for line in lines {
        let (key, value) = line.split_once(':').ok_or(Error::Malformed)?;
        if key.is_empty() || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
            return Err(Error::Malformed);
        }
        let value = value.trim();
        if value.bytes().any(|b| b < 0x20 || b == 0x7f) {
            return Err(Error::Malformed);
        }
        if key.eq_ignore_ascii_case("transfer-encoding") {
            return Err(Error::Malformed);
        }
        if key.eq_ignore_ascii_case("content-length") {
            if length.is_some() || value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                return Err(Error::Malformed);
            }
            let n = value.parse::<usize>().map_err(|_| Error::TooLarge)?;
            if n > MAX_BODY {
                return Err(Error::TooLarge);
            }
            length = Some(n);
        } else if key.eq_ignore_ascii_case("host") {
            if host.replace(value).is_some() || value.is_empty() || value.len() > 128 {
                return Err(Error::Malformed);
            }
        } else if key.eq_ignore_ascii_case("origin") {
            if origin.replace(value).is_some() {
                return Err(Error::Malformed);
            }
        } else if key.eq_ignore_ascii_case("content-type") {
            json = value
                .split(';')
                .next()
                .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"));
        }
    }
    let host = host.ok_or(Error::Malformed)?;
    if method != Method::Get {
        if !json {
            return Err(Error::UnsupportedMedia);
        }
        if let Some(origin) = origin {
            if origin.strip_prefix("http://") != Some(host)
                && origin.strip_prefix("https://") != Some(host)
            {
                return Err(Error::Forbidden);
            }
        }
    }
    let length = length.unwrap_or(0);
    if data.len() < end + 4 + length {
        return Ok(None);
    }
    if data.len() != end + 4 + length {
        return Err(Error::Malformed);
    }
    Ok(Some(Request {
        method,
        path: path.split('?').next().unwrap_or(path),
        body: &data[end + 4..],
        host,
    }))
}

pub fn trusted_host(host: &str, ip: &str, hostname: &str) -> bool {
    let name = host.strip_suffix(":80").unwrap_or(host);
    name == ip
        || name.eq_ignore_ascii_case(hostname)
        || name
            .strip_suffix(".local")
            .is_some_and(|v| v.eq_ignore_ascii_case(hostname))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn handles_fragmented_request_without_early_dispatch() {
        let request=b"PATCH /api/system HTTP/1.1\r\nHost: bitaxe\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}";
        for n in 0..request.len() {
            assert!(parse(&request[..n]).unwrap().is_none());
        }
        assert_eq!(parse(request).unwrap().unwrap().body, b"{}");
    }
    #[test]
    fn rejects_ambiguous_or_unbounded_requests() {
        for req in [
            "POST / HTTP/1.1\r\nHost: bitaxe\r\nTransfer-Encoding: chunked\r\n\r\n",
            "GET / HTTP/1.1\r\nHost: x\r\nContent-Length: 0\r\nContent-Length: 0\r\n\r\n",
            "GET / HTTP/1.1\r\nHost: x\r\nContent-Length: 4096\r\n\r\n",
            "GET / HTTP/1.1\r\nHost: x\r\n\r\njunk",
        ] {
            assert!(parse(req.as_bytes()).is_err());
        }
    }
    #[test]
    fn rejects_cross_origin_mutations_and_simple_forms() {
        assert!(matches!(parse(b"POST /api/system/restart HTTP/1.1\r\nHost: bitaxe\r\nOrigin: https://evil.example\r\nContent-Type: application/json\r\n\r\n"),Err(Error::Forbidden)));
        assert!(matches!(parse(b"POST /api/system/restart HTTP/1.1\r\nHost: bitaxe\r\nContent-Type: text/plain\r\n\r\n"),Err(Error::UnsupportedMedia)));
        assert!(!trusted_host("evil.example", "192.168.1.20", "bitaxe"));
        assert!(trusted_host("bitaxe.local:80", "192.168.1.20", "bitaxe"));
    }
}
