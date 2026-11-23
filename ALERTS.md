# Alerts

Example alerting rules for the metrics of this exporter, in the Prometheus rule format (also read by vmalert).
Load them with `rule_files` in Prometheus, or `-rule` in vmalert, then adjust them to your setup:

- `FreeboxExporterDown` selects the exporter with `job="freebox"`: use the job name of your scrape configuration.
- The thresholds of `FreeboxFtthLowRxPower`, `FreeboxHighTemperature` and `FreeboxDiskHot` are starting points: sensors and lines differ by model and installation.
- `FreeboxVmStopped` fires for every stopped virtual machine: add a `name` matcher to keep those that must run.

While the box refuses the login of the exporter, no metric of the box is served: `FreeboxExporterLoginRefused` is then the only alert that can fire, and the others resolve. A failed collector keeps exposing its last values: `FreeboxCollectorFailing` and `FreeboxCollectorStale` tell when the other alerts of that collector can no longer be trusted.

The rules were checked with `promtool check rules`, and the less obvious ones with `promtool test rules` (login refused, internet link down with the optical signal still present, no optical signal only once the media is known, VPN client enabled but down, disk errors across a restart of the box and right after it, full partition, dynamic DNS update in progress against failed).

## Exporter

| Alert | Severity | Fires when |
|---|---|---|
| `FreeboxExporterDown` | critical | `up{job="freebox"} == 0` for 5m |
| `FreeboxExporterLoginRefused` | critical | `max by (instance, error_code) (fbx_exporter_authentication_error) > 0` for 5m |
| `FreeboxCollectorFailing` | warning | `fbx_exporter_collector_success == 0` for 5m |
| `FreeboxCollectorStale` | warning | `time() - fbx_exporter_collector_last_success_timestamp_seconds > 300` for 5m |

## Internet link

| Alert | Severity | Fires when |
|---|---|---|
| `FreeboxInternetDown` | critical | `fbx_exporter_connection_state{state="up"} == 0` for 2m |
| `FreeboxFtthNoSignal` | critical | `fbx_exporter_connection_ftth_sfp_has_signal == 0 and on (instance) fbx_exporter_connection_media{media="ftth"}` for 2m |
| `FreeboxFtthLowRxPower` | warning | `fbx_exporter_connection_ftth_sfp_pwr_rx / 100 < -26` for 15m |
| `FreeboxNotAuthenticated` | warning | `fbx_exporter_system_box_authenticated == 0 and on (instance) fbx_exporter_collector_last_success_timestamp_seconds{collector="system"}` for 10m |

## Hardware

| Alert | Severity | Fires when |
|---|---|---|
| `FreeboxFanStopped` | critical | `fbx_exporter_system_fan_speed_rpm == 0` for 5m |
| `FreeboxHighTemperature` | warning | `fbx_exporter_system_temperature_celsius{sensor!~"hdd.*"} > 85` for 15m |

## Storage

| Alert | Severity | Fires when |
|---|---|---|
| `FreeboxDiskError` | critical | `fbx_exporter_nas_disk_state{state="error"}` for 5m |
| `FreeboxDiskIoErrors` | warning | `max by (instance, disk_id, model) (fbx_exporter_nas_disk_read_error_requests_total - fbx_exporter_nas_disk_read_error_requests_total offset 1h) > 0 or max by (instance, disk_id, model) (resets(fbx_exporter_nas_disk_read_error_requests_total[1h]) > 0 and fbx_exporter_nas_disk_read_error_requests_total > 0) or max by (instance, disk_id, model) (fbx_exporter_nas_disk_write_error_requests_total - fbx_exporter_nas_disk_write_error_requests_total offset 1h) > 0 or max by (instance, disk_id, model) (resets(fbx_exporter_nas_disk_write_error_requests_total[1h]) > 0 and fbx_exporter_nas_disk_write_error_requests_total > 0)` |
| `FreeboxDiskHot` | warning | `fbx_exporter_nas_disk_temperature_celsius > 55` for 15m |
| `FreeboxInternalDiskError` | critical | `fbx_exporter_system_disk_status{disk_status="error"} == 1` for 5m |
| `FreeboxPartitionError` | warning | `fbx_exporter_nas_partition_state{state=~"error\|umounted"}` for 10m |
| `FreeboxFilesystemNeedsCorrection` | warning | `fbx_exporter_nas_partition_fsck_result{fsck_result=~"fs_needs_correction\|failed"}` |
| `FreeboxPartitionAlmostFull` | warning | `fbx_exporter_nas_partition_used_bytes / fbx_exporter_nas_partition_total_bytes > 0.9` for 30m |

## RAID

| Alert | Severity | Fires when |
|---|---|---|
| `FreeboxRaidDegraded` | critical | `fbx_exporter_raid_degraded == 1` |
| `FreeboxRaidNotRunning` | critical | `fbx_exporter_raid_state{state!="running"} == 1` for 5m |
| `FreeboxRaidMemberNotActive` | warning | `fbx_exporter_raid_member_role{role!~"active\|spare"} == 1` for 5m |

## Virtual machines, VPN, dynamic DNS

| Alert | Severity | Fires when |
|---|---|---|
| `FreeboxVmStopped` | warning | `fbx_exporter_vm_running == 0` for 5m |
| `FreeboxVpnClientDown` | warning | `fbx_exporter_vpn_client_up == 0 and on (instance, job) fbx_exporter_vpn_client_enabled == 1` for 5m |
| `FreeboxDdnsFailing` | warning | `fbx_exporter_ddns_status{status!~"ok\|disabled"} == 1` for 15m |

## Rules

```yaml
groups:
  - name: freebox-exporter
    rules:
      - alert: FreeboxExporterDown
        # adjust the job selector to your scrape configuration
        expr: up{job="freebox"} == 0
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "freebox-exporter {{ $labels.instance }} cannot be scraped"
          description: "No metric of the box is collected. An unreachable box at startup stops the exporter: check its logs."
      - alert: FreeboxExporterLoginRefused
        expr: max by (instance, error_code) (fbx_exporter_authentication_error) > 0
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "The box refuses the login of the exporter ({{ $labels.error_code }})"
          description: "The exporter serves no metric of the box, so no other alert can fire. invalid_token or pending_token: register the application again; apps_denied: allow the applications in Freebox OS."
      - alert: FreeboxCollectorFailing
        expr: fbx_exporter_collector_success == 0
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Collector {{ $labels.collector }} of freebox-exporter is failing"
          description: "Its metrics keep their last values. The cause is in the logs of the exporter (missing permission, API error, feature not supported by the box)."
      - alert: FreeboxCollectorStale
        expr: time() - fbx_exporter_collector_last_success_timestamp_seconds > 300
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Collector {{ $labels.collector }} of freebox-exporter has not succeeded for 5 minutes"
          description: "Its metrics keep their last values."

  - name: freebox-connection
    rules:
      - alert: FreeboxInternetDown
        expr: fbx_exporter_connection_state{state="up"} == 0
        for: 2m
        labels:
          severity: critical
        annotations:
          summary: "The internet link of the box is down"
          description: "Whatever the media. On FTTH, FreeboxFtthNoSignal tells a missing optical signal apart."
      - alert: FreeboxFtthNoSignal
        # the media is known once the link has been up: before, the signal reads 0 without having been read
        expr: fbx_exporter_connection_ftth_sfp_has_signal == 0 and on (instance) fbx_exporter_connection_media{media="ftth"}
        for: 2m
        labels:
          severity: critical
        annotations:
          summary: "The SFP module of the box receives no optical signal"
          description: "Fibre unplugged or cut, or an outage of the operator."
      - alert: FreeboxFtthLowRxPower
        # GPON receivers stop around -28 dBm: alert before, adjust to your line
        expr: fbx_exporter_connection_ftth_sfp_pwr_rx / 100 < -26
        for: 15m
        labels:
          severity: warning
        annotations:
          summary: "Low optical power received by the box: {{ $value }} dBm"
          description: "A dirty or bent fibre, or a damaged connector, lowers the received power until the link drops."
      - alert: FreeboxNotAuthenticated
        # 0 until the system collector succeeds once
        expr: fbx_exporter_system_box_authenticated == 0 and on (instance) fbx_exporter_collector_last_success_timestamp_seconds{collector="system"}
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "The box is not authenticated on the network of the operator"
          description: "Its services (internet, TV, phone) may be unavailable."

  - name: freebox-hardware
    rules:
      - alert: FreeboxFanStopped
        expr: fbx_exporter_system_fan_speed_rpm == 0
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "Fan {{ $labels.name }} of the box is stopped"
      - alert: FreeboxHighTemperature
        # sensors differ by model: a Freebox Delta runs its CPU around 70 °C, adjust the threshold
        expr: fbx_exporter_system_temperature_celsius{sensor!~"hdd.*"} > 85
        for: 15m
        labels:
          severity: warning
        annotations:
          summary: "Sensor {{ $labels.name }} of the box at {{ $value }} °C"

  - name: freebox-storage
    rules:
      - alert: FreeboxDiskError
        expr: fbx_exporter_nas_disk_state{state="error"}
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "Disk {{ $labels.model }} ({{ $labels.disk_id }}) of the box is in error"
      - alert: FreeboxDiskIoErrors
        # against the value of one hour before (a new series does not fire), or since a restart of the box within the hour
        expr: max by (instance, disk_id, model) (fbx_exporter_nas_disk_read_error_requests_total - fbx_exporter_nas_disk_read_error_requests_total offset 1h) > 0 or max by (instance, disk_id, model) (resets(fbx_exporter_nas_disk_read_error_requests_total[1h]) > 0 and fbx_exporter_nas_disk_read_error_requests_total > 0) or max by (instance, disk_id, model) (fbx_exporter_nas_disk_write_error_requests_total - fbx_exporter_nas_disk_write_error_requests_total offset 1h) > 0 or max by (instance, disk_id, model) (resets(fbx_exporter_nas_disk_write_error_requests_total[1h]) > 0 and fbx_exporter_nas_disk_write_error_requests_total > 0)
        labels:
          severity: warning
        annotations:
          summary: "Disk {{ $labels.model }} ({{ $labels.disk_id }}) of the box had failed requests in the last hour"
          description: "Failed read or write requests may indicate a failing disk."
      - alert: FreeboxDiskHot
        expr: fbx_exporter_nas_disk_temperature_celsius > 55
        for: 15m
        labels:
          severity: warning
        annotations:
          summary: "Disk {{ $labels.model }} ({{ $labels.disk_id }}) of the box at {{ $value }} °C"
      - alert: FreeboxInternalDiskError
        expr: fbx_exporter_system_disk_status{disk_status="error"} == 1
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "The internal disk of the box failed to mount"
      - alert: FreeboxPartitionError
        expr: fbx_exporter_nas_partition_state{state=~"error|umounted"}
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "Partition {{ $labels.label }} of the box is {{ $labels.state }}"
      - alert: FreeboxFilesystemNeedsCorrection
        expr: fbx_exporter_nas_partition_fsck_result{fsck_result=~"fs_needs_correction|failed"}
        labels:
          severity: warning
        annotations:
          summary: "File system check of partition {{ $labels.label }}: {{ $labels.fsck_result }}"
          description: "Run a check of the partition from Freebox OS."
      - alert: FreeboxPartitionAlmostFull
        expr: fbx_exporter_nas_partition_used_bytes / fbx_exporter_nas_partition_total_bytes > 0.9
        for: 30m
        labels:
          severity: warning
        annotations:
          summary: "Partition {{ $labels.label }} of the box is {{ $value | humanizePercentage }} full"

  - name: freebox-raid
    rules:
      - alert: FreeboxRaidDegraded
        expr: fbx_exporter_raid_degraded == 1
        labels:
          severity: critical
        annotations:
          summary: "RAID array {{ $labels.name }} of the box is degraded"
          description: "A member is missing or failed: replace it before another one fails."
      - alert: FreeboxRaidNotRunning
        expr: fbx_exporter_raid_state{state!="running"} == 1
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "RAID array {{ $labels.name }} of the box is {{ $labels.state }}"
      - alert: FreeboxRaidMemberNotActive
        expr: fbx_exporter_raid_member_role{role!~"active|spare"} == 1
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "RAID member at {{ $labels.device_location }} of the box is {{ $labels.role }}"

  - name: freebox-services
    rules:
      - alert: FreeboxVmStopped
        # every stopped virtual machine: restrict with a name matcher to those that must run
        expr: fbx_exporter_vm_running == 0
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Virtual machine {{ $labels.name }} of the box is stopped"
      - alert: FreeboxVpnClientDown
        expr: fbx_exporter_vpn_client_up == 0 and on (instance, job) fbx_exporter_vpn_client_enabled == 1
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "The VPN client of the box is enabled but not connected"
      - alert: FreeboxDdnsFailing
        expr: fbx_exporter_ddns_status{status!~"ok|disabled"} == 1
        for: 15m
        labels:
          severity: warning
        annotations:
          summary: "Dynamic DNS {{ $labels.provider }} of the box is {{ $labels.status }}"
          description: "Check the account of this provider in the dynamic DNS settings of Freebox OS."
```
