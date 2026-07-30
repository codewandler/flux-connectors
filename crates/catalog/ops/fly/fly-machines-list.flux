op fly-machines-list(app_name: String) -> Any
  description "List every Machine in one Fly app, without optional state filters or lease details. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error`, its error code at `/status` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  $base = "https://api.machines.dev/v1"
  $url = fmt("{base}/apps/{app_name}/machines")
  $response = http.request({ method: "GET", url: $url })
  return $response
