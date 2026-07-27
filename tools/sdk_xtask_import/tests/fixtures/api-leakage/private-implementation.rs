use reqwest::Client;

pub struct Adapter {
    client: Client,
}

impl Adapter {
    fn client(&self) -> &Client {
        &self.client
    }
}
