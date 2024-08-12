use async_trait::async_trait;
use log::{debug, error, warn};
use prometheus_exporter::prometheus::{register_int_gauge, register_int_gauge_vec, IntGauge, IntGaugeVec};
use serde::Deserialize;
use crate::core::common::{AuthenticatedHttpClientFactory, FreeboxResponse, FreeboxResponseError};
use super::MetricMap;

#[derive(Deserialize, Clone, Debug)]
pub struct LanBrowserInterface {
    name: Option<String>,
    host_count: Option<i32>
}

#[derive(Deserialize, Clone, Debug)]
pub struct LanHost {
    id: Option<String>,
    primary_name: Option<String>,
    host_type: Option<String>,
    primary_name_manual: Option<bool>,
    l2ident: Option<LanHostL2Ident>,
    vendor_name: Option<String>,
    persistent: Option<bool>,
    reachable: Option<bool>,
    last_time_reachable: Option<i64>,
    active: Option<bool>,
    last_activity: Option<i64>,
    names: Option<Vec<LanHostName>>,
    l3connectivities: Option<Vec<LanHostL3Connectivity>>
}

#[derive(Deserialize, Clone, Debug)]
pub struct LanHostName {
    pub name: Option<String>,
    pub source: Option<String>
}

#[derive(Deserialize, Clone, Debug)]
pub struct LanHostL2Ident {
    pub id: Option<String>,
    #[serde(alias="type")]
    pub _type: Option<String>
}

#[derive(Deserialize, Clone, Debug)]
pub struct LanHostL3Connectivity {
    pub addr: Option<String>,
    pub af: Option<String>,
    pub active: Option<bool>,
    pub reachable: Option<bool>,
    pub last_activity: Option<i64>, // timestamp
    pub last_time_reachable: Option<i64> // timestamp
}

pub struct InterfaceMetrics {
    pub prefix: String,
    pub host_count_metric: IntGauge,
    pub devices_metric: Vec<LanHostMetrics>
}

impl InterfaceMetrics {
    pub fn new(prefix: String, iface: LanBrowserInterface) -> Self {

        let iface_name = iface.name.unwrap();
        let prfx = format!("{prefix}_{iface_name}");

        Self {
            prefix: prfx.to_owned(),
            host_count_metric: register_int_gauge!(format!("{prfx}_host_count"), format!("{prfx}_host_count")).expect(&format!("cannot create {prfx}_host_count gauge")),
            devices_metric: vec![]
        }
    }

    pub fn register_hosts(&mut self, hosts: Vec<LanHost>) { // lanhost

        for host in hosts {
            let metric = LanHostMetrics::new(host, self.prefix.to_owned());
            self.devices_metric.push(metric);
        }
    }

    pub async fn set(&self, iface: LanBrowserInterface, hosts: Vec<LanHost>) -> Result<(), Box<dyn std::error::Error>> {

        self.host_count_metric.set(iface.to_owned().host_count.unwrap_or(0).into());

        for host in hosts {

            let id = host.to_owned().id.unwrap().replace(":", "_").replace("-", "_");
            let metric_id = format!("{}_{id}", self.prefix);
            let device_metric = self.devices_metric.iter().find(|d| d.managed_id == metric_id);

            if device_metric.is_some() {
                match device_metric.unwrap().set(host).await { Err(_) => warn!("cannot set gauges for {metric_id}"), _ => {} }
            }
            else {
                warn!("unable to find metric gauges {metric_id}");
            }
        }

        Ok(())
    }

    // pub fn deallocate(&self) -> Result<(), ()> {

    //     // match unregister(Box::new(self.name_metric.to_owned())) { Err(_) => { return Err(()) }, _ => {} };
    //     match unregister(Box::new(self.host_count_metric.to_owned())) { Err(_) => { return Err(()) }, _ => {} };

    //     for host_metric in &self.devices_metric {
    //         match host_metric.deallocate() { Err(_) => { return Err(())}, _ => { } };
    //     }

    //     Ok(())
    // }
}

pub struct LanHostMetrics {

    pub managed_id: String,
    pub primary_name_metric: IntGaugeVec,
    pub host_type_metric: IntGaugeVec,
    pub primary_name_manual_metric: IntGauge,
    pub l2ident_id_metric: IntGaugeVec,
    pub l2ident_type_metric: IntGaugeVec,
    pub vendor_name_metric: IntGaugeVec,
    pub persistent_metric: IntGauge,
    pub reachable_metric: IntGauge,
    pub last_time_reachable_metric: IntGauge,
    pub active_metric: IntGauge,
    pub last_activity_metric: IntGauge,
    pub names_metrics: Vec<LanHostNameMetric>,
    pub l3connectivities_metrics: Vec<LanHostL3ConnectivityMetric>
}

impl LanHostMetrics {

    pub fn new(host: LanHost, prefix: String) -> Self {

        let id = host.id.unwrap().replace(":", "_").replace("-", "_");
        let prfx = format!("{prefix}_{id}");

        let l3connectivities = host.l3connectivities.unwrap();
        let mut l3s = vec![];

        for i in 0..l3connectivities.len() {
            l3s.push(LanHostL3ConnectivityMetric::new(format!("{prfx}_l3connectivities_{i}")));
        }

        let host_names = host.names.unwrap();
        let mut names = vec![];

        for i in 0..host_names.len() {
            names.push(LanHostNameMetric::new(format!("{prfx}_names_{i}")));
        }

        Self {
            managed_id: prfx.clone(),
            primary_name_metric: register_int_gauge_vec!(format!("{prfx}_primary_name"), "host primary name", &["primary_name"]).expect(&format!("cannot create {prfx}_primary_name gauge")),
            host_type_metric: register_int_gauge_vec!(format!("{prfx}_host_type"), "host type (from Freebox guess)", &["host_type"]).expect(&format!("cannot create {prfx}_host_type gauge")),
            primary_name_manual_metric: register_int_gauge!(format!("{prfx}_primary_name_manual"), "if 1 the primary name has been set manually").expect(&format!("cannot create {prfx}_primary_name_manual gauge")),
            l2ident_id_metric: register_int_gauge_vec!(format!("{prfx}_l2ident_id"), "layer 2 id", &["id"]).expect(&format!("cannot create {prfx}_l2ident_id gauge")),
            l2ident_type_metric: register_int_gauge_vec!(format!("{prfx}_l2ident_type"), "type of layer 2 address", &["type"]).expect(&format!("cannot create {prfx}_l2ident_type gauge")),
            vendor_name_metric: register_int_gauge_vec!(format!("{prfx}_vendor_name"), "host vendor name (from the mac address)", &["vendor_name"]).expect(&format!("cannot create {prfx}_vendor_name gauge")),
            persistent_metric: register_int_gauge!(format!("{prfx}_persistent"), "if 1 the host is always shown even if it has not been active since the Freebox startup").expect(&format!("cannot create {prfx}_persistent gauge")),
            reachable_metric: register_int_gauge!(format!("{prfx}_reachable"), "if 1 the host can receive traffic from the Freebox").expect(&format!("cannot create {prfx}_reachable gauge")),
            last_time_reachable_metric: register_int_gauge!(format!("{prfx}_last_time_reachable"), "last time the host was reached").expect(&format!("cannot create {prfx}_last_time_reachable gauge")),
            active_metric: register_int_gauge!(format!("{prfx}_active"), "if 1 the host sends traffic to the Freebox").expect(&format!("cannot create {prfx}_active gauge")),
            last_activity_metric: register_int_gauge!(format!("{prfx}_last_activity"), "last time the host sent traffic").expect(&format!("cannot create {prfx}_last_activity gauge")),
            l3connectivities_metrics: l3s,
            names_metrics: names
        }
    }

    pub async fn set(&self, host: LanHost) -> Result<(), Box<dyn std::error::Error>> {

        self.primary_name_metric.with_label_values(&[&host.to_owned().primary_name.unwrap_or_default()]).set(host.to_owned().primary_name.is_some().into());
        self.host_type_metric.with_label_values(&[&host.to_owned().host_type.unwrap_or_default()]).set(host.to_owned().host_type.is_some().into());
        self.primary_name_manual_metric.set(host.to_owned().primary_name_manual.unwrap_or(false).into());

        let l2indent = host.to_owned().l2ident;
        match l2indent {
            None => {
                self.l2ident_id_metric.with_label_values(&[&"id"]).set(0);
                self.l2ident_type_metric.with_label_values(&[&"type"]).set(0);
            },
            Some(l2) => {
                self.l2ident_id_metric.with_label_values(&[&l2.to_owned().id.unwrap_or_default()]).set(l2.to_owned().id.is_some().into());
                self.l2ident_type_metric.with_label_values(&[&l2.to_owned()._type.unwrap_or_default()]).set(l2.to_owned()._type.is_some().into());
            }
        }

        self.vendor_name_metric.with_label_values(&[&host.to_owned().vendor_name.unwrap_or_default()]).set(host.to_owned().vendor_name.is_some().into());
        self.persistent_metric.set(host.to_owned().persistent.unwrap_or(false).into());
        self.reachable_metric.set(host.to_owned().reachable.unwrap_or(false).into());
        self.last_time_reachable_metric.set(host.to_owned().last_time_reachable.unwrap_or(0));
        self.active_metric.set(host.to_owned().active.unwrap_or(false).into());
        self.last_activity_metric.set(host.to_owned().last_activity.unwrap_or(0));

        match host.l3connectivities {
            None => { },
            Some(l3s) => {

                let mut i = 0;

                for l3 in l3s {

                    let metric_id = format!("{}_l3connectivities_{i}", self.managed_id);
                    let metric = self.l3connectivities_metrics.iter().find(|l| l.managed_id == metric_id);
                    if metric.is_some() {
                        match metric.unwrap().set(l3) { Err(_) => warn!("cannot set gauges for {metric_id}"), _ => { } }
                    }
                    else {
                        warn!("unable to find metric gauges {metric_id}")
                    }
                    i += 1;
                }
            }
        }

        match host.names {
            None => { },
            Some(names) => {

                let mut i = 0;

                for name in names {
                    let metric_id = format!("{}_names_{i}", self.managed_id);
                    let metric = self.names_metrics.iter().find(|n| n.managed_id == metric_id);

                    if metric.is_some() {
                        match metric.unwrap().set(name) { Err(_) => warn!("cannot set gauges for {metric_id}"), _ => {} }
                    }
                    else {
                        warn!("unable to find metric gauges {metric_id}")
                    }
                    i += 1;
                }
            }
        }

        Ok(())
    }

    // pub fn deallocate(&self) -> Result<(), ()> {

    //     match unregister(Box::new(self.primary_name_metric.to_owned())) { Err(_) => { return Err(()) }, _ => {} };
    //     match unregister(Box::new(self.host_type_metric.to_owned())) { Err(_) => { return Err(()) }, _ => {} };
    //     match unregister(Box::new(self.primary_name_manual_metric.to_owned())) { Err(_) => { return Err(()) }, _ => {} };
    //     match unregister(Box::new(self.l2ident_id_metric.to_owned())) { Err(_) => { return Err(()) }, _ => {} };
    //     match unregister(Box::new(self.l2ident_type_metric.to_owned())) { Err(_) => { return Err(()) }, _ => {} };
    //     match unregister(Box::new(self.vendor_name_metric.to_owned())) { Err(_) => { return Err(()) }, _ => {} };
    //     match unregister(Box::new(self.persistent_metric.to_owned())) { Err(_) => { return Err(()) }, _ => {} };
    //     match unregister(Box::new(self.reachable_metric.to_owned())) { Err(_) => { return Err(()) }, _ => {} };
    //     match unregister(Box::new(self.last_time_reachable_metric.to_owned())) { Err(_) => { return Err(()) }, _ => {} };
    //     match unregister(Box::new(self.active_metric.to_owned())) { Err(_) => { return Err(()) }, _ => {} };
    //     match unregister(Box::new(self.last_activity_metric.to_owned())) { Err(_) => { return Err(()) }, _ => {} };

    //     for l3 in &self.l3connectivities_metrics {
    //         match l3.deallocate() { Err(_) => { return Err(())}, _ => { } };
    //     }

    //     for name in &self.names_metrics {
    //         match name.deallocate() { Err(_) => { return Err(())}, _ => { }};
    //     }

    //     Ok(())
    // }
}

pub struct LanHostL3ConnectivityMetric {

    pub managed_id: String,
    pub l3connectivities_addr_metric: IntGaugeVec,
    pub l3connectivities_af_metric: IntGaugeVec,
    pub l3connectivities_active_metric: IntGauge,
    pub l3connectivities_reachable_metric: IntGauge,
    pub l3connectivities_last_activity_metric: IntGauge,
    pub l3connectivities_last_time_reachable_metric: IntGauge
}

impl LanHostL3ConnectivityMetric {

    pub fn new(prefix: String) -> Self {

        Self {
            managed_id: prefix.to_owned(),
            l3connectivities_addr_metric: register_int_gauge_vec!(format!("{prefix}_addr"), "layer 3 address", &["addr"]).expect(&format!("cannot create {prefix}_addr gauge")),
            l3connectivities_af_metric: register_int_gauge_vec!(format!("{prefix}_af"), "ipv4 or ipv6", &["af"]).expect(&format!("cannot create {prefix}_af gauge")),
            l3connectivities_active_metric: register_int_gauge!(format!("{prefix}_active"), "is the connection active").expect(&format!("cannot create {prefix}_active gauge")),
            l3connectivities_reachable_metric: register_int_gauge!(format!("{prefix}_reachable"), "is the connection reachable").expect(&format!("cannot create {prefix}_reachable gauge")),
            l3connectivities_last_activity_metric: register_int_gauge!(format!("{prefix}_last_activity"), "last activity timestamp").expect(&format!("cannot create {prefix}_last_activity gauge")),
            l3connectivities_last_time_reachable_metric: register_int_gauge!(format!("{prefix}_last_time_reachable"), "last reachable timestamp").expect(&format!("cannot create {prefix}_last_time_reachable gauge")),
        }
    }

    pub fn set(&self, l3_connectivity: LanHostL3Connectivity) -> Result<(), ()> {

        self.l3connectivities_addr_metric.with_label_values(&[&l3_connectivity.to_owned().addr.unwrap_or_default()]).set(l3_connectivity.to_owned().addr.is_some().into());
        self.l3connectivities_af_metric.with_label_values(&[&l3_connectivity.to_owned().af.unwrap_or_default()]).set(l3_connectivity.to_owned().af.is_some().into());
        self.l3connectivities_active_metric.set(l3_connectivity.to_owned().active.unwrap_or(false).into());
        self.l3connectivities_reachable_metric.set(l3_connectivity.to_owned().reachable.unwrap_or(false).into());
        self.l3connectivities_last_activity_metric.set(l3_connectivity.to_owned().last_time_reachable.unwrap_or(0).into());
        self.l3connectivities_last_time_reachable_metric.set(l3_connectivity.to_owned().last_time_reachable.unwrap_or(0).into());
        Ok(())
    }

    // pub fn deallocate(&self) -> Result<(), ()> {

    //     match unregister(Box::new(self.l3connectivities_addr_metric.to_owned())) { Err(_) => { return Err(()) }, _ => {} };
    //     match unregister(Box::new(self.l3connectivities_af_metric.to_owned())) { Err(_) => { return Err(()) }, _ => {} };
    //     match unregister(Box::new(self.l3connectivities_active_metric.to_owned())) { Err(_) => { return Err(()) }, _ => {} };
    //     match unregister(Box::new(self.l3connectivities_reachable_metric.to_owned())) { Err(_) => { return Err(()) }, _ => {} };
    //     match unregister(Box::new(self.l3connectivities_last_activity_metric.to_owned())) { Err(_) => { return Err(()) }, _ => {} };
    //     match unregister(Box::new(self.l3connectivities_last_time_reachable_metric.to_owned())) { Err(_) => { return Err(()) }, _ => {} };

    //     Ok(())
    // }
}

pub struct LanHostNameMetric {
    pub managed_id: String,
    pub names_name_metric: IntGaugeVec,
    pub names_source_metric: IntGaugeVec
}

impl LanHostNameMetric {
    pub fn new(prefix: String) -> Self {

        Self {
            managed_id: prefix.to_owned(),
            names_name_metric: register_int_gauge_vec!(format!("{prefix}_name"), "host name", &["name"]).expect(&format!("cannot create {prefix}_name gauge")),
            names_source_metric: register_int_gauge_vec!(format!("{prefix}_source"), "source of the name", &["source"]).expect(&format!("cannot create {prefix}_source gauge")),
        }
    }

    pub fn set(&self, host_name: LanHostName) -> Result<(), ()> {

        self.names_name_metric.with_label_values(&[&host_name.to_owned().name.unwrap_or_default()]).set(host_name.to_owned().name.is_some().into());
        self.names_source_metric.with_label_values(&[&host_name.to_owned().source.unwrap_or_default()]).set(host_name.to_owned().source.is_some().into());
        Ok(())
    }

    // pub fn deallocate(&self) -> Result<(), ()> {

    //     match unregister(Box::new(self.names_name_metric.to_owned())) { Err(_) => { return Err(()) }, _ => {} };
    //     match unregister(Box::new(self.names_source_metric.to_owned())) { Err(_) => { return Err(()) }, _ => {} };

    //     Ok(())
    // }
}

// #[async_trait]
// pub trait DeallocatableMetric {
//     async fn deallocate(&mut self) -> Result<(), Box<dyn std::error::Error>>;
// }

pub struct LanBrowserMetricMap {
    factory: AuthenticatedHttpClientFactory,
    prefix: String,
    ifaces_metrics: Option<Vec<InterfaceMetrics>>
}

impl LanBrowserMetricMap {
    pub fn new(factory: AuthenticatedHttpClientFactory, prefix: String) -> Self {
        Self {
            factory,
            prefix: format!("{prefix}_lan_browser"),
            ifaces_metrics: None
        }
    }

    // async fn allocate_new_gauges() -> Result<(), Box<dyn std::error::Error>> {
    //     todo!()
    // }

    // async fn deallocate_orphan_gauges() -> Result<(), Box<dyn std::error::Error>> {
    //     todo!()
    // }

    async fn get_devices(&self, interface: &LanBrowserInterface) -> Result<Vec<LanHost>, Box<dyn std::error::Error>> {

        let iface = interface.name.as_ref().unwrap();

        debug!("fetching {} interface devices", iface);

        let body =
            self.factory.create_client().await.unwrap().get(format!("{}v4/lan/browser/{}", self.factory.api_url, iface))
            .send().await?
            .text().await?;

        let res = serde_json::from_str::<FreeboxResponse<Vec<LanHost>>>(&body);

        if res.is_err() || !res.as_ref().unwrap().success {
            return Err(Box::new(FreeboxResponseError::new(res.as_ref().unwrap().msg.clone())));
        }

        let devices= res.unwrap().result;

        Ok(devices)
    }

    async fn init_metrics(&self) -> Result<Vec<InterfaceMetrics>, Box<dyn std::error::Error>> {

        // let miface_metric.register_hosts()ut dev_by_iface = HashMap::new();

        let mut iface_metrics = vec![];

        let ifaces = self.get_ifaces().await?;

        for iface in ifaces {

            let mut iface_metric = InterfaceMetrics::new(self.prefix.to_owned(), iface.to_owned());

            if iface.host_count.unwrap_or(0) == 0 {
                iface_metrics.push(iface_metric);
                continue;
            }

            let devices = self.get_devices(&iface).await;

            match devices {
                Err(e) => error!("{e:#?}"),
                Ok(r) => {
                    iface_metric.register_hosts(r);
                }
            }

            iface_metrics.push(iface_metric);
        }

        Ok(iface_metrics)
    }

    async fn get_ifaces(&self) -> Result<Vec<LanBrowserInterface>, Box<dyn std::error::Error>> {
        debug!("fetching ifaces & devices");

        let body =
            self.factory.create_client().await.unwrap().get(format!("{}v4/lan/browser/interfaces", self.factory.api_url))
            .send().await?
            .text().await?;

        let res = serde_json::from_str::<FreeboxResponse<Vec<LanBrowserInterface>>>(&body);

        if res.is_err() || !res.as_ref().unwrap().success {
            return Err(Box::new(FreeboxResponseError::new(res.as_ref().unwrap().msg.clone())));
        }

        Ok(res.unwrap().result)
    }

    async fn set_all(&self) -> Result<(), Box<dyn std::error::Error>> {

        let ifaces = self.get_ifaces().await?;


        for iface in ifaces.iter().filter(|i| i.host_count.unwrap_or(0) > 0).map(|i| i.to_owned()) {

            let iface_id = format!("{}_{}", self.prefix, iface.to_owned().name.unwrap());
            let iface_metric = self.ifaces_metrics.as_ref().unwrap().iter().find(|i| i.prefix == iface_id);

            if iface_metric.is_some() {
                let hosts = self.get_devices(&iface).await;

                match iface_metric.unwrap().set(iface, hosts.unwrap()).await { Err(_) => { warn!("cannot set gauges for iface {}", iface_id)}, _ => {}};
            }
        }

        // match ifaces.len() - self.ifaces_metrics.as_ref().unwrap().len() {
        //     0 => { },
        //     1.. => { },
        //     |i| 0.gt(i) => {}
        // }


        Ok(())
    }

    async fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {

        match self.init_metrics().await {
            Err(e) => return Err(e),
            Ok(metrics) => {
                self.ifaces_metrics = Some(metrics);
            }
        };
        Ok(())
    }

}


#[async_trait]
impl MetricMap for LanBrowserMetricMap {

    async fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        match self.init().await { Err(e) => return Err(e), _  => { } };
        Ok(())
     }

    async fn set(&mut self) -> Result<(), Box<dyn std::error::Error>> {

        match self.set_all().await { Err(e) => return Err(e), _  => { } };
        Ok(())
    }
}
