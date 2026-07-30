op fly-machine-stop(app_name: String, machine_id: String) -> Any
  description "Stop a running Fly Machine using the API's default signal and timeout, interrupting its service. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error`, its error code at `/status` in the response body."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  $base = "https://api.machines.dev/v1"
  $url = fmt("{base}/apps/{app_name}/machines/{machine_id}/stop")
  $response = http.request({ method: "POST", url: $url })
  return $response
