//! Imagens para o markdown das respostas e as miniaturas de anexos: arquivo local lido na hora,
//! URL baixada numa thread (curl) e decodificada pelo `ImageCache` do rune.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use rune::{Image, ImageCache};

static CACHE: OnceLock<ImageCache> = OnceLock::new();
static WAKER: OnceLock<rune::Waker> = OnceLock::new();
/// Downloads: `None` enquanto baixa ou se falhou, `Some(bytes)` quando chegou.
static FETCHED: Mutex<Option<HashMap<String, Option<Vec<u8>>>>> = Mutex::new(None);

thread_local! {
    static LOADED: std::cell::RefCell<HashMap<String, Image>> = Default::default();
}

pub fn init(waker: rune::Waker) {
    let _ = CACHE.set(ImageCache::new(waker.clone()));
    let _ = WAKER.set(waker);
}

/// Imagem de um arquivo local, decodificando em segundo plano.
pub fn file(path: &std::path::Path) -> Option<Image> {
    let key = path.display().to_string();
    if let Some(i) = LOADED.with(|m| m.borrow().get(&key).cloned()) {
        return Some(i);
    }
    let img = CACHE.get()?.load(std::fs::read(path).ok()?);
    LOADED.with(|m| m.borrow_mut().insert(key, img.clone()));
    Some(img)
}

/// Fonte de `![alt](src)`: caminho absoluto, `file://` ou http(s).
pub fn source(src: &str) -> Option<Image> {
    let src = src.trim();
    if let Some(p) = src.strip_prefix("file://").or(src.starts_with('/').then_some(src)) {
        return file(std::path::Path::new(p));
    }
    if !(src.starts_with("https://") || src.starts_with("http://")) {
        return None;
    }
    if let Some(i) = LOADED.with(|m| m.borrow().get(src).cloned()) {
        return Some(i);
    }
    let mut g = FETCHED.lock().ok()?;
    let map = g.get_or_insert_with(HashMap::new);
    match map.get(src) {
        Some(Some(bytes)) => {
            let img = CACHE.get()?.load(bytes.clone());
            map.remove(src);
            LOADED.with(|m| m.borrow_mut().insert(src.to_string(), img.clone()));
            Some(img)
        }
        Some(None) => None,
        None => {
            map.insert(src.to_string(), None);
            let url = src.to_string();
            std::thread::spawn(move || {
                let out = std::process::Command::new("curl").args(["-sfL", "--max-time", "20", "--max-filesize", "20000000", &url]).output();
                if let Ok(o) = out
                    && o.status.success()
                    && !o.stdout.is_empty()
                    && let Ok(mut g) = FETCHED.lock()
                {
                    g.get_or_insert_with(HashMap::new).insert(url, Some(o.stdout));
                    if let Some(w) = WAKER.get() {
                        w();
                    }
                }
            });
            None
        }
    }
}
