use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
#[allow(unused)]
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

impl Default for Permissions {
    fn default() -> Self {
        Self {
            connection: Default::default(),
            settings: Default::default(),
            contacts: Default::default(),
            calls: Default::default(),
            explorer: Default::default(),
            downloader: Default::default(),
            parental: Default::default(),
            pvr: Default::default(),
        }
    }
}
