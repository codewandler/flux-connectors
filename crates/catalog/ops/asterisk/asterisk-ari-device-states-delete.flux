op asterisk-ari-device-states-delete(deviceName: String) -> Any
  description "Destroy a device-state controlled by ARI."
  risk "destructive"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/deviceStates/{deviceName}")
  response = http.request(method: "DELETE", url)
  return response
