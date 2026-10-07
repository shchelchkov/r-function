use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;

pub fn serve_once(body: &'static str) -> (String, mpsc::Receiver<(String, String)>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let (mut sock, _) = listener.accept().unwrap();
        let mut buf = Vec::new();
        let mut chunk = [0u8; 1024];
        let mut read_more = |buf: &mut Vec<u8>| {
            let n = sock.read(&mut chunk).unwrap();
            assert_ne!(n, 0, "client closed the connection");
            buf.extend_from_slice(&chunk[..n]);
        };
        let head_len = loop {
            if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                break i + 4;
            }
            read_more(&mut buf);
        };
        let head = String::from_utf8(buf[..head_len].to_vec()).unwrap();
        let body_len: usize =
            find_header(&head, "content-length").map_or(0, |v| v.parse().unwrap());
        while buf.len() < head_len + body_len {
            read_more(&mut buf);
        }
        let req_body = String::from_utf8(buf[head_len..head_len + body_len].to_vec()).unwrap();

        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        sock.write_all(resp.as_bytes()).unwrap();
        tx.send((head, req_body)).unwrap();
    });
    (base, rx)
}

pub fn header(head: &str, name: &str) -> String {
    find_header(head, name).unwrap_or_else(|| panic!("no header {name} in {head}"))
}

fn find_header(head: &str, name: &str) -> Option<String> {
    head.lines()
        .find_map(|l| {
            l.split_once(": ")
                .filter(|(k, _)| k.eq_ignore_ascii_case(name))
        })
        .map(|(_, v)| v.to_string())
}
