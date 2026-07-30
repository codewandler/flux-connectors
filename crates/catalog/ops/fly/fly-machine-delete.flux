op fly-machine-delete(app_name: String, machine_id: String) -> Any
  description "Permanently delete a stopped Fly Machine. Forced deletion is intentionally unavailable, so a running Machine is refused instead of killed implicitly. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error`, its error code at `/status` in the response body."
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://api.machines.dev/v1"
  url = fmt("{base}/apps/{app_name}/machines/{machine_id}")
  response = http.request(method: "DELETE", url)
  return response
