# Troubleshooting & Bug Reporting

If you encounter issues with the exporter, the following diagnostic commands will help you investigate and report bugs effectively.

## Getting Diagnostic Information

The exporter provides a built-in session diagnostic command that can help identify authentication and API connectivity issues:

```bash
# Get session diagnostic information
./freebox-exporter-rs session-diagnostic true
```

This command will output session token information and verify your authentication with the Freebox API.

## Collecting Raw API Data for Bug Reports

When reporting bugs, especially those related to missing metrics or parsing errors, it's extremely helpful to include the raw JSON data from your Freebox. This allows developers to reproduce the exact issue with your hardware configuration.

**Steps to collect diagnostic data:**

1. **Get your session token:**
   ```bash
   TOKEN=$(./freebox-exporter-rs session-diagnostic true | awk -F': ' '/^SESSION_TOKEN:/ {print $2}')
   ```

2. **Collect raw API data for the problematic endpoint:**
   ```bash
   # For system-related issues (CPU, temperature, etc.)
   curl -ks -H "X-Fbx-App-Auth: $TOKEN" https://mafreebox.freebox.fr/api/v4/system | jq . > system_data.json
   
   # For WiFi-related issues
   curl -ks -H "X-Fbx-App-Auth: $TOKEN" https://mafreebox.freebox.fr/api/v2/wifi/ap/0/stations | jq . > wifi_data.json
   
   # For connection-related issues
   curl -ks -H "X-Fbx-App-Auth: $TOKEN" https://mafreebox.freebox.fr/api/v4/connection | jq . > connection_data.json
   
   # For DHCP-related issues
   curl -ks -H "X-Fbx-App-Auth: $TOKEN" https://mafreebox.freebox.fr/api/v2/dhcp/dynamic_lease | jq . > dhcp_data.json
   ```

3. **Anonymize sensitive data before sharing:**
   - Replace real MAC addresses with `XX:XX:XX:XX:XX:XX`
   - Replace serial numbers with `FBXXXXXXXXXXXXXXX`
   - Replace IP addresses with `192.168.1.XXX`
   - Replace device names/hostnames with generic names

4. **Include in your bug report:**
   - Your Freebox model (Generation 8, Ultra, etc.)
   - Firmware version from the JSON data
   - The anonymized JSON file(s)
   - Exact error message or unexpected behavior
   - Relevant log output with verbosity enabled

## Common Issues

### Authentication failures
- Ensure the Freebox allows LAN API access (Settings → Network → API)
- Check if the application is properly registered: `./freebox-exporter-rs register`
- Verify the Freebox and exporter are on the same network

### Missing metrics
- Verify the endpoint is enabled in your `config.toml`
- Check if your Freebox model supports the specific API endpoint
- Include raw JSON data when reporting missing metrics
- Try increasing log verbosity: `./freebox-exporter-rs -v Debug auto`

### Performance issues
- Adjust the `refresh` interval in `config.toml` (default: 5 seconds)
- Monitor Freebox API rate limiting
- Check network connectivity between exporter and Freebox

### WiFi issues
- If you see "channel survey history is empty" at debug level, this is normal
- WiFi stations with unresolved hostnames can be controlled via `policies.unresolved_station_hostnames` setting
- Some WiFi features may be unavailable in bridge mode

### System metrics issues
- CPU temperature metrics automatically adapt to your Freebox generation (legacy vs Ultra format)
- Missing temperature sensors are normal on some Freebox models
- HDD temperature only available on models with internal storage

## Advanced Troubleshooting

### Enabling Debug Logging

For detailed troubleshooting, enable debug logging:

```bash
./freebox-exporter-rs -v Debug auto
```

### Manual API Testing

You can manually test Freebox API endpoints to verify connectivity:

```bash
# Test basic API access
curl -k https://mafreebox.freebox.fr/api_version

# Test with authentication (replace TOKEN)
curl -ks -H "X-Fbx-App-Auth: YOUR_TOKEN" https://mafreebox.freebox.fr/api/v4/system
```

### Configuration Validation

Verify your configuration file syntax:

```bash
# Check configuration parsing
./freebox-exporter-rs -c config.toml --help
```

### Network Troubleshooting

If the exporter cannot reach your Freebox:

```bash
# Test basic connectivity
ping mafreebox.freebox.fr

# Test HTTPS connectivity  
curl -k https://mafreebox.freebox.fr/api_version

# Check if API is enabled on Freebox
# Navigate to: Settings → Network → API → Allow LAN API access
```

## Need Help?

- **Bug reports:** [GitHub Issues](https://github.com/shackerd/freebox-exporter-rs/issues) with diagnostic data
- **Questions:** [Matrix chat](https://matrix.to/#/#freebox-exporter-rs:matrix.org) or [Discord](https://discord.gg/QfV2D2KZ)
- **Feature requests:** [GitHub Discussions](https://github.com/shackerd/freebox-exporter-rs/discussions)

> [!TIP]
> Including raw JSON data with your bug reports significantly speeds up issue resolution, as developers can reproduce the exact scenario with your hardware configuration.

## Known Issues & Workarounds

### Issue: WiFi metrics not appearing
**Cause:** WiFi might be disabled or in bridge mode  
**Solution:** Enable WiFi or check bridge mode settings

### Issue: Some temperature sensors missing  
**Cause:** Hardware differences between Freebox generations  
**Solution:** This is expected behavior, different models have different sensors

### Issue: Rate limiting errors
**Cause:** Refresh interval too low  
**Solution:** Increase `refresh` interval in config.toml to 10+ seconds

### Issue: Certificate errors
**Cause:** Freebox using self-signed certificates  
**Solution:** The exporter automatically handles this, no action needed