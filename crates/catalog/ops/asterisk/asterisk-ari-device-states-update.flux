op asterisk-ari-device-states-update(deviceName: String, deviceState: String) -> Any
  description "Change the state of a device controlled by ARI. (Note - implicitly creates the device state)."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/deviceStates/{deviceName}")
  response = http.request(method: "PUT", query: { deviceState }, url)
  return response
