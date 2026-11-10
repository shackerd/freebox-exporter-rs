use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
#[allow(unused)]
#[derive(Default)]
pub struct Permissions {
    pub connection: Option<bool>,
    pub settings: Option<bool>,
    pub contacts: Option<bool>,
    pub calls: Option<bool>,
    pub explorer: Option<bool>,
    pub downloader: Option<bool>,
    pub parental: Option<bool>,
    pub pvr: Option<bool>,
}
