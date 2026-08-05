op fly-machine-get(app_name: String, machine_id: String) -> Any
  description "Get one Fly Machine's current configuration, state, region, addresses and recent events. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error`, its error code at `/status` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.machines.dev/v1"
  url = fmt("{base}/apps/{app_name}/machines/{machine_id}")
  response = http.request(method: "GET", url)
  return response
