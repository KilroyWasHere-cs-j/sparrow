use xmlrpc::Request;

pub struct Fldigi {
    pub version: String,
    pub fldigi_url: String,
}

impl Fldigi {
    pub fn new() -> Self {
        Self {
            version: String::new(),
            fldigi_url: "http://127.0.0.1:7362/RPC2".to_string(),
        }
    }

    pub fn get_version(&self) {
        let request = Request::new("fldigi.version").call_url(self.fldigi_url.as_str()).unwrap();
        println!("{:?}", request);
    }
}

