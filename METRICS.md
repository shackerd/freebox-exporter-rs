# Metrics

Every metric name starts with the `prefix` of the `[metrics]` section of `config.toml`,
`fbx_exporter` by default, as written below. Each section below is a collector, enabled by its
key in `[metrics]` (e.g. `connection = true`). A collector that is not enabled, or that the box
does not support, exposes nothing. A collector that fails keeps the values of its last successful
run, with the exceptions given in its section; see [Exporter health and login](#exporter-health-and-login).

A field missing from the response of the box gives 0, or no series, as stated per metric.
`rate()` and `increase()` treat any decrease as a counter reset: they are right on a value that
only grows and restarts from 0, wrong on a value that can drop otherwise. Totals exposed as
gauges are noted in each section.

## Exporter health and login

Always exposed. The `collector` label is the key of the collector in `[metrics]`.

| Metric | Type | Labels | Description |
|---|---|---|---|
| `fbx_exporter_authenticated` | gauge | | 1 while the box accepts the login of the exporter, 0 while it refuses it. |
| `fbx_exporter_authentication_error` | gauge | `error_code` | 1 for the `error_code` of the box (`invalid_token`, `pending_token` or `apps_denied`) while it refuses the login; absent otherwise. |
| `fbx_exporter_collector_success` | gauge | `collector` | 1 if the last run of the collector succeeded, 0 otherwise; 0 also for a collector whose initialization failed, and for `vm`, `raid`, `vpn` or `ddns` enabled when the API version of the box could not be detected at startup. |
| `fbx_exporter_collector_last_success_timestamp_seconds` | gauge | `collector` | Unix timestamp of the last successful run of the collector; absent until the first one, kept on failure. |
| `fbx_exporter_collector_duration_seconds` | gauge | `collector` | Duration of the last run of the collector, successful or not, in seconds. |

While the box refuses the login, e.g. after the app was revoked, only
`fbx_exporter_authenticated` and `fbx_exporter_authentication_error` are served: every other
metric, collector health included, is hidden until the login is accepted again. The login is
attempted again every minute.

## connection

Enabled by `connection`. Source: `/api/v4/connection/`, `/connection/config/`,
`/connection/ipv6/config/`, and `/connection/ftth/` or `/connection/xdsl/` depending on the media.

The media of the link is read while the link is up and kept while it is down: the FTTH metrics
are set only for an `ftth` media, the xDSL ones only for an `xdsl` media. A failure of
`/connection/ipv6/config/` keeps the previous IPv6 values and fails the collection, the other
values are updated.

`fbx_exporter_connection_bytes_down` and `fbx_exporter_connection_bytes_up` are totals since the
last connection exposed as gauges. The xDSL error counts are gauges too.

| Metric | Type | Labels | Description |
|---|---|---|---|
| `fbx_exporter_connection_state` | gauge | `state` | 1 if the connection state is `up`, 0 otherwise; the `state` label is always `up`. |
| `fbx_exporter_connection_type` | gauge | `type` | Info gauge, always 1: connection type (`ethernet`, `rfc2684`, `pppoatm`) when the link was last up; absent until the link was seen up. |
| `fbx_exporter_connection_media` | gauge | `media` | Info gauge, always 1: media (`ftth`, `ethernet`, `xdsl`, `backup_4g`) when the link was last up; absent until the link was seen up. |
| `fbx_exporter_connection_ipv4` | gauge | `ipv4` | Info gauge, always 1: public IPv4 address of the box, empty when the box does not give it (only given while the link is up). |
| `fbx_exporter_connection_ipv6` | gauge | `ipv6` | Info gauge, always 1: public IPv6 address of the box, empty when the box does not give it (only given while the link is up). |
| `fbx_exporter_connection_rate_down` | gauge | | Current download rate in bytes per second. |
| `fbx_exporter_connection_rate_up` | gauge | | Current upload rate in bytes per second. |
| `fbx_exporter_connection_bandwidth_down` | gauge | | Available download bandwidth in bits per second. |
| `fbx_exporter_connection_bandwidth_up` | gauge | | Available upload bandwidth in bits per second. |
| `fbx_exporter_connection_bytes_down` | gauge | | Bytes downloaded since the last connection. |
| `fbx_exporter_connection_bytes_up` | gauge | | Bytes uploaded since the last connection. |
| `fbx_exporter_connection_conf_ping` | gauge | | 1 if the box answers external ping requests, 0 otherwise. |
| `fbx_exporter_connection_conf_is_secure_pass` | gauge | | 1 if the admin password is secure enough to enable remote access, 0 otherwise. |
| `fbx_exporter_connection_conf_remote_access` | gauge | | 1 if HTTP remote access is enabled, 0 otherwise. |
| `fbx_exporter_connection_conf_remote_access_port` | gauge | | Port of the HTTP remote access. |
| `fbx_exporter_connection_conf_remote_access_ip` | gauge | `remote_access_ip` | Info gauge: IPv4 used for remote access, empty when the box does not give it; 1 when the box returns the `remote_access` field, 0 otherwise. |
| `fbx_exporter_connection_conf_api_remote_access` | gauge | | 1 if remote access is enabled for apps or share links, 0 otherwise. |
| `fbx_exporter_connection_conf_allow_token_request` | gauge | | 1 if new apps can request a token, 0 if the user disabled it. |
| `fbx_exporter_connection_conf_adblock` | gauge | | 1 if ads blocking is enabled, 0 otherwise. |
| `fbx_exporter_connection_conf_adblock_not_set` | gauge | | 1 if the ads blocking setting has never been set by the user, 0 otherwise. |
| `fbx_exporter_connection_wol_conf` | gauge | | 1 if the Wake-on-LAN proxy is enabled, 0 otherwise. |
| `fbx_exporter_connection_ipv6_conf_ipv6_enabled` | gauge | | 1 if IPv6 is enabled, 0 otherwise. |
| `fbx_exporter_connection_ipv6_conf_delegations` | gauge | `prefix`, `next_hop` | Always 1, one series per delegated IPv6 prefix with its next hop (empty when unset). |
| `fbx_exporter_connection_ffth_sfp_present` | gauge | | 1 if an SFP module is present, 0 otherwise or when the media is not FTTH. |
| `fbx_exporter_connection_ffth_sfp_alim_ok` | gauge | | 1 if the SFP power supply is OK (`sfp_alim_ok`), 0 otherwise or when the media is not FTTH. |
| `fbx_exporter_connection_ftth_sfp_has_power_report` | gauge | | 1 if the SFP reports its optical power, 0 otherwise or when the media is not FTTH. |
| `fbx_exporter_connection_ftth_sfp_has_signal` | gauge | | 1 if the SFP receives a signal, 0 otherwise or when the media is not FTTH. |
| `fbx_exporter_connection_ftth_link` | gauge | | 1 if the FTTH link is up, 0 otherwise or when the media is not FTTH. |
| `fbx_exporter_connection_ftth_sfp_pwr_rx` | gauge | | SFP received optical power in hundredths of dBm (-3698 is -36.98 dBm), 0 when the media is not FTTH. |
| `fbx_exporter_connection_ftth_sfp_pwr_tx` | gauge | | SFP transmitted optical power in hundredths of dBm, 0 when the media is not FTTH. |
| `fbx_exporter_connection_ftth_sfp_model` | gauge | `sfp_model` | Info gauge: SFP model; 1, or 0 with an empty label when the box omits it; absent when the media is not FTTH. |
| `fbx_exporter_connection_ftth_sfp_serial` | gauge | `sfp_serial` | Info gauge: SFP serial number; 1, or 0 with an empty label when the box omits it; absent when the media is not FTTH. |
| `fbx_exporter_connection_ftth_sfp_vendor` | gauge | `sfp_vendor` | Info gauge: SFP vendor; 1, or 0 with an empty label when the box omits it; absent when the media is not FTTH. |
| `fbx_exporter_connection_xdsl_status_uptime` | gauge | `status`, `protocol`, `modulation` | xDSL uptime in seconds; labels: line status (`down`, `training`, `started`, `chan_analysis`, `msg_exchange`, `showtime`, `disabled`), protocol (e.g. `adsl2plus_a`) and modulation (`adsl`, `vdsl`). Absent when the media is not xDSL. |
| `fbx_exporter_connection_xdsl_stats_rate` | gauge | `direction` | ATM rate in kbit/s, for `direction` `up` and `down`. Absent when the media is not xDSL, like every xDSL stat below. |
| `fbx_exporter_connection_xdsl_stats_maxrate` | gauge | `direction` | ATM max rate in kbit/s. |
| `fbx_exporter_connection_xdsl_stats_snr` | gauge | `direction` | Signal to noise ratio (`snr`) in dB. |
| `fbx_exporter_connection_xdsl_stats_attn` | gauge | `direction` | Line attenuation in dB. |
| `fbx_exporter_connection_xdsl_stats_fec` | gauge | `direction` | Value of the `fec` field (forward error correction count, period not documented). |
| `fbx_exporter_connection_xdsl_stats_crc` | gauge | `direction` | Value of the `crc` field (CRC error count, period not documented). |
| `fbx_exporter_connection_xdsl_stats_hec` | gauge | `direction` | Value of the `hec` field (header error check count, period not documented). |
| `fbx_exporter_connection_xdsl_stats_es` | gauge | `direction` | Value of the `es` field (errored seconds, period not documented). |
| `fbx_exporter_connection_xdsl_stats_ses` | gauge | `direction` | Value of the `ses` field (severely errored seconds, period not documented). |
| `fbx_exporter_connection_xdsl_stats_rxmt` | gauge | `direction` | Value of the `rxmt` field, only given when PhyR is on, 0 otherwise (meaning not documented). |
| `fbx_exporter_connection_xdsl_stats_rxmt_corr` | gauge | `direction` | Value of the `rxmt_corr` field, only given when PhyR is on, 0 otherwise (meaning not documented). |
| `fbx_exporter_connection_xdsl_stats_rxmt_uncorr` | gauge | `direction` | Value of the `rxmt_uncorr` field, only given when PhyR is on, 0 otherwise (meaning not documented). |
| `fbx_exporter_connection_xdsl_stats_rtx_tx` | gauge | `direction` | Value of the `rtx_tx` field, only given when G.INP is on, 0 otherwise (meaning not documented). |
| `fbx_exporter_connection_xdsl_stats_rtx_c` | gauge | `direction` | Value of the `rtx_c` field, only given when G.INP is on, 0 otherwise (meaning not documented). |
| `fbx_exporter_connection_xdsl_stats_rtx_uc` | gauge | `direction` | Value of the `rtx_uc` field, only given when G.INP is on, 0 otherwise (meaning not documented). |

## system

Enabled by `system`. Source: `/api/v4/system/`, and `/api/v6/system/` for the fans and the
temperature sensors when the box lists them (API v6 or later, detected at startup).

`fbx_exporter_system_uptime_val` grows while the box is up and restarts from 0 when it restarts.

| Metric | Type | Labels | Description |
|---|---|---|---|
| `fbx_exporter_system_uptime_val` | gauge | | Uptime of the box in seconds. |
| `fbx_exporter_system_temperature_celsius` | gauge | `sensor`, `name` | Temperature of each sensor in °C. From the sensors list of `/api/v6/system/` when the box gives it: `sensor` is the sensor id without its `temp_` prefix, `name` its display name. Otherwise from every `temp_*` field of `/api/v4/system/`: `sensor` and `name` are the field name without `temp_`. |
| `fbx_exporter_system_fan_speed_rpm` | gauge | `id`, `name` | Speed of each fan in RPM, from the fans list of `/api/v6/system/`; absent on a box without that list. |
| `fbx_exporter_system_fan_rpm` | gauge | | Fan speed in RPM from the `fan_rpm` field, 0 when the box does not report it. |
| `fbx_exporter_system_temp_cpu` | gauge | `core` | CPU temperature in °C: `core` `0` to `3` from the `temp_cpu0` to `temp_cpu3` fields when the box reports them, otherwise `b` and `m` from `temp_cpub` and `temp_cpum`. |
| `fbx_exporter_system_temp_cpub` | gauge | | `temp_cpub` in °C, or `temp_cpu0` on a box reporting `temp_cpuN` fields; 0 when neither is reported. |
| `fbx_exporter_system_temp_cpum` | gauge | | `temp_cpum` in °C, or `temp_cpu1` on a box reporting `temp_cpuN` fields; 0 when neither is reported. |
| `fbx_exporter_system_temp_sw` | gauge | | Value of the `temp_sw` field in °C, 0 when the box does not report it. |
| `fbx_exporter_system_temp_hdd` | gauge | | Value of the `temp_hdd` field, 0 when the box does not report it (unit not documented). |
| `fbx_exporter_system_temp_t1` | gauge | | Value of the `temp_t1` field, 0 when the box does not report it (unit not documented). |
| `fbx_exporter_system_temp_t2` | gauge | | Value of the `temp_t2` field, 0 when the box does not report it (unit not documented). |
| `fbx_exporter_system_box_authenticated` | gauge | | 1 if the box is authenticated (`box_authenticated`), 0 otherwise. |
| `fbx_exporter_system_disk_status` | gauge | `disk_status` | Info gauge: internal disk status (`not_detected`, `disabled`, `initializing`, `error`, `active`); 1, or 0 with an empty label when the box omits it. |
| `fbx_exporter_system_user_main_storage` | gauge | `user_main_storage` | Info gauge: label of the partition used for user data; 1, or 0 with an empty label when the box omits it. |
| `fbx_exporter_system_box_flavor` | gauge | `box_flavor` | Info gauge, always 1: box flavor (`full` with internal storage, `light` without), empty when omitted. |
| `fbx_exporter_system_board_name` | gauge | `board_name` | Info gauge, always 1: hardware revision, empty when omitted. |
| `fbx_exporter_system_firmware_version` | gauge | `firmware_version` | Info gauge, always 1: firmware version, empty when omitted. |
| `fbx_exporter_system_mac` | gauge | `mac` | Info gauge, always 1: MAC address of the box, empty when omitted. |
| `fbx_exporter_system_serial` | gauge | `serial` | Info gauge, always 1: serial number of the box, empty when omitted. |
| `fbx_exporter_system_model_info` | gauge | `model`, `model_name` | Info gauge, always 1: model of the box (`box_model` and `box_model_name` of `/api_version`, for example `fbxgw7-r1/full` and `Freebox v7 (r1)`), read once at the start of the exporter; absent when `/api_version` does not give them. |
| `fbx_exporter_system_box_model_name` | gauge | `model_name` | Deprecated: use `fbx_exporter_system_model_info`, kept for compatibility. Info gauge, always 1: `box_model_name` field of `v4/system`, which the box does not return (absent in practice). |
| `fbx_exporter_system_device_name` | gauge | `device_name` | Deprecated, kept for compatibility. Info gauge, always 1: `device_name` field of `v4/system`, which the box does not return (absent in practice). |
| `fbx_exporter_system_api_version` | gauge | `api_version` | Info gauge, always 1: `api_version` field of `v4/system`, which the box does not return (absent in practice). |

## lan

Enabled by `lan`. Source: `/api/v4/lan/config/`.

Each metric is an info gauge: the label holds the value of the field; 1, or 0 with an empty label
when the box omits the field.

| Metric | Type | Labels | Description |
|---|---|---|---|
| `fbx_exporter_lan_config_ip` | gauge | `ip` | IPv4 address of the box on the LAN. |
| `fbx_exporter_lan_config_mode` | gauge | `mode` | Network mode: `router` or `bridge`. |
| `fbx_exporter_lan_config_name` | gauge | `name` | Name of the box. |
| `fbx_exporter_lan_config_name_dns` | gauge | `name_dns` | DNS name of the box. |
| `fbx_exporter_lan_config_name_mdns` | gauge | `name_mdns` | mDNS name of the box. |
| `fbx_exporter_lan_config_name_netbios` | gauge | `name_netbios` | NetBIOS name of the box. |

## lan_browser

Enabled by `lan_browser`, in router mode only. Source: `/api/v4/lan/browser/interfaces/` and
`/api/v4/lan/browser/{interface}/` for each interface with hosts.

When the hosts of an interface cannot be fetched, the collection fails: the other interfaces are
updated, and the hosts of that interface are no longer exposed until the next success.

| Metric | Type | Labels | Description |
|---|---|---|---|
| `fbx_exporter_lan_browser_iface_hosts` | gauge | `name` | Number of hosts of each LAN browser interface (`host_count`). |
| `fbx_exporter_lan_browser_device` | gauge | `iface`, `primary_name`, `id`, `type`, `primary_name_manual`, `l2ident_id`, `l2ident_type`, `vendor_name` | 1 if the host is active (sends traffic to the box), 0 otherwise; one series per host. `type` is the host type (e.g. `smartphone`), `primary_name_manual` is `true` when the user set the name, `l2ident_id` and `l2ident_type` are its layer 2 id and id type. |
| `fbx_exporter_lan_browser_device_l3_connectivity` | gauge | `ident`, `iface`, `addr`, `name`, `af` | 1 if the layer 3 connection of the host is active, 0 otherwise; one series per address. `ident` is the layer 2 id, `name` the primary name, `af` `ipv4` or `ipv6`. |
| `fbx_exporter_lan_browser_device_last_activity` | gauge | `iface`, `name` | Unix timestamp of the last time the host sent traffic, 0 when unknown; hosts sharing a primary name on an interface share the series. |
| `fbx_exporter_lan_browser_device_name` | gauge | `name`, `source`, `ident`, `iface` | Always 1, one series per name of the host, with the source of the name (`dhcp`, `netbios`, `mdns`, `mdns_srv`, `upnp`, `wsd`) and the layer 2 id. |

## switch

Enabled by `switch`, in router mode only. Source: `/api/v4/switch/status/` and
`/api/v4/switch/port/{id}/stats/`. The `port` label is the switch port id.

The byte and packet counts are totals kept by the box, exposed as gauges; the API does not
document when they reset. The rates are values computed by the box.

| Metric | Type | Labels | Description |
|---|---|---|---|
| `fbx_exporter_switch_port_status` | gauge | `port` | 1 if the port link is up, 0 otherwise. |
| `fbx_exporter_switch_port_speed` | gauge | `port` | Port speed in Mbit/s (10, 100, 1000), 0 when the box gives no numeric speed. |
| `fbx_exporter_switch_port_mac_list` | gauge | `port`, `mac`, `hostname` | Always 1, one series per host the box sees on the port. |
| `fbx_exporter_switch_stats_rx_bytes_rate` | gauge | `port` | Receive rate in bytes (`rx_bytes_rate`, time unit not documented). |
| `fbx_exporter_switch_stats_tx_bytes_rate` | gauge | `port` | Transmit rate in bytes (`tx_bytes_rate`, time unit not documented). |
| `fbx_exporter_switch_stats_rx_packets_rate` | gauge | `port` | Receive rate in packets (`rx_packets_rate`, time unit not documented). |
| `fbx_exporter_switch_stats_tx_packets_rate` | gauge | `port` | Transmit rate in packets (`tx_packets_rate`, time unit not documented). |
| `fbx_exporter_switch_stats_rx_good_bytes` | gauge | `port` | Bytes received in good packets. |
| `fbx_exporter_switch_stats_rx_bad_bytes` | gauge | `port` | Bytes received in bad packets. |
| `fbx_exporter_switch_stats_tx_bytes` | gauge | `port` | Bytes transmitted. |
| `fbx_exporter_switch_stats_rx_good_packets` | gauge | `port` | Good packets received. |
| `fbx_exporter_switch_stats_tx_packets` | gauge | `port` | Packets transmitted. |
| `fbx_exporter_switch_stats_rx_unicast_packets` | gauge | `port` | Unicast packets received. |
| `fbx_exporter_switch_stats_tx_unicast_packets` | gauge | `port` | Unicast packets transmitted. |
| `fbx_exporter_switch_stats_rx_multicast_packets` | gauge | `port` | Multicast packets received. |
| `fbx_exporter_switch_stats_tx_multicast_packets` | gauge | `port` | Multicast packets transmitted. |
| `fbx_exporter_switch_stats_rx_broadcast_packets` | gauge | `port` | Broadcast packets received. |
| `fbx_exporter_switch_stats_tx_broadcast_packets` | gauge | `port` | Broadcast packets transmitted. |
| `fbx_exporter_switch_stats_rx_err_packets` | gauge | `port` | Packets received with an error. |
| `fbx_exporter_switch_stats_rx_fcs_packets` | gauge | `port` | Packets received with an FCS error. |
| `fbx_exporter_switch_stats_rx_fragments_packets` | gauge | `port` | Fragment packets received. |
| `fbx_exporter_switch_stats_rx_jabber_packets` | gauge | `port` | Jabber packets received. |
| `fbx_exporter_switch_stats_rx_oversize_packets` | gauge | `port` | Oversize packets received. |
| `fbx_exporter_switch_stats_rx_undersize_packets` | gauge | `port` | Undersize packets received. |
| `fbx_exporter_switch_stats_rx_pause` | gauge | `port` | Pause frames received. |
| `fbx_exporter_switch_stats_tx_pause` | gauge | `port` | Pause frames transmitted. |
| `fbx_exporter_switch_stats_tx_collisions` | gauge | `port` | Transmit collisions. |
| `fbx_exporter_switch_stats_tx_single` | gauge | `port` | Value of the `tx_single` field (meaning not documented). |
| `fbx_exporter_switch_stats_tx_multiple` | gauge | `port` | Value of the `tx_multiple` field (meaning not documented). |
| `fbx_exporter_switch_stats_tx_late` | gauge | `port` | Value of the `tx_late` field (meaning not documented). |
| `fbx_exporter_switch_stats_tx_excessive` | gauge | `port` | Value of the `tx_excessive` field (meaning not documented). |
| `fbx_exporter_switch_stats_tx_deferred` | gauge | `port` | Value of the `tx_deferred` field (meaning not documented). |
| `fbx_exporter_switch_stats_tx_fcs` | gauge | `port` | Value of the `tx_fcs` field (meaning not documented). |

## wifi

Enabled by `wifi`, when Wi-Fi is enabled on the box. Source: `/api/v4/wifi/ap/` (or each
`expected_phys` of `/api/v4/wifi/config/` when the list is empty), then for each access point
`channel_survey_history`, `channel_usage`, `neighbors` and `stations`. The `ap` and `ap_id`
labels are the access point id, `name` and `ap_name` its name, `band` its band (e.g. `2d4g`, `5g`).

A failure of `channel_usage` or `neighbors` only removes their series. When some access points
of `expected_phys` cannot be fetched, the collection fails and the others are updated.

A station is exposed only with its host, its IPv4 address (the most recently reachable one, in
the `ipv4` label), its flags and its last rx and tx stats. With `unresolved_station_hostnames` of
the `[policies]` section set to another value than `ignore`, a station with a host but without
the rest is exposed with `primary_name` and `ipv4` set to `unresolved` and 0 for the link values.

`fbx_exporter_wifi_station_rx_bytes` and `fbx_exporter_wifi_station_tx_bytes` are totals exposed
as gauges.

| Metric | Type | Labels | Description |
|---|---|---|---|
| `fbx_exporter_wifi_busy_percent` | gauge | `ap`, `name`, `band` | Percentage of time the channel was sensed busy, integer average of the most recent channel survey entries (10 per second of `refresh` of `[api]`). Absent when the box returns no survey history result, like the 3 metrics below. |
| `fbx_exporter_wifi_tx_percent` | gauge | `ap`, `name`, `band` | Percentage of time spent sending on the channel, averaged likewise. |
| `fbx_exporter_wifi_rx_percent` | gauge | `ap`, `name`, `band` | Percentage of time spent receiving Wi-Fi traffic on the channel, averaged likewise. |
| `fbx_exporter_wifi_rx_bss_percent` | gauge | `ap`, `name`, `band` | Percentage of time spent receiving Wi-Fi traffic for a local BSS, averaged likewise. |
| `fbx_exporter_wifi_channel_usage` | gauge | `band`, `channel`, `rx_busy_percent` | Noise level of each channel in dB (-128 when not given); the `rx_busy_percent` label is the rx busy time percentage of the channel. |
| `fbx_exporter_wifi_neighbors_access_point` | gauge | `channel`, `ssid`, `bssid`, `band`, `vht`, `legacy`, `he`, `ht`, `eht`, `secondary_channel` | Signal of each neighbor access point seen by the box, in dB as the API reports it (-128 when not given); the capability labels are `true` or `false`. |
| `fbx_exporter_wifi_station_active` | gauge | `primary_name`, `ap_name`, `band`, `ap_id`, `mac`, `vendor_name` | 1 if the host of the station is active (sends traffic to the box), 0 otherwise. |
| `fbx_exporter_wifi_station_state` | gauge | `primary_name`, `ipv4`, `ap_name`, `band`, `ap_id`, `mac`, `state` | Info gauge, always 1: station state (`associated`, `authenticated`). |
| `fbx_exporter_wifi_station_flags` | gauge | `primary_name`, `ipv4`, `ap_name`, `band`, `ap_id`, `mac`, `vht`, `legacy`, `authorized`, `ht` | Always 0: the station flags are the `true` or `false` labels. |
| `fbx_exporter_wifi_station_signal` | gauge | `primary_name`, `ipv4`, `ap_name`, `band`, `ap_id`, `mac` | Signal of the station in dB as the API reports it (-128 when not given). |
| `fbx_exporter_wifi_station_inactive` | gauge | `primary_name`, `ipv4`, `ap_name`, `band`, `ap_id`, `mac` | Inactive duration of the station in seconds (-2^63 when not given). |
| `fbx_exporter_wifi_station_last_activity` | gauge | `primary_name`, `ipv4`, `ap_name`, `band`, `ap_id`, `mac` | Unix timestamp of the last time the host sent traffic (-2^63 when not given). |
| `fbx_exporter_wifi_station_last_time_reachable` | gauge | `primary_name`, `ipv4`, `ap_name`, `band`, `ap_id`, `mac` | Unix timestamp of the last time the host was reached (-2^63 when not given). |
| `fbx_exporter_wifi_station_rx_bytes` | gauge | `primary_name`, `ipv4`, `ap_name`, `band`, `ap_id`, `mac` | Bytes received from the station. |
| `fbx_exporter_wifi_station_tx_bytes` | gauge | `primary_name`, `ipv4`, `ap_name`, `band`, `ap_id`, `mac` | Bytes transmitted to the station. |
| `fbx_exporter_wifi_station_rx_rate` | gauge | `primary_name`, `ipv4`, `ap_name`, `band`, `ap_id`, `mac` | Data rate of the station, `rx_rate` field, in bytes per second. |
| `fbx_exporter_wifi_station_tx_rate` | gauge | `primary_name`, `ipv4`, `ap_name`, `band`, `ap_id`, `mac` | Data rate of the station, `tx_rate` field, in bytes per second. |
| `fbx_exporter_wifi_station_rx_bitrate` | gauge | `primary_name`, `ipv4`, `ap_name`, `band`, `ap_id`, `mac` | Physical link rate of the last rx stats, in tenths of Mbit/s. |
| `fbx_exporter_wifi_station_tx_bitrate` | gauge | `primary_name`, `ipv4`, `ap_name`, `band`, `ap_id`, `mac` | Physical link rate of the last tx stats, in tenths of Mbit/s. |
| `fbx_exporter_wifi_station_rx_mcs` | gauge | `primary_name`, `ipv4`, `ap_name`, `band`, `ap_id`, `mac` | MCS of the last rx stats, -1 if not used. |
| `fbx_exporter_wifi_station_tx_mcs` | gauge | `primary_name`, `ipv4`, `ap_name`, `band`, `ap_id`, `mac` | MCS of the last tx stats, -1 if not used. |
| `fbx_exporter_wifi_station_rx_vht_mcs` | gauge | `primary_name`, `ipv4`, `ap_name`, `band`, `ap_id`, `mac` | VHT MCS of the last rx stats, -1 if not used. |
| `fbx_exporter_wifi_station_tx_vht_mcs` | gauge | `primary_name`, `ipv4`, `ap_name`, `band`, `ap_id`, `mac` | VHT MCS of the last tx stats, -1 if not used. |
| `fbx_exporter_wifi_station_rx_shortgi` | gauge | `primary_name`, `ipv4`, `ap_name`, `band`, `ap_id`, `mac` | 1 if short guard interval is enabled in the last rx stats, 0 otherwise. |
| `fbx_exporter_wifi_station_tx_shortgi` | gauge | `primary_name`, `ipv4`, `ap_name`, `band`, `ap_id`, `mac` | 1 if short guard interval is enabled in the last tx stats, 0 otherwise. |
| `fbx_exporter_wifi_station_rx_width` | gauge | `primary_name`, `ipv4`, `ap_name`, `band`, `ap_id`, `mac` | Channel width of the last rx stats, the `width` string read as a number (e.g. `80`), 0 when it is not a number. |
| `fbx_exporter_wifi_station_tx_width` | gauge | `primary_name`, `ipv4`, `ap_name`, `band`, `ap_id`, `mac` | Channel width of the last tx stats, read likewise. |

## dhcp

Enabled by `dhcp`, in router mode only. Source: `/api/v4/dhcp/dynamic_lease/` and
`/api/v4/dhcp/static_lease/`. One series per lease; `is_static` is `true` or `false`.

| Metric | Type | Labels | Description |
|---|---|---|---|
| `fbx_exporter_dhcp_assign_time` | gauge | `id`, `hostname`, `ip`, `mac`, `is_static` | Unix timestamp of the first assignment of a dynamic lease; 0 for a static lease. |
| `fbx_exporter_dhcp_refresh_time` | gauge | `id`, `hostname`, `ip`, `mac`, `is_static` | Unix timestamp of the last refresh of a dynamic lease; -1 for a static lease. |
| `fbx_exporter_dhcp_lease_remaining` | gauge | `id`, `hostname`, `ip`, `mac`, `is_static` | Time left before a dynamic lease must be refreshed (unit not documented); -1 for a static lease. |

## nas

Enabled by `nas`. Source: `/api/v4/storage/disk/`. A disk is identified by `disk_id`,
`disk_type` (`internal`, `usb`, `sata`, `nvme`), `model` and `serial`; a partition by
`partition_id`, `disk_id`, `label` and `fstype`. A missing label value is `unknown`. A metric
whose field the box does not report for a disk or partition has no series for it, except the
states.

The 4 request counters follow the totals of the box since its start: they restart from the
value of the box when it decreases, after a restart of the box. They are removed with the disk,
or when the box no longer reports them.

| Metric | Type | Labels | Description |
|---|---|---|---|
| `fbx_exporter_nas_disk_state` | gauge | `disk_id`, `disk_type`, `model`, `serial`, `state` | Disk state: 1 `enabled`, 0 `disabled`, -1 `error`, 2 `formatting`, 0 for any other state; `state` holds the state. |
| `fbx_exporter_nas_disk_total_bytes` | gauge | `disk_id`, `disk_type`, `model`, `serial` | Disk size in bytes. |
| `fbx_exporter_nas_disk_temperature_celsius` | gauge | `disk_id`, `disk_type`, `model`, `serial` | Disk temperature in °C, when supported. |
| `fbx_exporter_nas_disk_spinning` | gauge | `disk_id`, `disk_type`, `model`, `serial` | 1 if the disk is spinning, 0 otherwise. |
| `fbx_exporter_nas_disk_idle` | gauge | `disk_id`, `disk_type`, `model`, `serial` | 1 if the disk is idle, 0 otherwise. |
| `fbx_exporter_nas_disk_idle_duration_seconds` | gauge | `disk_id`, `disk_type`, `model`, `serial` | Idle duration of the disk in seconds. |
| `fbx_exporter_nas_disk_active_duration_seconds` | gauge | `disk_id`, `disk_type`, `model`, `serial` | Activity duration of the disk in seconds. |
| `fbx_exporter_nas_disk_time_before_spindown_seconds` | gauge | `disk_id`, `disk_type`, `model`, `serial` | Seconds left before the disk spins down. |
| `fbx_exporter_nas_disk_read_requests_total` | counter | `disk_id`, `disk_type`, `model`, `serial` | Read requests counted by the box since its start: a restart of the box resets the counter, use `increase()` or `rate()`. Absent when the box does not report it. |
| `fbx_exporter_nas_disk_read_error_requests_total` | counter | `disk_id`, `disk_type`, `model`, `serial` | Failed read requests counted by the box since its start: a restart of the box resets the counter, use `increase()` or `rate()`. Absent when the box does not report it. |
| `fbx_exporter_nas_disk_write_requests_total` | counter | `disk_id`, `disk_type`, `model`, `serial` | Write requests counted by the box since its start: a restart of the box resets the counter, use `increase()` or `rate()`. Absent when the box does not report it. |
| `fbx_exporter_nas_disk_write_error_requests_total` | counter | `disk_id`, `disk_type`, `model`, `serial` | Failed write requests counted by the box since its start: a restart of the box resets the counter, use `increase()` or `rate()`. Absent when the box does not report it. |
| `fbx_exporter_nas_disk_connector_info` | gauge | `disk_id`, `disk_type`, `model`, `serial`, `connector` | Info gauge, always 1: physical connector id of the disk. |
| `fbx_exporter_nas_disk_table_type_info` | gauge | `disk_id`, `disk_type`, `model`, `serial`, `table_type` | Info gauge, always 1: partition table type (`msdos`, `gpt`, `superfloppy`, `empty`). |
| `fbx_exporter_nas_disk_firmware_info` | gauge | `disk_id`, `disk_type`, `model`, `serial`, `firmware` | Info gauge, always 1: firmware version of the disk; absent when empty. |
| `fbx_exporter_nas_disk_operation_progress_percent` | gauge | `disk_id`, `disk_type`, `model`, `serial` | Progress of the current step of the disk operation, in percent. |
| `fbx_exporter_nas_disk_operation_steps` | gauge | `disk_id`, `disk_type`, `model`, `serial`, `step_type` | Steps of the disk operation: done (`step_type="done"`) and total (`step_type="total"`). |
| `fbx_exporter_nas_partition_state` | gauge | `partition_id`, `disk_id`, `label`, `fstype`, `state` | Partition state: 1 `mounted`, 0 `umounted`, -1 `error`, 2 `checking`, 3 `formatting`, 4 `mounting`, 5 `maintenance`, 6 `umounting`, 7 `ejecting`, 0 for any other state; `state` holds the state. |
| `fbx_exporter_nas_partition_total_bytes` | gauge | `partition_id`, `disk_id`, `label`, `fstype` | Partition size in bytes. |
| `fbx_exporter_nas_partition_used_bytes` | gauge | `partition_id`, `disk_id`, `label`, `fstype` | Partition used space in bytes. |
| `fbx_exporter_nas_partition_free_bytes` | gauge | `partition_id`, `disk_id`, `label`, `fstype` | Partition free space in bytes. |
| `fbx_exporter_nas_partition_fsck_result` | gauge | `partition_id`, `disk_id`, `label`, `fstype`, `fsck_result` | File system check result: 0 `no_run_yet`, 1 `running`, 2 `fs_clean`, 3 `fs_corrected`, 4 `fs_needs_correction`, -1 `failed`, 0 for any other value; `fsck_result` holds the result. |
| `fbx_exporter_nas_partition_operation_progress_percent` | gauge | `partition_id`, `disk_id`, `label`, `fstype` | Progress of the current step of the partition operation, in percent. |
| `fbx_exporter_nas_partition_operation_steps` | gauge | `partition_id`, `disk_id`, `label`, `fstype`, `step_type` | Steps of the partition operation: done (`step_type="done"`) and total (`step_type="total"`). |

## download

Enabled by `download`. Source: `/api/v4/downloads/stats/`. A field the box does not report gives
0.

`fbx_exporter_download_tasks_total` and `fbx_exporter_download_peers_total` are current counts,
gauges despite their `_total` suffix.

| Metric | Type | Labels | Description |
|---|---|---|---|
| `fbx_exporter_download_rx_rate_bytes_per_second` | gauge | | Current receive rate of the downloader in bytes per second. |
| `fbx_exporter_download_tx_rate_bytes_per_second` | gauge | | Current transmit rate of the downloader in bytes per second. |
| `fbx_exporter_download_tasks_total` | gauge | | Number of download tasks. |
| `fbx_exporter_download_tasks_active` | gauge | | Number of active tasks (checking, queued, extracting, repairing, seeding and downloading). |
| `fbx_exporter_download_tasks_downloading` | gauge | | Number of downloading tasks. |
| `fbx_exporter_download_tasks_seeding` | gauge | | Number of seeding tasks. |
| `fbx_exporter_download_tasks_queued` | gauge | | Number of queued tasks. |
| `fbx_exporter_download_tasks_checking` | gauge | | Number of checking tasks. |
| `fbx_exporter_download_tasks_extracting` | gauge | | Number of extracting tasks. |
| `fbx_exporter_download_tasks_repairing` | gauge | | Number of repairing tasks. |
| `fbx_exporter_download_tasks_stopping` | gauge | | Number of stopping tasks. |
| `fbx_exporter_download_tasks_stopped` | gauge | | Number of stopped tasks. |
| `fbx_exporter_download_tasks_done` | gauge | | Number of done tasks. |
| `fbx_exporter_download_tasks_error` | gauge | | Number of tasks in error. |
| `fbx_exporter_download_rss_items_unread` | gauge | | Number of unread RSS items. |
| `fbx_exporter_download_peers_total` | gauge | | Number of BitTorrent peers. |
| `fbx_exporter_download_connection_ready` | gauge | | 1 if the downloader connection is ready, 0 otherwise. |
| `fbx_exporter_download_throttling_mode_info` | gauge | `mode` | Info gauge, always 1: active throttling mode (`normal`, `slow`, `hibernate`, `schedule`), `unknown` when not reported. |
| `fbx_exporter_download_throttling_is_scheduled` | gauge | | 1 if the throttling mode comes from the schedule, 0 if it was forced manually. |
| `fbx_exporter_download_throttling_rx_rate_bytes_per_second` | gauge | | Receive rate limit of the current throttling in bytes per second. |
| `fbx_exporter_download_throttling_tx_rate_bytes_per_second` | gauge | | Transmit rate limit of the current throttling in bytes per second. |

## vm

Enabled by `vm`, on a box with API v7 or later that supports virtual machines. Source:
`/api/v7/vm/info/` and `/api/v7/vm/`. A virtual machine is identified by `id` and `name`. A value
the box does not report has no series.

| Metric | Type | Labels | Description |
|---|---|---|---|
| `fbx_exporter_vm_host_memory_total_bytes` | gauge | | Memory available to virtual machines in bytes (`total_memory` × 1048576, its unit is not documented). |
| `fbx_exporter_vm_host_memory_allocated_bytes` | gauge | | Memory used by all virtual machines in bytes (`used_memory` × 1048576, its unit is not documented). |
| `fbx_exporter_vm_host_vcpus` | gauge | | Virtual CPUs available to virtual machines. |
| `fbx_exporter_vm_host_vcpus_allocated` | gauge | | Virtual CPUs used by all virtual machines. |
| `fbx_exporter_vm_running` | gauge | `id`, `name` | 1 if the virtual machine is running, 0 otherwise. |
| `fbx_exporter_vm_status` | gauge | `id`, `name`, `status` | Info gauge, always 1: status (`stopped`, `running`, `starting`, `stopping`), `unknown` when not reported. |
| `fbx_exporter_vm_info` | gauge | `id`, `name`, `os`, `disk_type` | Info gauge, always 1: OS (e.g. `debian`) and disk image type (`raw`, `qcow2`), `unknown` when not reported. |
| `fbx_exporter_vm_memory_allocated_bytes` | gauge | `id`, `name` | Memory allocated to the virtual machine in bytes (megabytes of the API × 1048576). |
| `fbx_exporter_vm_vcpus` | gauge | `id`, `name` | Virtual CPUs allocated to the virtual machine. |

## raid

Enabled by `raid`, on a box with API v8 or later. Source: `/api/v8/storage/raid/`. An array is
identified by `id`, `name` and `level` (`basic`, `raid0`, `raid1`, `raid5`, `raid10`, `unknown`
when not reported), a member by `array_id`, `id` (its disk id) and `device_location` (e.g.
`sata-internal-p0`). A value the box does not report has no series.

`fbx_exporter_raid_member_corrected_read_errors` is a count exposed as a gauge.

| Metric | Type | Labels | Description |
|---|---|---|---|
| `fbx_exporter_raid_state` | gauge | `id`, `name`, `state` | Info gauge, always 1: array state (`stopped`, `running`, `error`). |
| `fbx_exporter_raid_degraded` | gauge | `id`, `name`, `level` | 1 if the array is degraded, 0 otherwise. |
| `fbx_exporter_raid_disks_expected` | gauge | `id`, `name`, `level` | Number of members that should be in the array. |
| `fbx_exporter_raid_size_bytes` | gauge | `id`, `name`, `level` | Array size in bytes. |
| `fbx_exporter_raid_sync_action` | gauge | `id`, `name`, `sync_action` | Info gauge, always 1: sync action (`idle`, `resync`, `recover`, `check`, `repair`, `reshape`, `frozen`). |
| `fbx_exporter_raid_sync_completed_ratio` | gauge | `id`, `name`, `level` | Sync completion from 0 to 1 (percentage of the API / 100). |
| `fbx_exporter_raid_sync_speed_bytes_per_second` | gauge | `id`, `name`, `level` | Sync speed in bytes per second. |
| `fbx_exporter_raid_last_check_timestamp_seconds` | gauge | `id`, `name`, `level` | Unix timestamp of the last check; absent when 0. |
| `fbx_exporter_raid_next_check_timestamp_seconds` | gauge | `id`, `name`, `level` | Unix timestamp of the next check; absent when 0, e.g. when checks are disabled. |
| `fbx_exporter_raid_member_role` | gauge | `array_id`, `id`, `device_location`, `role` | Info gauge, always 1: member role (`active`, `faulty`, `spare`, `missing`). |
| `fbx_exporter_raid_member_size_bytes` | gauge | `array_id`, `id`, `device_location` | Member size in bytes. |
| `fbx_exporter_raid_member_corrected_read_errors` | gauge | `array_id`, `id`, `device_location` | Read errors count of the member device. |
| `fbx_exporter_raid_member_temperature_celsius` | gauge | `array_id`, `id`, `device_location` | Temperature of the member disk in °C. |

## vpn

Enabled by `vpn`, in router mode on a box with API v4 or later. Source: `/api/v4/vpn/`,
`/api/v4/vpn/connection/` and `/api/v4/vpn_client/status/`. A VPN server is identified by `name`
and `type` (`ipsec`, `pptp`, `openvpn`, `wireguard`), `unknown` when not reported. A value the
box does not report has no series.

`fbx_exporter_vpn_connections_rx_bytes` and `fbx_exporter_vpn_connections_tx_bytes` are sums
over the open connections, gauges that drop when a connection closes: `rate()` on them is wrong.
The 2 client counters follow the totals of the box: they restart from the value of the box when
it decreases, and are removed when the box no longer reports them.

| Metric | Type | Labels | Description |
|---|---|---|---|
| `fbx_exporter_vpn_server_state` | gauge | `name`, `type`, `state` | Info gauge, always 1: server state (`stopped`, `starting`, `started`, `stopping`, `error`). |
| `fbx_exporter_vpn_server_started` | gauge | `name`, `type` | 1 if the server state is `started`, 0 otherwise. |
| `fbx_exporter_vpn_server_connections` | gauge | `name`, `type` | Number of active connections to the server. |
| `fbx_exporter_vpn_server_authenticated_connections` | gauge | `name`, `type` | Number of active connections that passed authentication. |
| `fbx_exporter_vpn_connections_rx_bytes` | gauge | `vpn` | Bytes received by the open connections of each server (`vpn`, `unknown` when not reported); absent without open connection. |
| `fbx_exporter_vpn_connections_tx_bytes` | gauge | `vpn` | Bytes sent by the open connections of each server; absent without open connection. |
| `fbx_exporter_vpn_client_enabled` | gauge | | 1 if the VPN client is enabled, 0 otherwise. |
| `fbx_exporter_vpn_client_up` | gauge | | 1 if the VPN client state is `up`, 0 otherwise. |
| `fbx_exporter_vpn_client_state` | gauge | `state` | Info gauge, always 1: client state (`waiting_wan`, `going_up`, `up`, `going_down`, `down`). |
| `fbx_exporter_vpn_client_last_error` | gauge | `last_error` | Info gauge, always 1: last client error (`none`, `authentication_failed`, `connect_timeout`, ...). |
| `fbx_exporter_vpn_client_last_up_timestamp_seconds` | gauge | | Unix timestamp of the last successful client connection; absent when the client never connected. |
| `fbx_exporter_vpn_client_rate_up_bytes_per_second` | gauge | | Current upload rate of the client in bytes per second. |
| `fbx_exporter_vpn_client_rate_down_bytes_per_second` | gauge | | Current download rate of the client in bytes per second. |
| `fbx_exporter_vpn_client_uploaded_bytes_total` | counter | | Total bytes uploaded by the client, as reported by the box. |
| `fbx_exporter_vpn_client_downloaded_bytes_total` | counter | | Total bytes downloaded by the client, as reported by the box. |

## ddns

Enabled by `ddns`, in router mode on a box with API v4 or later. Source:
`/api/v4/connection/ddns/{provider}/status/` for each provider known to the exporter. A
timestamp that is 0, an event that never happened or is not scheduled, has no series.

| Metric | Type | Labels | Description |
|---|---|---|---|
| `fbx_exporter_ddns_status` | gauge | `provider`, `status` | Info gauge, always 1: provider status (`disabled`, `ok`, `wait`, `reqfail`, `authfail`, `nocredential`, `ipinval`, `hostinval`, `abuse`, `dnserror`, `unavailable`, `nowan`, `unknown`). |
| `fbx_exporter_ddns_last_refresh_timestamp_seconds` | gauge | `provider` | Unix timestamp of the last refresh. |
| `fbx_exporter_ddns_next_refresh_timestamp_seconds` | gauge | `provider` | Unix timestamp of the next refresh. |
| `fbx_exporter_ddns_last_error_timestamp_seconds` | gauge | `provider` | Unix timestamp of the last error. |
| `fbx_exporter_ddns_next_retry_timestamp_seconds` | gauge | `provider` | Unix timestamp of the next retry after an error. |
