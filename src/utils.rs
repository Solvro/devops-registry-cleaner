use reqwest::Url;

pub trait UrlExt {
    fn extend_path(&mut self, segments: impl IntoIterator<Item = impl AsRef<str>>);
}

impl UrlExt for Url {
    fn extend_path(&mut self, segments: impl IntoIterator<Item = impl AsRef<str>>) {
        let mut editor = self
            .path_segments_mut()
            .expect("URL should be a valid base");
        editor.extend(segments);
    }
}
