op fly-machine-events-list(app_name: String, machine_id: String) -> Any
  description "List the recorded lifecycle events for one Fly Machine; this is history, not a durable event subscription. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error`, its error code at `/status` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.machines.dev/v1"
  url = fmt("{base}/apps/{app_name}/machines/{machine_id}/events")
  response = http.request(method: "GET", url)
  return response
