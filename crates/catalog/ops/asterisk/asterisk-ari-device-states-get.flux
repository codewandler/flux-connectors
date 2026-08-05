op asterisk-ari-device-states-get(deviceName: String) -> Any
  description "Retrieve the current state of a device."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/deviceStates/{deviceName}")
  response = http.request(method: "GET", url)
  return response
