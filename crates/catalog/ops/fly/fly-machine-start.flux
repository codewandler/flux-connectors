op fly-machine-start(app_name: String, machine_id: String) -> Any
  description "Start a stopped Fly Machine, restoring service and beginning billable compute. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error`, its error code at `/status` in the response body."
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://api.machines.dev/v1"
  url = fmt("{base}/apps/{app_name}/machines/{machine_id}/start")
  response = http.request(method: "POST", url)
  return response
